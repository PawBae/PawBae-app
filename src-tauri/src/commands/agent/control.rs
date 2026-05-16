//! Agent interrupt and extra-info commands.

use std::path::PathBuf;

use crate::agent_gateway::{remote_sessions_json_path, sessions_json_path};
use crate::app_init::home_dir_string;
use crate::lsof::lsof_open_jsonl_paths;
use crate::ssh_core::{ssh_exec, ssh_read_file};
use crate::state::ActiveAgentPid;

use super::{AgentExtraInfo, DailyCount};

#[tauri::command]
pub async fn interrupt_agent(
    agent_id: String,
    state: tauri::State<'_, ActiveAgentPid>,
) -> Result<String, String> {
    // Strategy 1: Send interrupt signal to the tracked openclaw agent subprocess (pet-window turns)
    let tracked_pid = *state.pid.lock().unwrap();
    if let Some(pid) = tracked_pid {
        #[cfg(unix)]
        let killed = unsafe { libc::kill(pid as i32, libc::SIGINT) == 0 };
        #[cfg(windows)]
        let killed = {
            // On Windows, use GenerateConsoleCtrlEvent to send Ctrl+C to the process group,
            // or TerminateProcess as a fallback.
            use windows::Win32::System::Console::GenerateConsoleCtrlEvent;
            use windows::Win32::System::Console::CTRL_BREAK_EVENT;
            unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid).is_ok() }
        };
        if killed {
            return Ok(format!(
                "已向 openclaw agent 进程 (pid={}) 发送中断信号",
                pid
            ));
        }
    }

    // Strategy 2: WebSocket chat.abort (channel-based turns like Feishu/Telegram)
    let home = home_dir_string();

    // 1. Read gateway config
    let config_path = PathBuf::from(&home).join(".openclaw").join("openclaw.json");
    let config_str = tokio::fs::read_to_string(&config_path)
        .await
        .map_err(|e| format!("读取 openclaw.json 失败: {}", e))?;
    let config: serde_json::Value =
        serde_json::from_str(&config_str).map_err(|e| format!("解析 openclaw.json 失败: {}", e))?;
    let port = config["gateway"]["port"].as_u64().unwrap_or(18789) as u16;
    let token = config["gateway"]["auth"]["token"]
        .as_str()
        .unwrap_or("")
        .to_string();
    if token.is_empty() {
        return Err("openclaw.json 中未找到 gateway token".into());
    }

    // 2. Find the ACTIVE session key.
    //    On macOS/Linux: use lsof to find which .jsonl file is currently held open.
    //    On Windows: use recently modified .jsonl files as a heuristic.
    let sess_path = sessions_json_path(&agent_id);
    let content = tokio::fs::read_to_string(&sess_path)
        .await
        .map_err(|e| format!("读取 sessions.json 失败: {}", e))?;
    let sess_map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&content).map_err(|e| e.to_string())?;

    // Get the set of currently active .jsonl file paths
    let open_jsonl_paths = lsof_open_jsonl_paths().await;

    // Match open/active .jsonl paths against sessionFile entries in sessions.json
    let session_key = sess_map
        .iter()
        .find(|(_, v)| {
            if let Some(sf) = v["sessionFile"].as_str() {
                // sessionFile may be exact path or may contain the uuid; check if any open path starts with or equals it
                open_jsonl_paths.iter().any(|p| {
                    p.starts_with(sf) || sf.starts_with(p.as_str())
                    // On Windows, also compare with backslash-normalized paths
                    || p.replace('\\', "/").starts_with(&sf.replace('\\', "/"))
                    || sf.replace('\\', "/").starts_with(&p.replace('\\', "/"))
                })
            } else {
                false
            }
        })
        .map(|(k, _)| k.clone())
        // Fallback: most recently updated session
        .or_else(|| {
            sess_map
                .iter()
                .max_by_key(|(_, v)| v["updatedAt"].as_u64().unwrap_or(0))
                .map(|(k, _)| k.clone())
        })
        .ok_or("没有找到活跃 session")?;

    // 3. WebSocket: wait for challenge -> send connect -> send chat.abort
    let script = format!(
        r#"const ws=new WebSocket('ws://127.0.0.1:{port}/');const t=setTimeout(()=>{{process.stderr.write('timeout');process.exit(1)}},6000);let ok=false;ws.onmessage=(e)=>{{const d=JSON.parse(e.data);if(d.event==='connect.challenge'){{ws.send(JSON.stringify({{type:'req',id:'c',method:'connect',params:{{auth:{{token:'{token}'}},minProtocol:3,maxProtocol:3,client:{{id:'gateway-client',platform:'darwin',mode:'backend',version:'0.1.0'}},role:'operator',scopes:['operator.admin'],caps:[]}}}}))}}else if(d.id==='c'&&d.ok&&!ok){{ok=true;ws.send(JSON.stringify({{type:'req',id:'a',method:'chat.abort',params:{{sessionKey:'{sk}',stopReason:'user'}}}}))}}else if(d.id==='c'&&!d.ok){{process.stderr.write(d.error?.message||'connect failed');clearTimeout(t);ws.close();process.exit(1)}}else if(d.id==='a'){{process.stdout.write(JSON.stringify(d.payload||d));clearTimeout(t);ws.close();process.exit(0)}}}};ws.onerror=(e)=>{{process.stderr.write(e.message||'ws error');process.exit(1)}};"#,
        port = port,
        token = token,
        sk = session_key,
    );

    let output = tokio::process::Command::new("node")
        .args(["-e", &script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("node: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("打断失败: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let aborted = stdout.contains("\"aborted\":true");
    if aborted {
        Ok(format!("已打断 ({})", session_key))
    } else {
        Ok(format!("指令已发送，当前无活跃 run ({})", session_key))
    }
}

#[tauri::command]
pub async fn get_agent_extra_info(
    agent_id: String,
    mode: Option<String>,
    ssh_host: Option<String>,
    ssh_user: Option<String>,
) -> Result<AgentExtraInfo, String> {
    if mode.as_deref() == Some("remote") {
        let sh = ssh_host.as_deref().unwrap_or("");
        let su = ssh_user.as_deref().unwrap_or("");
        if !sh.is_empty() && !su.is_empty() {
            let agent_dir = if agent_id.is_empty() {
                "main"
            } else {
                &agent_id
            };

            // Skills from remote sessions.json
            let sess_path = remote_sessions_json_path(&agent_id);
            let skills: Vec<String> = if let Ok(content) = ssh_read_file(sh, su, &sess_path).await {
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&content)
                    .ok()
                    .and_then(|map| {
                        map.into_values()
                            .max_by_key(|v| v["updatedAt"].as_u64().unwrap_or(0))
                            .and_then(|v| v["skillsSnapshot"]["skills"].as_array().cloned())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|s| s["name"].as_str().map(|n| n.to_string()))
                                    .collect()
                            })
                    })
                    .unwrap_or_default()
            } else {
                vec![]
            };

            // Daily counts from remote .jsonl files
            // Use find+exec to avoid ARG_MAX with many files, and process server-side
            // to minimise SSH data transfer.
            let mut daily_calls: std::collections::HashMap<String, u32> =
                std::collections::HashMap::new();
            let mut daily_tokens: std::collections::HashMap<String, u64> =
                std::collections::HashMap::new();

            // Server-side: extract "date calls tokens" summary per day using awk
            // Also output server's "today" to avoid timezone mismatch with local machine
            let summary_cmd = format!(
                concat!(
                    "find ~/.openclaw/agents/{}/sessions -name '*.jsonl' -exec cat {{}} + 2>/dev/null | ",
                    "awk '{{ ",
                    "  if (match($0, /\"timestamp\":\"([0-9]{{4}}-[0-9]{{2}}-[0-9]{{2}})/, a)) {{ d=a[1]; c[d]++ }} ",
                    "  if (match($0, /\"totalTokens\":([0-9]+)/, b) && d) t[d]+=b[1] ",
                    "}} END {{ for (d in c) print d, c[d], t[d]+0 }}' && echo \"SERVER_TODAY:$(date +%Y-%m-%d)\""
                ),
                agent_dir
            );
            log::info!(
                "[get_agent_extra_info] running daily summary cmd for agent={}",
                agent_dir
            );
            let mut server_today: Option<String> = None;
            match ssh_exec(sh, su, &summary_cmd).await {
                Ok(summary) => {
                    for line in summary.lines() {
                        if let Some(date_str) = line.strip_prefix("SERVER_TODAY:") {
                            server_today = Some(date_str.trim().to_string());
                            continue;
                        }
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 3 {
                            let date = parts[0].to_string();
                            let calls: u32 = parts[1].parse().unwrap_or(0);
                            let tokens: u64 = parts[2].parse().unwrap_or(0);
                            daily_calls.insert(date.clone(), calls);
                            daily_tokens.insert(date, tokens);
                        }
                    }
                    log::info!(
                        "[get_agent_extra_info] parsed {} daily entries, server_today={:?}",
                        daily_calls.len(),
                        server_today
                    );
                }
                Err(e) => {
                    log::warn!(
                        "[get_agent_extra_info] daily summary cmd failed: {}, trying fallback",
                        e
                    );
                    // Fallback: cat with find (no glob), limited output
                    let cat_cmd = format!(
                        "find ~/.openclaw/agents/{}/sessions -name '*.jsonl' -exec cat {{}} + 2>/dev/null | tail -n 30000",
                        agent_dir
                    );
                    if let Ok(content) = ssh_exec(sh, su, &cat_cmd).await {
                        let mut current_date: Option<String> = None;
                        for line in content.lines() {
                            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
                                if let Some(ts) = obj["timestamp"].as_str() {
                                    if ts.len() >= 10 {
                                        current_date = Some(ts[..10].to_string());
                                        *daily_calls.entry(ts[..10].to_string()).or_insert(0) += 1;
                                    }
                                }
                                if obj["type"].as_str() == Some("message") {
                                    if let Some(total) =
                                        obj["message"]["usage"]["totalTokens"].as_u64()
                                    {
                                        if let Some(ref date) = current_date {
                                            *daily_tokens.entry(date.clone()).or_insert(0) += total;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            use chrono::{Duration, Local, NaiveDate};
            let today = server_today
                .as_deref()
                .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
                .unwrap_or_else(|| Local::now().date_naive());
            let daily_counts: Vec<DailyCount> = (0..14i64)
                .rev()
                .map(|i| {
                    let date = (today - Duration::days(i)).format("%Y-%m-%d").to_string();
                    let count = daily_calls.get(&date).copied().unwrap_or(0);
                    let tokens = daily_tokens.get(&date).copied().unwrap_or(0);
                    DailyCount {
                        date,
                        count,
                        tokens,
                    }
                })
                .collect();

            return Ok(AgentExtraInfo {
                skills,
                cron_jobs: vec![],
                daily_counts,
            });
        }
        return Ok(AgentExtraInfo {
            skills: vec![],
            cron_jobs: vec![],
            daily_counts: vec![],
        });
    }

    let home = home_dir_string();
    let agent_dir = if agent_id.is_empty() {
        "main"
    } else {
        &agent_id
    };

    // 1. Skills from sessions.json (most recently updated session)
    let skills: Vec<String> =
        if let Ok(content) = tokio::fs::read_to_string(sessions_json_path(&agent_id)).await {
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&content)
                .ok()
                .and_then(|map| {
                    map.into_values()
                        .max_by_key(|v| v["updatedAt"].as_u64().unwrap_or(0))
                        .and_then(|v| v["skillsSnapshot"]["skills"].as_array().cloned())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|s| s["name"].as_str().map(|n| n.to_string()))
                                .collect()
                        })
                })
                .unwrap_or_default()
        } else {
            vec![]
        };

    // 2. Cron jobs filtered by agent
    let cron_jobs: Vec<serde_json::Value> = tokio::process::Command::new("openclaw")
        .args(["cron", "list", "--json"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| {
            let i = s.find('{')?;
            serde_json::from_str::<serde_json::Value>(&s[i..]).ok()
        })
        .and_then(|v| v["jobs"].as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter(|j| {
            let job_agent = j["agentId"].as_str().unwrap_or("main");
            let target = if agent_id.is_empty() {
                "main"
            } else {
                &agent_id
            };
            job_agent == target || (target == "main" && job_agent.is_empty())
        })
        .collect();

    // 3. Daily call counts + token usage -- last 14 days from .jsonl files
    let sessions_dir = format!("{}/.openclaw/agents/{}/sessions", home, agent_dir);
    let mut daily_calls: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut daily_tokens: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    if let Ok(mut dir) = tokio::fs::read_dir(&sessions_dir).await {
        while let Ok(Some(entry)) = dir.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                let mut current_date: Option<String> = None;
                for line in content.lines() {
                    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
                        if let Some(ts) = obj["timestamp"].as_str() {
                            if ts.len() >= 10 {
                                current_date = Some(ts[..10].to_string());
                                *daily_calls.entry(ts[..10].to_string()).or_insert(0) += 1;
                            }
                        }
                        // Accumulate tokens from assistant message usage
                        if obj["type"].as_str() == Some("message") {
                            if let Some(total) = obj["message"]["usage"]["totalTokens"].as_u64() {
                                if let Some(ref date) = current_date {
                                    *daily_tokens.entry(date.clone()).or_insert(0) += total;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    use chrono::{Duration, Local};
    let today = Local::now().date_naive();
    let daily_counts: Vec<DailyCount> = (0..14i64)
        .rev()
        .map(|i| {
            let date = (today - Duration::days(i)).format("%Y-%m-%d").to_string();
            let count = daily_calls.get(&date).copied().unwrap_or(0);
            let tokens = daily_tokens.get(&date).copied().unwrap_or(0);
            DailyCount {
                date,
                count,
                tokens,
            }
        })
        .collect();

    Ok(AgentExtraInfo {
        skills,
        cron_jobs,
        daily_counts,
    })
}
