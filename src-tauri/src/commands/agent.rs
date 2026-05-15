//! Tauri agent-facing commands: status, chat send, agent listing, health, metrics, interrupt, extra-info.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agent_gateway::{
    build_agent_health_from_meta, check_agent_active_from_lines, extract_sessions, invoke_tool,
    is_remote_session_active, is_session_active, remote_sessions_json_path, sessions_json_path,
};
use crate::app_init::home_dir_string;
use crate::lsof::{lsof_active_agents, lsof_open_jsonl_paths};
use crate::ssh_core::{ssh_exec, ssh_is_agent_active, ssh_read_file};
use crate::state::{ActiveAgentPid, SessionInfo};

#[cfg(target_os = "windows")]
use crate::platform::windows::{hide_window_tokio_cmd, tail_lines_from_file};

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayStatus {
    pub active: bool,
    pub sessions: Vec<SessionInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentInfo {
    pub id: String,
    #[serde(rename = "identityName")]
    pub identity_name: Option<String>,
    #[serde(rename = "identityEmoji")]
    pub identity_emoji: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionHealth {
    pub key: String,
    pub active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentHealth {
    #[serde(rename = "agentId")]
    pub agent_id: String,
    pub active: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<SessionHealth>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResult {
    pub agents: Vec<AgentHealth>,
    /// Whether the local OpenClaw gateway process is running.
    /// Always `true` for remote connections (we can't check remote process).
    /// Frontend uses this to auto-remove the local connection when gateway is dead.
    #[serde(default = "default_true", rename = "gatewayAlive")]
    pub gateway_alive: bool,
}

fn default_true() -> bool {
    true
}
/// Check whether the local OpenClaw gateway process is alive.
///
/// OpenClaw uses a lock file at `$TMPDIR/openclaw-<uid>/gateway.<hash>.lock`
/// containing `{"pid": <n>, ...}`. When the gateway shuts down it deletes the
/// lock file. If the file is missing or the PID inside is no longer running,
/// the gateway is considered dead — meaning any "active" session state left in
/// the JSONL files is stale and should be forced to inactive.
fn is_openclaw_gateway_alive() -> bool {
    // Build the lock directory: $TMPDIR/openclaw-<uid>
    let tmp = std::env::temp_dir();
    #[cfg(unix)]
    let lock_dir = {
        let uid = unsafe { libc::getuid() };
        tmp.join(format!("openclaw-{}", uid))
    };
    #[cfg(windows)]
    let lock_dir = tmp.join("openclaw");

    // Look for any gateway.*.lock file in the lock directory
    let rd = match std::fs::read_dir(&lock_dir) {
        Ok(rd) => rd,
        Err(_) => return false, // no lock dir → gateway not running
    };
    for entry in rd.filter_map(|e| e.ok()) {
        let fname = entry.file_name();
        let name = fname.to_string_lossy();
        if !name.starts_with("gateway.") || !name.ends_with(".lock") {
            continue;
        }
        // Read the lock file to extract the PID
        if let Ok(contents) = std::fs::read_to_string(entry.path()) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&contents) {
                if let Some(pid) = val["pid"].as_u64() {
                    // Check if the process is still alive (kill -0)
                    #[cfg(unix)]
                    {
                        let alive = unsafe { libc::kill(pid as libc::pid_t, 0) } == 0;
                        if alive {
                            return true;
                        }
                    }
                    #[cfg(windows)]
                    {
                        // OpenProcess with PROCESS_QUERY_LIMITED_INFORMATION
                        // returns Ok(handle) if the process exists.
                        use windows::Win32::System::Threading::{
                            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
                        };
                        if let Ok(handle) = unsafe {
                            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid as u32)
                        } {
                            let _ = unsafe { windows::Win32::Foundation::CloseHandle(handle) };
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}
#[tauri::command]
pub async fn get_status(
    _gateway_url: String,
    _token: String,
    agent_id: String,
) -> Result<GatewayStatus, String> {
    // Step 1: check gateway is running
    #[cfg(unix)]
    {
        let pgrep_gw = tokio::process::Command::new("pgrep")
            .args(["-x", "openclaw-gateway"])
            .output()
            .await
            .map_err(|e| format!("pgrep: {}", e))?;
        if !pgrep_gw.status.success() {
            return Err("gateway not running".into());
        }
    }
    #[cfg(windows)]
    {
        // On Windows, openclaw gateway runs as a node.exe process (not a separate
        // openclaw-gateway.exe binary).  Check whether anything is listening on
        // the default gateway port (18789) instead.
        let mut ps_cmd = tokio::process::Command::new("powershell");
        ps_cmd.args([
                "-NoProfile",
                "-Command",
                "(Get-NetTCPConnection -LocalPort 18789 -State Listen -ErrorAction SilentlyContinue | Measure-Object).Count",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        hide_window_tokio_cmd(&mut ps_cmd);
        let listening = ps_cmd
            .output()
            .await
            .map_err(|e| format!("powershell: {}", e))?;
        let count_str = String::from_utf8_lossy(&listening.stdout)
            .trim()
            .to_string();
        let count: u32 = count_str.parse().unwrap_or(0);
        if count == 0 {
            return Err("gateway not running".into());
        }
    }

    // Step 2: check if any .jsonl is being actively used for this agent
    let active_agents = lsof_active_agents().await;
    let agent_dir = if agent_id.is_empty() {
        "main"
    } else {
        &agent_id
    };
    let active = active_agents.contains(agent_dir);

    // Step 3: read sessions.json → session list
    let path = sessions_json_path(&agent_id);
    let sessions = match tokio::fs::read_to_string(&path).await {
        Ok(content) => {
            let map: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&content).unwrap_or_default();
            map.iter()
                .map(|(key, val)| SessionInfo {
                    id: val["sessionId"].as_str().unwrap_or(key).to_string(),
                    label: Some(key.clone()),
                    status: "stored".into(),
                    model: None,
                    channel: val["lastChannel"].as_str().map(|s| s.to_string()),
                })
                .collect()
        }
        Err(_) => vec![],
    };

    Ok(GatewayStatus { active, sessions })
}

#[tauri::command]
pub async fn send_chat(
    message: String,
    agent_id: String,
    state: tauri::State<'_, ActiveAgentPid>,
) -> Result<String, String> {
    // Read sessions.json to get the first sessionId
    let path = sessions_json_path(&agent_id);
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("read sessions.json: {}", e))?;
    let map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&content).map_err(|e| e.to_string())?;

    let session_id = map
        .values()
        .find_map(|v| v["sessionId"].as_str())
        .ok_or("no session found")?
        .to_string();

    // Spawn openclaw agent and track its PID so interrupt_agent can SIGINT it
    let child = tokio::process::Command::new("openclaw")
        .args([
            "agent",
            "--message",
            &message,
            "--session-id",
            &session_id,
            "--json",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("openclaw agent: {}", e))?;

    // Store PID for interrupt_agent
    if let Some(pid) = child.id() {
        *state.pid.lock().unwrap() = Some(pid);
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| format!("openclaw agent wait: {}", e))?;

    // Clear PID once done
    *state.pid.lock().unwrap() = None;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Try to parse JSON from stdout first — exit code may be non-zero due to config warnings
    // even when the agent turn succeeded
    if let Some(json_start) = stdout.find('{') {
        if let Ok(body) = serde_json::from_str::<serde_json::Value>(&stdout[json_start..]) {
            let reply = body["result"]["payloads"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|p| p["text"].as_str())
                .unwrap_or("")
                .to_string();
            return Ok(reply);
        }
    }

    // No usable JSON — treat as real failure
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("openclaw agent failed: {}", stderr));
    }

    Ok(String::new())
}

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
            // filename → tail lines
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

    // === local mode — content-based detection with session-level data ===
    let home = home_dir_string();
    let agents_dir = std::path::PathBuf::from(&home)
        .join(".openclaw")
        .join("agents");

    // If the OpenClaw gateway process is not running, every session's "active"
    // state in the JSONL files is stale — the gateway was killed mid-turn and
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
                // Build tails map: basename → last 5 lines
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
                // Gateway dead → all sessions are stale, force everything inactive
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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCallStat {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RecentAction {
    /// "tool" or "text"
    #[serde(rename = "type")]
    pub action_type: String,
    /// tool name (for tool) or text snippet (for text)
    pub summary: String,
    pub detail: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentMetrics {
    #[serde(rename = "agentId")]
    pub agent_id: String,
    pub active: bool,
    #[serde(rename = "currentModel")]
    pub current_model: Option<String>,
    #[serde(rename = "thinkingLevel")]
    pub thinking_level: Option<String>,
    #[serde(rename = "activeSessionCount")]
    pub active_session_count: usize,
    #[serde(rename = "currentTask")]
    pub current_task: Option<String>,
    #[serde(rename = "currentTool")]
    pub current_tool: Option<String>,
    #[serde(rename = "totalTokens")]
    pub total_tokens: u64,
    #[serde(rename = "inputTokens")]
    pub input_tokens: u64,
    #[serde(rename = "outputTokens")]
    pub output_tokens: u64,
    #[serde(rename = "cacheReadTokens")]
    pub cache_read_tokens: u64,
    #[serde(rename = "cacheWriteTokens")]
    pub cache_write_tokens: u64,
    #[serde(rename = "totalCost")]
    pub total_cost: f64,
    #[serde(rename = "toolCalls")]
    pub tool_calls: Vec<ToolCallStat>,
    #[serde(rename = "recentActions")]
    pub recent_actions: Vec<RecentAction>,
    #[serde(rename = "errorCount")]
    pub error_count: usize,
    #[serde(rename = "messageCount")]
    pub message_count: usize,
    #[serde(rename = "sessionStart")]
    pub session_start: Option<String>,
    #[serde(rename = "lastActivity")]
    pub last_activity: Option<String>,
    pub channel: Option<String>,
}
/// Extract the actual user message from openclaw's metadata-wrapped format.
/// Handles both direct messages and queued messages.
/// Formats:
///   - `Conversation info...\n[message_id: xxx]\nSender: actual message`
///   - `[Queued messages...]\n---\nQueued #N\n...\n[message_id: xxx]\nSender: msg\n---\nQueued #M\n...`
///   - `[timestamp] message` (simple format)
fn extract_user_message(text: &str) -> Option<String> {
    // For queued messages, extract the last queued message's content
    if text.starts_with("[Queued messages") {
        // Find the last "[message_id: ...]" line and take the line after it
        let mut last_msg: Option<String> = None;
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.starts_with("[message_id:") {
                // Next line is "sender: actual message"
                if let Some(next) = lines.get(i + 1) {
                    // Strip "Sender: " prefix if present
                    let content = if let Some(pos) = next.find(": ") {
                        &next[pos + 2..]
                    } else {
                        next
                    };
                    if !content.trim().is_empty() {
                        last_msg = Some(content.trim().to_string());
                    }
                }
            }
        }
        return last_msg.map(|m| truncate_str(&m, 100));
    }

    // For regular messages with metadata wrapper
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("[message_id:") {
            // Next line is "sender: actual message"
            if let Some(next) = lines.get(i + 1) {
                let content = if let Some(pos) = next.find(": ") {
                    &next[pos + 2..]
                } else {
                    next
                };
                if !content.trim().is_empty() {
                    return Some(truncate_str(content.trim(), 100));
                }
            }
        }
    }

    // Simple format: "[timestamp] message"
    if text.starts_with('[') {
        if let Some(end) = text.find(']') {
            let after = text[end + 1..].trim();
            if !after.is_empty() {
                return Some(truncate_str(after, 100));
            }
        }
    }

    // Fallback: first non-empty line
    text.lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| truncate_str(l.trim(), 100))
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        // Truncate at char boundary
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}
#[tauri::command]
pub async fn get_agent_metrics(
    agent_id: String,
    mode: Option<String>,
    url: Option<String>,
    token: Option<String>,
    ssh_host: Option<String>,
    ssh_user: Option<String>,
) -> Result<AgentMetrics, String> {
    log::info!(
        "[get_agent_metrics] agent_id={} mode={:?} ssh_host={:?}",
        agent_id,
        mode,
        ssh_host
    );
    if mode.as_deref() == Some("remote") {
        let sh = ssh_host.as_deref().unwrap_or("");
        let su = ssh_user.as_deref().unwrap_or("");
        if !sh.is_empty() && !su.is_empty() {
            let active = ssh_is_agent_active(sh, su, &agent_id).await;

            let mut metrics = AgentMetrics {
                agent_id: agent_id.clone(),
                active,
                current_model: None,
                thinking_level: None,
                active_session_count: 0,
                current_task: None,
                current_tool: None,
                total_tokens: 0,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                total_cost: 0.0,
                tool_calls: vec![],
                recent_actions: vec![],
                error_count: 0,
                message_count: 0,
                session_start: None,
                last_activity: None,
                channel: None,
            };

            let sess_path = remote_sessions_json_path(&agent_id);
            let sess_content = match ssh_read_file(sh, su, &sess_path).await {
                Ok(c) => c,
                Err(_) => return Ok(metrics),
            };
            let sess_map: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&sess_content).unwrap_or_default();

            metrics.active_session_count = sess_map.len();

            let best_entry = sess_map
                .values()
                .max_by_key(|v| v["updatedAt"].as_u64().unwrap_or(0));
            if let Some(entry) = best_entry {
                metrics.channel = entry["origin"]["surface"].as_str().map(|s| s.to_string());
                metrics.current_model = entry["model"].as_str().map(|s| s.to_string());
            }

            let mut best_session: Option<(String, u64)> = None;
            for val in sess_map.values() {
                if let (Some(file), Some(updated)) =
                    (val["sessionFile"].as_str(), val["updatedAt"].as_u64())
                {
                    if best_session.as_ref().map_or(true, |(_, t)| updated > *t) {
                        best_session = Some((file.to_string(), updated));
                    }
                }
            }

            let session_file = match best_session {
                Some((f, _)) => f,
                None => return Ok(metrics),
            };

            let content = match ssh_read_file(sh, su, &session_file).await {
                Ok(c) => {
                    log::info!(
                        "[get_agent_metrics] SSH read session file OK, len={}",
                        c.len()
                    );
                    c
                }
                Err(e) => {
                    log::error!("[get_agent_metrics] SSH read session file failed: {}", e);
                    return Ok(metrics);
                }
            };

            let mut tool_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            let mut last_user_text: Option<String> = None;
            let mut last_tool_name: Option<String> = None;
            let mut last_timestamp: Option<String> = None;
            let mut recent_actions: Vec<RecentAction> = vec![];
            #[allow(unused_assignments)]
            let mut current_msg_timestamp: Option<String> = None;

            for line in content.lines() {
                let val: serde_json::Value = match serde_json::from_str(line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let event_type = val["type"].as_str().unwrap_or("");
                if let Some(ts) = val["timestamp"].as_str() {
                    last_timestamp = Some(ts.to_string());
                }
                match event_type {
                    "session" => {
                        metrics.session_start = val["timestamp"].as_str().map(|s| s.to_string());
                    }
                    "model_change" => {
                        metrics.current_model = val["modelId"].as_str().map(|s| s.to_string());
                    }
                    "thinking_level_change" => {
                        metrics.thinking_level =
                            val["thinkingLevel"].as_str().map(|s| s.to_string());
                    }
                    "message" => {
                        let msg = &val["message"];
                        let role = msg["role"].as_str().unwrap_or("");
                        current_msg_timestamp = val["timestamp"].as_str().map(|s| s.to_string());
                        if role == "user" {
                            if let Some(content_arr) = msg["content"].as_array() {
                                for item in content_arr {
                                    if item["type"].as_str() == Some("text") {
                                        if let Some(text) = item["text"].as_str() {
                                            last_user_text = extract_user_message(text);
                                        }
                                    }
                                }
                            }
                            metrics.message_count += 1;
                        } else if role == "assistant" {
                            if let Some(usage) = msg["usage"].as_object() {
                                metrics.input_tokens +=
                                    usage.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
                                metrics.output_tokens +=
                                    usage.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
                                metrics.cache_read_tokens +=
                                    usage.get("cacheRead").and_then(|v| v.as_u64()).unwrap_or(0);
                                metrics.cache_write_tokens += usage
                                    .get("cacheWrite")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                metrics.total_tokens += usage
                                    .get("totalTokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                if let Some(cost) =
                                    usage.get("cost").and_then(|c| c["total"].as_f64())
                                {
                                    metrics.total_cost += cost;
                                }
                            }
                            if let Some(content_arr) = msg["content"].as_array() {
                                for item in content_arr {
                                    match item["type"].as_str() {
                                        Some("toolCall") => {
                                            if let Some(name) = item["name"].as_str() {
                                                *tool_counts
                                                    .entry(name.to_string())
                                                    .or_insert(0) += 1;
                                                last_tool_name = Some(name.to_string());
                                                let detail = item["input"]
                                                    .as_object()
                                                    .map(|obj| {
                                                        obj.iter()
                                                            .map(|(k, v)| {
                                                                let val_str = match v.as_str() {
                                                                    Some(s) => truncate_str(s, 300),
                                                                    None => {
                                                                        let j = v.to_string();
                                                                        truncate_str(&j, 100)
                                                                    }
                                                                };
                                                                format!("{}: {}", k, val_str)
                                                            })
                                                            .collect::<Vec<_>>()
                                                            .join("\n")
                                                    })
                                                    .filter(|s| !s.is_empty());
                                                recent_actions.push(RecentAction {
                                                    action_type: "tool".to_string(),
                                                    summary: name.to_string(),
                                                    detail,
                                                    timestamp: current_msg_timestamp.clone(),
                                                });
                                            }
                                        }
                                        Some("text") => {
                                            if let Some(text) = item["text"].as_str() {
                                                let trimmed = text.trim();
                                                if !trimmed.is_empty() {
                                                    let summary = truncate_str(trimmed, 60);
                                                    let detail = if trimmed.len() > 60 {
                                                        Some(truncate_str(trimmed, 500))
                                                    } else {
                                                        None
                                                    };
                                                    recent_actions.push(RecentAction {
                                                        action_type: "text".to_string(),
                                                        summary,
                                                        detail,
                                                        timestamp: current_msg_timestamp.clone(),
                                                    });
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            metrics.message_count += 1;
                        }
                    }
                    "custom"
                        if val["customType"]
                            .as_str()
                            .is_some_and(|t| t.contains("error")) =>
                    {
                        metrics.error_count += 1;
                    }
                    _ => {}
                }
            }

            metrics.current_task = last_user_text;
            metrics.current_tool = last_tool_name;
            metrics.last_activity = last_timestamp;

            let len = recent_actions.len();
            if len > 3 {
                metrics.recent_actions = recent_actions[len - 3..].to_vec();
            } else {
                metrics.recent_actions = recent_actions;
            }
            metrics.recent_actions.reverse();

            let mut tool_vec: Vec<ToolCallStat> = tool_counts
                .into_iter()
                .map(|(name, count)| ToolCallStat { name, count })
                .collect();
            tool_vec.sort_by_key(|t| std::cmp::Reverse(t.count));
            metrics.tool_calls = tool_vec;

            log::info!("[get_agent_metrics] SSH result: active={} recent_actions={} tool_calls={} message_count={} current_task={:?}",
                metrics.active, metrics.recent_actions.len(), metrics.tool_calls.len(), metrics.message_count, metrics.current_task);
            return Ok(metrics);
        }
        // Gateway API fallback
        let url = url.as_deref().unwrap_or("");
        let tok = token.as_deref().unwrap_or("");
        let result = invoke_tool(
            url,
            tok,
            "sessions_list",
            serde_json::json!({"agentId": agent_id, "activeMinutes": 60}),
        )
        .await?;
        let sessions = extract_sessions(&result);
        let active_count = sessions.iter().filter(|s| is_session_active(s)).count();
        let total_tokens: u64 = sessions
            .iter()
            .map(|s| s["totalTokens"].as_u64().unwrap_or(0))
            .sum();
        let model = sessions
            .iter()
            .find_map(|s| s["model"].as_str().map(|s| s.to_string()));
        let channel = sessions
            .iter()
            .find_map(|s| s["channel"].as_str().map(|s| s.to_string()));
        let last_updated = sessions
            .iter()
            .filter_map(|s| s["updatedAt"].as_u64())
            .max();
        let last_activity = last_updated.map(|ms| {
            let secs = (ms / 1000) as i64;
            chrono::DateTime::from_timestamp(secs, 0)
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                .unwrap_or_default()
        });
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut current_task: Option<String> = None;
        let default_key = format!("agent:{}:main", agent_id);
        let session_key = sessions
            .first()
            .and_then(|s| s["key"].as_str())
            .unwrap_or(&default_key);
        if let Ok(status_result) = invoke_tool(
            url,
            tok,
            "session_status",
            serde_json::json!({"sessionKey": session_key}),
        )
        .await
        {
            let sr = status_result.get("result").unwrap_or(&status_result);
            let det = sr.get("details").unwrap_or(sr);
            if let Some(text) = det["statusText"].as_str() {
                for line in text.lines() {
                    if line.contains("Tokens:") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        for (i, p) in parts.iter().enumerate() {
                            if *p == "in" && i > 0 {
                                input_tokens = parts[i - 1]
                                    .replace(",", "")
                                    .replace("k", "000")
                                    .parse()
                                    .unwrap_or(0);
                            }
                            if *p == "out" && i > 0 {
                                output_tokens = parts[i - 1]
                                    .replace(",", "")
                                    .replace("k", "000")
                                    .parse()
                                    .unwrap_or(0);
                            }
                        }
                    }
                    if line.contains("Queue:") {
                        let queue_part = line.split("Queue:").nth(1).unwrap_or("").trim();
                        if queue_part.starts_with("running")
                            || queue_part.starts_with("thinking")
                            || queue_part.starts_with("streaming")
                        {
                            current_task = Some(queue_part.to_string());
                        }
                    }
                }
            }
        }
        let metrics = AgentMetrics {
            agent_id: agent_id.clone(),
            active: active_count > 0,
            current_model: model,
            thinking_level: None,
            active_session_count: active_count,
            current_task,
            current_tool: None,
            total_tokens,
            input_tokens,
            output_tokens,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            total_cost: 0.0,
            tool_calls: vec![],
            recent_actions: vec![],
            error_count: 0,
            message_count: sessions.len(),
            session_start: None,
            last_activity,
            channel,
        };
        return Ok(metrics);
    }

    // === local mode (original) ===
    let active_set = lsof_active_agents().await;
    let agent_dir = if agent_id.is_empty() {
        "main"
    } else {
        &agent_id
    };
    let active = active_set.contains(agent_dir);

    let mut metrics = AgentMetrics {
        agent_id: agent_id.clone(),
        active,
        current_model: None,
        thinking_level: None,
        active_session_count: 0,
        current_task: None,
        current_tool: None,
        total_tokens: 0,
        input_tokens: 0,
        output_tokens: 0,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        total_cost: 0.0,
        tool_calls: vec![],
        recent_actions: vec![],
        error_count: 0,
        message_count: 0,
        session_start: None,
        last_activity: None,
        channel: None,
    };

    // Read sessions.json to find active sessions
    let sess_path = sessions_json_path(&agent_id);
    let sess_map: serde_json::Map<String, serde_json::Value> =
        match tokio::fs::read_to_string(&sess_path).await {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => return Ok(metrics),
        };

    metrics.active_session_count = sess_map.len();

    // Get model + channel from most recently updated session in sessions.json
    let best_entry = sess_map
        .values()
        .max_by_key(|v| v["updatedAt"].as_u64().unwrap_or(0));
    if let Some(entry) = best_entry {
        metrics.channel = entry["origin"]["surface"].as_str().map(|s| s.to_string());
        // Model is stored directly in sessions.json
        if metrics.current_model.is_none() {
            metrics.current_model = entry["model"].as_str().map(|s| s.to_string());
        }
    }

    // Find the most recently updated session file
    let mut best_session: Option<(String, u64)> = None;
    for val in sess_map.values() {
        if let (Some(file), Some(updated)) =
            (val["sessionFile"].as_str(), val["updatedAt"].as_u64())
        {
            if best_session.as_ref().map_or(true, |(_, t)| updated > *t) {
                best_session = Some((file.to_string(), updated));
            }
        }
    }

    let session_file = match best_session {
        Some((f, _)) => f,
        None => return Ok(metrics),
    };

    // Parse the .jsonl file
    let content = match tokio::fs::read_to_string(&session_file).await {
        Ok(c) => c,
        Err(_) => return Ok(metrics),
    };

    let mut tool_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut last_user_text: Option<String> = None;
    let mut last_tool_name: Option<String> = None;
    let mut last_timestamp: Option<String> = None;
    let mut recent_actions: Vec<RecentAction> = vec![];
    #[allow(unused_assignments)]
    let mut current_msg_timestamp: Option<String> = None;

    for line in content.lines() {
        let val: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event_type = val["type"].as_str().unwrap_or("");
        if let Some(ts) = val["timestamp"].as_str() {
            last_timestamp = Some(ts.to_string());
        }

        match event_type {
            "session" => {
                metrics.session_start = val["timestamp"].as_str().map(|s| s.to_string());
            }
            "model_change" => {
                metrics.current_model = val["modelId"].as_str().map(|s| s.to_string());
            }
            "thinking_level_change" => {
                metrics.thinking_level = val["thinkingLevel"].as_str().map(|s| s.to_string());
            }
            "message" => {
                let msg = &val["message"];
                let role = msg["role"].as_str().unwrap_or("");
                current_msg_timestamp = val["timestamp"].as_str().map(|s| s.to_string());

                if role == "user" {
                    if let Some(content_arr) = msg["content"].as_array() {
                        for item in content_arr {
                            if item["type"].as_str() == Some("text") {
                                if let Some(text) = item["text"].as_str() {
                                    last_user_text = extract_user_message(text);
                                }
                            }
                        }
                    }
                    metrics.message_count += 1;
                } else if role == "assistant" {
                    // Extract usage
                    if let Some(usage) = msg["usage"].as_object() {
                        metrics.input_tokens +=
                            usage.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
                        metrics.output_tokens +=
                            usage.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
                        metrics.cache_read_tokens +=
                            usage.get("cacheRead").and_then(|v| v.as_u64()).unwrap_or(0);
                        metrics.cache_write_tokens += usage
                            .get("cacheWrite")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        metrics.total_tokens += usage
                            .get("totalTokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        if let Some(cost) = usage.get("cost").and_then(|c| c["total"].as_f64()) {
                            metrics.total_cost += cost;
                        }
                    }

                    // Extract tool calls and text actions
                    if let Some(content_arr) = msg["content"].as_array() {
                        for item in content_arr {
                            match item["type"].as_str() {
                                Some("toolCall") => {
                                    if let Some(name) = item["name"].as_str() {
                                        *tool_counts.entry(name.to_string()).or_insert(0) += 1;
                                        last_tool_name = Some(name.to_string());

                                        let detail = item["input"]
                                            .as_object()
                                            .map(|obj| {
                                                let mut parts: Vec<String> = vec![];
                                                for (k, v) in obj.iter() {
                                                    let val_str = match v.as_str() {
                                                        Some(s) => {
                                                            if s.len() > 300 {
                                                                let mut end = 300;
                                                                while end > 0
                                                                    && !s.is_char_boundary(end)
                                                                {
                                                                    end -= 1;
                                                                }
                                                                format!("{}...", &s[..end])
                                                            } else {
                                                                s.to_string()
                                                            }
                                                        }
                                                        None => {
                                                            let j = v.to_string();
                                                            if j.len() > 100 {
                                                                let mut end = 100;
                                                                while end > 0
                                                                    && !j.is_char_boundary(end)
                                                                {
                                                                    end -= 1;
                                                                }
                                                                format!("{}...", &j[..end])
                                                            } else {
                                                                j
                                                            }
                                                        }
                                                    };
                                                    parts.push(format!("{}: {}", k, val_str));
                                                }
                                                parts.join("\n")
                                            })
                                            .filter(|s| !s.is_empty());
                                        recent_actions.push(RecentAction {
                                            action_type: "tool".to_string(),
                                            summary: name.to_string(),
                                            detail,
                                            timestamp: current_msg_timestamp.clone(),
                                        });
                                    }
                                }
                                Some("text") => {
                                    if let Some(text) = item["text"].as_str() {
                                        let trimmed = text.trim();
                                        if !trimmed.is_empty() {
                                            let summary = if trimmed.len() > 60 {
                                                let mut end = 60;
                                                while end > 0 && !trimmed.is_char_boundary(end) {
                                                    end -= 1;
                                                }
                                                format!("{}...", &trimmed[..end])
                                            } else {
                                                trimmed.to_string()
                                            };
                                            let detail = if trimmed.len() > 60 {
                                                let full = if trimmed.len() > 500 {
                                                    let mut end = 500;
                                                    while end > 0 && !trimmed.is_char_boundary(end)
                                                    {
                                                        end -= 1;
                                                    }
                                                    format!("{}...", &trimmed[..end])
                                                } else {
                                                    trimmed.to_string()
                                                };
                                                Some(full)
                                            } else {
                                                None
                                            };
                                            recent_actions.push(RecentAction {
                                                action_type: "text".to_string(),
                                                summary,
                                                detail,
                                                timestamp: current_msg_timestamp.clone(),
                                            });
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    metrics.message_count += 1;
                }
            }
            "custom"
                if val["customType"]
                    .as_str()
                    .is_some_and(|t| t.contains("error")) =>
            {
                metrics.error_count += 1;
            }
            _ => {}
        }
    }

    metrics.current_task = last_user_text;
    metrics.current_tool = last_tool_name;
    metrics.last_activity = last_timestamp;

    // Keep only the last 3 actions (most recent first)
    let len = recent_actions.len();
    if len > 3 {
        metrics.recent_actions = recent_actions[len - 3..].to_vec();
    } else {
        metrics.recent_actions = recent_actions;
    }
    metrics.recent_actions.reverse();

    // Sort tool calls by count desc
    let mut tool_vec: Vec<ToolCallStat> = tool_counts
        .into_iter()
        .map(|(name, count)| ToolCallStat { name, count })
        .collect();
    tool_vec.sort_by_key(|t| std::cmp::Reverse(t.count));
    metrics.tool_calls = tool_vec;

    Ok(metrics)
}

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

    // 3. WebSocket: wait for challenge → send connect → send chat.abort
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

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DailyCount {
    date: String,
    count: u32,
    tokens: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AgentExtraInfo {
    skills: Vec<String>,
    cron_jobs: Vec<serde_json::Value>,
    daily_counts: Vec<DailyCount>,
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

    // 3. Daily call counts + token usage — last 14 days from .jsonl files
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
