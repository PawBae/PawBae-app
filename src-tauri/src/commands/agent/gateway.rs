//! Gateway status, liveness check, and chat send commands.

use crate::agent_gateway::sessions_json_path;
use crate::lsof::lsof_active_agents;
use crate::state::{ActiveAgentPid, SessionInfo};

#[cfg(target_os = "windows")]
use crate::platform::windows::hide_window_tokio_cmd;

use super::GatewayStatus;

/// Check whether the local OpenClaw gateway process is alive.
///
/// OpenClaw uses a lock file at `$TMPDIR/openclaw-<uid>/gateway.<hash>.lock`
/// containing `{"pid": <n>, ...}`. When the gateway shuts down it deletes the
/// lock file. If the file is missing or the PID inside is no longer running,
/// the gateway is considered dead — meaning any "active" session state left in
/// the JSONL files is stale and should be forced to inactive.
pub(super) fn is_openclaw_gateway_alive() -> bool {
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
        Err(_) => return false, // no lock dir -> gateway not running
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

    // Step 3: read sessions.json -> session list
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

    // Try to parse JSON from stdout first -- exit code may be non-zero due to config warnings
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

    // No usable JSON -- treat as real failure
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("openclaw agent failed: {}", stderr));
    }

    Ok(String::new())
}
