//! Agent listing and health check commands.

#[cfg(windows)]
use std::path::PathBuf;

use crate::agent_gateway::{
    build_agent_health_from_meta, check_agent_active_from_lines, extract_sessions, invoke_tool,
    is_remote_session_active, is_session_active,
};
use crate::app_init::home_dir_string;
use crate::ssh_core::{ssh_exec, ssh_read_file};

#[cfg(target_os = "windows")]
use crate::platform::windows::tail_lines_from_file;

use super::gateway::is_openclaw_gateway_alive;
use super::{AgentHealth, AgentInfo, HealthResult};

/// Built-in assets directory (read-only in production).

#[tauri::command]
pub async fn get_agents(
    mode: Option<String>,
    url: Option<String>,
    token: Option<String>,
    ssh_host: Option<String>,
    ssh_user: Option<String>,
) -> Result<Vec<AgentInfo>, String> {
    log::info!("[get_agents] mode={:?} ssh_host={:?}", mode, ssh_host);
    if mode.as_deref() == Some("remote") {
        let sh = ssh_host.as_deref().unwrap_or("");
        let su = ssh_user.as_deref().unwrap_or("");
        if !sh.is_empty() && !su.is_empty() {
            let dirs = ssh_exec(sh, su, "ls -1 $HOME/.openclaw/agents/ 2>/dev/null").await?;
            let mut agents: Vec<AgentInfo> = Vec::new();
            for id in dirs.lines().filter(|l| !l.trim().is_empty()) {
                let id = id.trim().to_string();
                let config_path = format!("$HOME/.openclaw/agents/{}/agent.json", id);
                let (name, emoji) = match ssh_read_file(sh, su, &config_path).await {
                    Ok(c) => {
                        let val: serde_json::Value = serde_json::from_str(&c).unwrap_or_default();
                        (
                            val.get("identityName")
                                .or_else(|| val.get("identity_name"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            val.get("identityEmoji")
                                .or_else(|| val.get("identity_emoji"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        )
                    }
                    Err(_) => (None, None),
                };
                agents.push(AgentInfo {
                    id,
                    identity_name: name,
                    identity_emoji: emoji,
                });
            }
            return Ok(agents);
        }
        // Gateway API fallback
        let url = url.as_deref().unwrap_or("");
        let token = token.as_deref().unwrap_or("");
        let result = invoke_tool(url, token, "agents_list", serde_json::json!({})).await?;
        let r = result.get("result").unwrap_or(&result);
        let agents_arr = r
            .pointer("/details/agents")
            .and_then(|v| v.as_array())
            .or_else(|| r.as_array());
        let agents: Vec<AgentInfo> = if let Some(arr) = agents_arr {
            arr.iter()
                .filter_map(|v| {
                    let id = v["id"].as_str()?.to_string();
                    Some(AgentInfo {
                        id,
                        identity_name: v
                            .get("identityName")
                            .or_else(|| v.get("identity_name"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        identity_emoji: v
                            .get("identityEmoji")
                            .or_else(|| v.get("identity_emoji"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                    })
                })
                .collect()
        } else if let Some(map) = r.as_object() {
            map.iter()
                .filter(|(_, v)| v.is_object())
                .map(|(id, val)| AgentInfo {
                    id: id.clone(),
                    identity_name: val
                        .get("identityName")
                        .or_else(|| val.get("identity_name"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    identity_emoji: val
                        .get("identityEmoji")
                        .or_else(|| val.get("identity_emoji"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                })
                .collect()
        } else {
            return Err(format!("unexpected agents_list format: {}", r));
        };
        return Ok(agents);
    }

    // === local mode ===
    // On Windows, read ~/.openclaw/agents/ directly (no CLI dependency).
    // On macOS/Linux, use the original `openclaw agents list --json` CLI.
    #[cfg(windows)]
    {
        let home = home_dir_string();
        let agents_dir = PathBuf::from(&home).join(".openclaw").join("agents");
        log::info!(
            "[get_agents] local mode, agents_dir={:?}, exists={}",
            agents_dir,
            agents_dir.exists()
        );

        let entries = std::fs::read_dir(&agents_dir).map_err(|e| {
            log::error!("[get_agents] read_dir failed: {}", e);
            format!("read agents dir: {}", e)
        })?;

        let mut agents: Vec<AgentInfo> = Vec::new();
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().to_string();
            let config_path = path.join("agent.json");
            let (name, emoji) = if config_path.exists() {
                match std::fs::read_to_string(&config_path) {
                    Ok(c) => {
                        let val: serde_json::Value = serde_json::from_str(&c).unwrap_or_default();
                        (
                            val.get("identityName")
                                .or_else(|| val.get("identity_name"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            val.get("identityEmoji")
                                .or_else(|| val.get("identity_emoji"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        )
                    }
                    Err(_) => (None, None),
                }
            } else {
                (None, None)
            };
            agents.push(AgentInfo {
                id,
                identity_name: name,
                identity_emoji: emoji,
            });
        }
        Ok(agents)
    }
    #[cfg(not(windows))]
    {
        let output = tokio::process::Command::new("openclaw")
            .args(["agents", "list", "--json"])
            .output()
            .await
            .map_err(|e| format!("openclaw agents list: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("openclaw agents list failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json_start = stdout.find('[').ok_or("no JSON array in agents output")?;
        let json_end = stdout.rfind(']').ok_or("no closing bracket")? + 1;
        let agents: Vec<AgentInfo> =
            serde_json::from_str(&stdout[json_start..json_end]).map_err(|e| e.to_string())?;
        Ok(agents)
    }
}

#[tauri::command]
pub async fn get_health(
    mode: Option<String>,
    url: Option<String>,
    token: Option<String>,
    ssh_host: Option<String>,
    ssh_user: Option<String>,
) -> Result<HealthResult, String> {
    log::info!("[get_health] mode={:?} ssh_host={:?}", mode, ssh_host);
    if mode.as_deref() == Some("remote") {
        let sh = ssh_host.as_deref().unwrap_or("");
        let su = ssh_user.as_deref().unwrap_or("");
        if !sh.is_empty() && !su.is_empty() {
            // Single SSH command: read sessions.json + tail each session file per agent
            let cmd = r#"for d in $HOME/.openclaw/agents/*/; do id=$(basename "$d"); sj="$d/sessions/sessions.json"; echo "AGENT:$id"; if [ -f "$sj" ]; then echo "META_START"; cat "$sj"; echo ""; echo "META_END"; fi; for f in "$d"sessions/*.jsonl; do [ -f "$f" ] || continue; echo "TAIL:$(basename "$f")"; tail -5 "$f"; echo "END_TAIL"; done; echo "END_AGENT"; done"#;
            let output = ssh_exec(sh, su, cmd).await.unwrap_or_default();

            let mut agents = Vec::new();
            let mut current_id: Option<String> = None;
            let mut meta_buf = String::new();
            let mut in_meta = false;
            // filename -> tail lines
            let mut tails: std::collections::HashMap<String, Vec<String>> =
                std::collections::HashMap::new();
            let mut current_tail_file: Option<String> = None;
            let mut tail_lines: Vec<String> = Vec::new();

            for line in output.lines() {
                if let Some(id) = line.strip_prefix("AGENT:") {
                    // Finalize previous agent
                    if let Some(prev_id) = current_id.take() {
                        let agent = build_agent_health_from_meta(&prev_id, &meta_buf, &tails);
                        agents.push(agent);
                    }
                    current_id = Some(id.to_string());
                    meta_buf.clear();
                    tails.clear();
                    in_meta = false;
                } else if line == "META_START" {
                    in_meta = true;
                    meta_buf.clear();
                } else if line == "META_END" {
                    in_meta = false;
                } else if in_meta {
                    meta_buf.push_str(line);
                    meta_buf.push('\n');
                } else if let Some(fname) = line.strip_prefix("TAIL:") {
                    if let Some(prev_file) = current_tail_file.take() {
                        tails.insert(prev_file, std::mem::take(&mut tail_lines));
                    }
                    current_tail_file = Some(fname.to_string());
                    tail_lines.clear();
                } else if line == "END_TAIL" {
                    if let Some(prev_file) = current_tail_file.take() {
                        tails.insert(prev_file, std::mem::take(&mut tail_lines));
                    }
                } else if line == "END_AGENT" {
                    if let Some(prev_file) = current_tail_file.take() {
                        tails.insert(prev_file, std::mem::take(&mut tail_lines));
                    }
                    if let Some(prev_id) = current_id.take() {
                        let agent = build_agent_health_from_meta(&prev_id, &meta_buf, &tails);
                        agents.push(agent);
                    }
                    meta_buf.clear();
                    tails.clear();
                } else if current_tail_file.is_some() {
                    tail_lines.push(line.to_string());
                }
            }
            // Handle last agent if no END_AGENT
            if let Some(prev_file) = current_tail_file.take() {
                tails.insert(prev_file, tail_lines);
            }
            if let Some(prev_id) = current_id {
                let agent = build_agent_health_from_meta(&prev_id, &meta_buf, &tails);
                agents.push(agent);
            }
            return Ok(HealthResult {
                agents,
                gateway_alive: true,
            });
        }
        // Gateway API fallback
        let url = url.as_deref().unwrap_or("");
        let token = token.as_deref().unwrap_or("");
        let result = invoke_tool(
            url,
            token,
            "sessions_list",
            serde_json::json!({"activeMinutes": 5}),
        )
        .await?;
        let sessions = extract_sessions(&result);
        let mut agent_active: std::collections::HashMap<String, bool> =
            std::collections::HashMap::new();
        for s in &sessions {
            let agent_id = s["agentId"]
                .as_str()
                .or_else(|| s["key"].as_str().and_then(|k| k.split(':').nth(1)))
                .unwrap_or("main")
                .to_string();
            let session_key = s["key"].as_str().unwrap_or("");
            let active = if !session_key.is_empty() {
                is_remote_session_active(url, token, session_key, s).await
            } else {
                is_session_active(s)
            };
            let entry = agent_active.entry(agent_id).or_insert(false);
            if active {
                *entry = true;
            }
        }
        let agents = agent_active
            .into_iter()
            .map(|(agent_id, active)| AgentHealth {
                agent_id,
                active,
                sessions: vec![],
            })
            .collect();
        return Ok(HealthResult {
            agents,
            gateway_alive: true,
        });
    }

    // === local mode -- content-based detection with session-level data ===
    let home = home_dir_string();
    let agents_dir = std::path::PathBuf::from(&home)
        .join(".openclaw")
        .join("agents");

    // If the OpenClaw gateway process is not running, every session's "active"
    // state in the JSONL files is stale -- the gateway was killed mid-turn and
    // never wrote a final inactive message. Force all agents to inactive.
    let gateway_alive = is_openclaw_gateway_alive();
    log::info!("[get_health] local mode, gateway_alive={}", gateway_alive);

    let mut agents = Vec::new();
    let Ok(entries) = std::fs::read_dir(&agents_dir) else {
        return Err("read agents dir".into());
    };
    for entry in entries.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()) {
        let agent_id = entry.file_name().to_string_lossy().to_string();
        let agent_dir = entry.path();
        let sessions_dir = agent_dir.join("sessions");
        let meta_path = sessions_dir.join("sessions.json");

        // Try to read sessions.json and build per-session health
        if meta_path.exists() {
            if let Ok(meta_str) = std::fs::read_to_string(&meta_path) {
                // Build tails map: basename -> last 5 lines
                let mut tails: std::collections::HashMap<String, Vec<String>> =
                    std::collections::HashMap::new();
                if let Ok(rd) = std::fs::read_dir(&sessions_dir) {
                    for fe in rd.filter_map(|e| e.ok()) {
                        let p = fe.path();
                        if p.extension().map_or(true, |ext| ext != "jsonl") {
                            continue;
                        }
                        #[cfg(windows)]
                        let lines = tail_lines_from_file(&p, 5);
                        #[cfg(not(windows))]
                        let lines = {
                            let out = tokio::process::Command::new("tail")
                                .args(["-5", &p.to_string_lossy()])
                                .output()
                                .await
                                .ok();
                            out.map(|o| {
                                String::from_utf8_lossy(&o.stdout)
                                    .lines()
                                    .map(|l| l.to_string())
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default()
                        };
                        if !lines.is_empty() {
                            if let Some(fname) = p.file_name() {
                                tails.insert(fname.to_string_lossy().to_string(), lines);
                            }
                        }
                    }
                }
                let mut agent = build_agent_health_from_meta(&agent_id, &meta_str, &tails);
                // Gateway dead -> all sessions are stale, force everything inactive
                if !gateway_alive {
                    agent.active = false;
                    for s in &mut agent.sessions {
                        s.active = false;
                    }
                }
                agents.push(agent);
                continue;
            }
        }

        // Fallback: no sessions.json, check most recent file only
        let latest = std::fs::read_dir(&sessions_dir).ok().and_then(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
                .max_by_key(|e| e.metadata().ok().and_then(|m| m.modified().ok()))
        });
        let active = if let Some(f) = latest {
            #[cfg(windows)]
            let lines = tail_lines_from_file(&f.path(), 5);
            #[cfg(not(windows))]
            let lines = {
                let out = tokio::process::Command::new("tail")
                    .args(["-5", &f.path().to_string_lossy()])
                    .output()
                    .await
                    .ok();
                out.map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .map(|l| l.to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
            };
            // Only trust JSONL content if gateway is still running
            gateway_alive && check_agent_active_from_lines(&lines)
        } else {
            false
        };
        agents.push(AgentHealth {
            agent_id,
            active,
            sessions: vec![],
        });
    }

    Ok(HealthResult {
        agents,
        gateway_alive,
    })
}
