use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

mod state;
use crate::state::*;

mod platform;
#[cfg(target_os = "macos")]
#[allow(unused_imports)]
use crate::platform::macos::*;
#[cfg(target_os = "windows")]
#[allow(unused_imports)]
use crate::platform::windows::*;
#[allow(unused_imports)]
use crate::platform::common::*;

mod commands;
#[allow(unused_imports)]
use crate::commands::*;
use crate::commands::codex_pet::codex_pets_dir;
use crate::commands::hook::process_claude_event;

#[cfg(target_os = "macos")]
mod speech;

use percent_encoding::percent_decode_str;
use std::sync::atomic::Ordering;

fn unix_now() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn ssh_backoff_remaining(host_key: &str) -> Option<u64> {
    let map = ssh_backoff_map().lock().unwrap();
    let state = map.get(host_key)?;
    if state.fail_count == 0 { return None; }
    let cooldown = std::cmp::min(15u64 * 2u64.pow(state.fail_count.saturating_sub(1)), 300);
    let elapsed = unix_now().saturating_sub(state.fail_epoch);
    if elapsed < cooldown { Some(cooldown - elapsed) } else { None }
}

fn ssh_backoff_record_failure(host_key: &str) {
    let mut map = ssh_backoff_map().lock().unwrap();
    let state = map.entry(host_key.to_string()).or_insert(SshBackoffState { fail_count: 0, fail_epoch: 0 });
    state.fail_count += 1;
    state.fail_epoch = unix_now();
}

pub(crate) fn ssh_backoff_reset(host_key: &str) {
    let mut map = ssh_backoff_map().lock().unwrap();
    map.remove(host_key);
}

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};
#[cfg(target_os = "macos")]
use tauri::menu::CheckMenuItem;
#[cfg(unix)]
use libc;




/// Fix PATH for macOS GUI apps which only get /usr/bin:/bin:/usr/sbin:/sbin.
/// openclaw is a Node.js script installed via pnpm, so both `openclaw` and `node`
/// must be reachable via PATH.
/// On Windows, GUI apps inherit the full user PATH, so no fix is needed.
fn fix_path() {
    #[cfg(target_os = "macos")]
    {
        for shell in ["/bin/zsh", "/bin/bash"] {
            if let Ok(output) = std::process::Command::new(shell)
                .args(["-lic", "echo $PATH"])
                .output()
            {
                if output.status.success() {
                    let shell_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !shell_path.is_empty() {
                        std::env::set_var("PATH", &shell_path);
                        log::info!("[fix_path] PATH set to: {}", &shell_path);
                        return;
                    }
                }
            }
        }
        log::warn!("[fix_path] could not get PATH from login shell");
    }
    #[cfg(target_os = "windows")]
    {
        // Windows GUI apps inherit the full user/system PATH from the registry.
        // No fix needed — openclaw and node should be reachable if installed.
        log::info!("[fix_path] Windows: using inherited PATH");
    }
}



/// Get the user home directory string in a cross-platform way.
pub(crate) fn home_dir_string() -> String {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| {
            #[cfg(unix)]
            { std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()) }
            #[cfg(windows)]
            { std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".into()) }
        })
}

/// Returns the full set of open .jsonl file paths across all agents.
/// On macOS/Linux: uses `lsof +D` to detect open files.
/// On Windows: falls back to checking file modification time (recent = active).
pub(crate) async fn lsof_open_jsonl_paths() -> std::collections::HashSet<String> {
    #[cfg(unix)]
    {
        let home = home_dir_string();
        let agents_dir = format!("{}/.openclaw/agents", home);
        let lsof_bin = if std::path::Path::new("/usr/sbin/lsof").exists() { "/usr/sbin/lsof" } else { "lsof" };
        let Ok(output) = tokio::process::Command::new(lsof_bin)
            .args(["+D", &agents_dir])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .await
        else { return std::collections::HashSet::new() };
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines()
            .filter(|l| l.contains(".jsonl"))
            .filter_map(|l| l.split_whitespace().last().map(|s| s.to_string()))
            .collect()
    }
    #[cfg(windows)]
    {
        // Windows fallback: find .jsonl files modified in the last 5 seconds
        // (indicates an active agent writing to them)
        let home = home_dir_string();
        let agents_dir = PathBuf::from(&home).join(".openclaw").join("agents");
        let mut result = std::collections::HashSet::new();
        let now = SystemTime::now();
        if let Ok(agents) = std::fs::read_dir(&agents_dir) {
            for agent_entry in agents.flatten() {
                let sessions_dir = agent_entry.path().join("sessions");
                if let Ok(files) = std::fs::read_dir(&sessions_dir) {
                    for file_entry in files.flatten() {
                        let path = file_entry.path();
                        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                            if let Ok(meta) = path.metadata() {
                                if let Ok(modified) = meta.modified() {
                                    if now.duration_since(modified).unwrap_or_default().as_secs() < 5 {
                                        result.insert(path.to_string_lossy().to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        result
    }
}


/// Single `lsof +D` over the entire agents dir → set of active agent directory names.
/// A .jsonl being held open by a process = that agent is working.
/// On Windows: uses file modification time heuristic instead of lsof.
pub(crate) async fn lsof_active_agents() -> std::collections::HashSet<String> {
    #[cfg(unix)]
    {
        let home = home_dir_string();
        let agents_dir = format!("{}/.openclaw/agents", home);
        let mut active = std::collections::HashSet::new();

        let lsof_bin = if std::path::Path::new("/usr/sbin/lsof").exists() {
            "/usr/sbin/lsof"
        } else {
            "lsof"
        };

        let Ok(output) = tokio::process::Command::new(lsof_bin)
            .args(["+D", &agents_dir])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .await
        else {
            return active;
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let prefix = ".openclaw/agents/";
        for line in stdout.lines() {
            if !line.contains(".jsonl") {
                continue;
            }
            if let Some(idx) = line.find(prefix) {
                let rest = &line[idx + prefix.len()..];
                if let Some(slash) = rest.find('/') {
                    active.insert(rest[..slash].to_string());
                }
            }
        }
        active
    }
    #[cfg(windows)]
    {
        // Windows: find agent directories that have recently modified .jsonl files
        let home = home_dir_string();
        let agents_dir = PathBuf::from(&home).join(".openclaw").join("agents");
        let mut active = std::collections::HashSet::new();
        let now = SystemTime::now();
        if let Ok(agents) = std::fs::read_dir(&agents_dir) {
            for agent_entry in agents.flatten() {
                let agent_name = agent_entry.file_name().to_string_lossy().to_string();
                let sessions_dir = agent_entry.path().join("sessions");
                if let Ok(files) = std::fs::read_dir(&sessions_dir) {
                    for file_entry in files.flatten() {
                        let path = file_entry.path();
                        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                            if let Ok(meta) = path.metadata() {
                                if let Ok(modified) = meta.modified() {
                                    if now.duration_since(modified).unwrap_or_default().as_secs() < 5 {
                                        active.insert(agent_name.clone());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        active
    }
}

/// Generic helper: call OpenClaw remote API via /tools/invoke
pub(crate) async fn invoke_tool(url: &str, token: &str, tool: &str, args: serde_json::Value) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/tools/invoke", url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({ "tool": tool, "args": args }))
        .send()
        .await
        .map_err(|e| format!("remote request failed: {}", e))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("remote API error ({}): {}", status, text));
    }
    serde_json::from_str(&text).map_err(|e| format!("parse remote response: {} body: {}", e, &text[..text.len().min(200)]))
}

/// Extract sessions array from remote API response, handling both formats:
/// - Old: { "result": [ ... ] }
/// - New (MCP): { "result": { "content": [...], "details": { "sessions": [...] } } }
pub(crate) fn extract_sessions(result: &serde_json::Value) -> Vec<serde_json::Value> {
    let r = result.get("result").unwrap_or(result);
    if let Some(sessions) = r.pointer("/details/sessions").and_then(|v| v.as_array()) {
        return sessions.clone();
    }
    if let Some(arr) = r.as_array() {
        return arr.clone();
    }
    vec![]
}

/// Check if a session is active (local mode fallback).
pub(crate) fn is_session_active(s: &serde_json::Value) -> bool {
    if let Some(active) = s.get("active").and_then(|v| v.as_bool()) {
        return active;
    }
    if let Some(updated_at) = s.get("updatedAt").and_then(|v| v.as_u64()) {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        return now_ms.saturating_sub(updated_at) < 3_000;
    }
    false
}

/// Check Queue status from session_status statusText.
fn is_queue_active(status_text: &str) -> bool {
    status_text.lines().any(|line| {
        if let Some(q) = line.split("Queue:").nth(1) {
            let q = q.trim();
            !q.starts_with("collect") && !q.starts_with("idle") && !q.starts_with("waiting")
        } else {
            false
        }
    })
}

/// Remote activity detection: Queue active (instant) OR updatedAt within 3s (smooth stop).
pub(crate) async fn is_remote_session_active(url: &str, token: &str, session_key: &str, s: &serde_json::Value) -> bool {
    if let Ok(status) = invoke_tool(url, token, "session_status", serde_json::json!({"sessionKey": session_key})).await {
        let sr = status.get("result").unwrap_or(&status);
        let det = sr.get("details").unwrap_or(sr);
        if let Some(text) = det["statusText"].as_str() {
            if is_queue_active(text) {
                return true;
            }
        }
    }
    // Queue says idle — use updatedAt as a brief buffer for smooth transition
    is_session_active(s)
}

/// Parse tail lines of a session .jsonl to determine if an agent is active.
///
/// OpenClaw JSONL format: each line is `{"type":"message","message":{...}}`
/// Key fields on `message`:
///   - `role`: "user" | "assistant" | "toolResult"
///   - `usage`: present (object) when an API call is complete
///   - `content`: array of `{type: "text"|"toolCall"|"thinking"|"image", ...}`
///   - NOTE: stop_reason is NOT present in OpenClaw JSONL
///
/// A single turn may involve multiple API calls (tool use loop):
///   1. user message          content=['text']           ← user prompt
///   2. assistant message     content=['toolCall']       ← calls a tool, NOT done
///   3. toolResult message    content=['text']           ← tool output
///   4. assistant message     content=['toolCall']       ← calls another tool, still NOT done
///   5. toolResult message    content=['text']           ← tool output
///   6. assistant message     content=['text']           ← final reply, turn done
///
/// Between steps 2→3 and 4→5 the queue briefly goes idle, but the turn is NOT over.
/// We check: if the last assistant message has "toolCall" content, the turn continues.
/// Also: if the last message is "toolResult", the agent is about to process it → active.
/// This affects: pet working/idle animation, completion sound, session active indicator.
pub(crate) fn check_agent_active_from_lines(lines: &[String]) -> bool {
    let mut last_role = String::new();
    let mut has_usage = false;
    let mut has_tool_call = false;
    for line in lines.iter().rev() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            if val["type"].as_str() == Some("message") {
                last_role = val["message"]["role"].as_str().unwrap_or("").to_string();
                has_usage = val["message"]["usage"].is_object();
                // Check if assistant message contains a toolCall content block
                if let Some(content) = val["message"]["content"].as_array() {
                    has_tool_call = content.iter().any(|c| c["type"].as_str() == Some("toolCall"));
                }
                break;
            }
        }
    }
    // Active when:
    //   - last msg is "user" → waiting for assistant response
    //   - last msg is "toolResult" → agent will process tool output next
    //   - last msg is "assistant" without usage → still streaming
    //   - last msg is "assistant" with toolCall content → called a tool, turn continues
    // Inactive when:
    //   - last msg is "assistant" with usage, no toolCall → turn truly ended
    last_role == "user"
        || last_role == "toolResult"
        || (last_role == "assistant" && (!has_usage || has_tool_call))
}

/// Build AgentHealth with session-level data from sessions.json + tail outputs.
pub(crate) fn build_agent_health_from_meta(
    agent_id: &str,
    meta_json: &str,
    tails: &std::collections::HashMap<String, Vec<String>>,
) -> AgentHealth {
    let mut sessions = Vec::new();
    let mut any_active = false;

    if let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(meta_json) {
        for (key, val) in map.iter() {
            let sf = val["sessionFile"].as_str().unwrap_or("");
            if sf.is_empty() { continue; }
            // Match session file path to tail output by basename
            #[cfg(windows)]
            let basename = sf.rsplit(|c: char| c == '/' || c == '\\').next().unwrap_or("");
            #[cfg(not(windows))]
            let basename = sf.rsplit('/').next().unwrap_or("");
            let active = if let Some(lines) = tails.get(basename) {
                check_agent_active_from_lines(lines)
            } else {
                false
            };
            if active { any_active = true; }
            sessions.push(SessionHealth { key: key.clone(), active });
        }
    }

    // Fallback: no sessions.json or parse failed — check all tails directly (v1.3.3 behavior)
    if sessions.is_empty() && !tails.is_empty() {
        for (fname, lines) in tails {
            let active = check_agent_active_from_lines(lines);
            if active { any_active = true; }
            // Use filename (without .jsonl) as session key
            let key = fname.strip_suffix(".jsonl").unwrap_or(fname).to_string();
            sessions.push(SessionHealth { key, active });
        }
    }

    AgentHealth { agent_id: agent_id.to_string(), active: any_active, sessions }
}

/// Get the SSH control socket path for a given host.
/// On macOS/Linux: /tmp/pawbae-ssh-user@host:22
/// On Windows: returns a path in %TEMP% (used only as a "marker" since ControlMaster
/// is not supported; the marker file tracks whether a connection was recently validated).
fn ssh_control_path(ssh_user: &str, ssh_host: &str) -> String {
    #[cfg(unix)]
    { format!("/tmp/pawbae-ssh-{}@{}:22", ssh_user, ssh_host) }
    #[cfg(windows)]
    {
        let temp = std::env::temp_dir();
        temp.join(format!("pawbae-ssh-{}@{}.marker", ssh_user, ssh_host))
            .to_string_lossy().to_string()
    }
}

/// Ensure an SSH ControlMaster socket is established (called once, reused by all ssh_exec).
/// On Windows, ControlMaster is not available — we just validate the connection once
/// and create a marker file. Each ssh_exec call will open its own SSH connection.
/// Implements exponential backoff on connection failure (15s, 30s, 60s, … capped at 300s)
/// to avoid flooding the server with reconnection attempts.
async fn ensure_ssh_master(ssh_host: &str, ssh_user: &str) -> Result<(), String> {
    let host_key = format!("{}@{}", ssh_user, ssh_host);
    if let Some(remaining) = ssh_backoff_remaining(&host_key) {
        return Err(format!("SSH connection to {} backing off, retry in {}s", host_key, remaining));
    }

    let control_path = ssh_control_path(ssh_user, ssh_host);
    // Fast path: socket/marker already exists, reuse the master connection.
    if std::path::Path::new(&control_path).exists() { return Ok(()); }

    // Per-host lock so only one task establishes the master at a time.
    use std::sync::OnceLock;
    use tokio::sync::Mutex as TokioMutex;
    static LOCKS: OnceLock<Mutex<HashMap<String, std::sync::Arc<TokioMutex<()>>>>> = OnceLock::new();
    let lock = {
        let mut locks = LOCKS.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap();
        locks.entry(host_key.clone()).or_insert_with(|| Arc::new(TokioMutex::new(()))).clone()
    };
    let _guard = lock.lock().await;
    // Re-check after acquiring the lock
    if std::path::Path::new(&control_path).exists() { return Ok(()); }

    #[cfg(unix)]
    {
        let cp = format!("ControlPath={}", control_path);
        let child = tokio::process::Command::new("ssh")
            .args([
                "-o", "StrictHostKeyChecking=no",
                "-o", "BatchMode=yes",
                "-o", "ConnectTimeout=10",
                "-o", "ControlMaster=yes",
                "-o", &cp,
                "-o", "ControlPersist=600",
                "-o", "ServerAliveInterval=15",
                "-o", "ServerAliveCountMax=3",
                "-fN",
                &host_key,
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("ssh master spawn: {}", e))?;

        let child_id = child.id();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            child.wait_with_output(),
        ).await;

        let output = match result {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => {
                ssh_backoff_record_failure(&host_key);
                return Err(format!("ssh master wait: {}", e));
            }
            Err(_) => {
                if let Some(pid) = child_id {
                    unsafe { libc::kill(pid as i32, libc::SIGKILL); }
                }
                ssh_backoff_record_failure(&host_key);
                return Err(format!("ssh master to {} timed out after 15s", host_key));
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            ssh_backoff_record_failure(&host_key);
            let count = ssh_backoff_map().lock().unwrap().get(&host_key).map(|s| s.fail_count).unwrap_or(0);
            let code = output.status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".into());
            log::warn!("[ssh] connection to {} failed (attempt {}), entering backoff", host_key, count);
            return Err(format!("SSH master failed [exit {}]: {}", code, stderr));
        }

        // Wait for the socket file to appear
        for _ in 0..30 {
            if std::path::Path::new(&control_path).exists() { break; }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        if !std::path::Path::new(&control_path).exists() {
            ssh_backoff_record_failure(&host_key);
            return Err(format!("ssh master socket for {} never appeared", host_key));
        }
    }

    #[cfg(windows)]
    {
        // Windows: use persistent SSH subprocess multiplexer instead of per-command
        // connections. This avoids the TCP+SSH handshake overhead on every call and
        // prevents hitting server-side MaxStartups limits.
        if let Err(e) = win_ssh_mux::ensure(ssh_user, ssh_host).await {
            ssh_backoff_record_failure(&host_key);
            let count = ssh_backoff_map().lock().unwrap().get(&host_key).map(|s| s.fail_count).unwrap_or(0);
            log::warn!("[ssh] connection to {} failed (attempt {}), entering backoff", host_key, count);
            return Err(format!("SSH connection failed: {}", e));
        }
        // Create marker file so the fast-path check at the top works.
        let _ = std::fs::write(&control_path, "connected");
    }

    // Detect which key was used by querying ssh config for this host.
    let mut ssh_g_cmd = tokio::process::Command::new("ssh");
    ssh_g_cmd.args(["-G", &host_key])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    hide_window_tokio_cmd(&mut ssh_g_cmd);
    if let Ok(cfg_output) = ssh_g_cmd.output().await
    {
        let cfg = String::from_utf8_lossy(&cfg_output.stdout);
        for line in cfg.lines() {
            if let Some(path) = line.strip_prefix("identityfile ") {
                let expanded = path.replace("~", &home_dir_string());
                if std::path::Path::new(&expanded).exists() {
                    log::info!("[ssh] {} will use key: {}", host_key, expanded);
                    ssh_key_map().lock().unwrap().insert(host_key.clone(), expanded);
                    break;
                }
            }
        }
    }

    ssh_backoff_reset(&host_key);
    Ok(())
}

/// Execute a command on remote host via SSH.
/// On macOS/Linux: reuses ControlMaster socket for fast multiplexed connections.
/// On Windows: routes through a persistent SSH subprocess (win_ssh_mux) so all
///   commands share a single TCP connection instead of opening one per call.
/// If the command fails (e.g. stale socket), removes the socket and retries once.
pub(crate) async fn ssh_exec(ssh_host: &str, ssh_user: &str, cmd: &str) -> Result<String, String> {
    ensure_ssh_master(ssh_host, ssh_user).await?;
    let safe_cmd = format!(
        "export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:$PATH && {}",
        cmd
    );

    #[cfg(windows)]
    {
        match win_ssh_mux::exec(ssh_user, ssh_host, &safe_cmd).await {
            Ok(out) => return Ok(out),
            Err(e) if e.contains("transport error") || e.contains("connection lost") || e.contains("process exited") || e.contains("not connected") || e.contains("timed out") => {
                log::warn!("[ssh] transport error, removing marker and retrying: {}", e);
                let _ = tokio::fs::remove_file(&ssh_control_path(ssh_user, ssh_host)).await;
                ensure_ssh_master(ssh_host, ssh_user).await?;
                return win_ssh_mux::exec(ssh_user, ssh_host, &safe_cmd).await;
            }
            Err(e) => return Err(e),
        }
    }

    #[cfg(unix)]
    {
        let target = format!("{}@{}", ssh_user, ssh_host);
        let control_path = ssh_control_path(ssh_user, ssh_host);
        let cp = format!("ControlPath={}", control_path);

        let mut ssh_args: Vec<&str> = vec![
            "-o", "BatchMode=yes",
            "-o", "ConnectTimeout=5",
            "-o", &cp,
        ];
        ssh_args.push(&target);
        ssh_args.push(&safe_cmd);

        let output = tokio::process::Command::new("ssh")
            .args(&ssh_args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
            .map_err(|e| format!("ssh: {}", e))?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }

        let exit_code = output.status.code().unwrap_or(-1);
        if exit_code != 255 {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut msg = format!("ssh cmd failed [exit {}]", exit_code);
            if !stderr.trim().is_empty() { msg.push_str(&format!("\nstderr: {}", stderr.trim())); }
            if !stdout.trim().is_empty() { msg.push_str(&format!("\nstdout: {}", stdout.trim())); }
            return Err(msg);
        }

        log::warn!("[ssh] transport error (exit 255), removing stale socket and retrying");
        let _ = tokio::fs::remove_file(&control_path).await;
        ensure_ssh_master(ssh_host, ssh_user).await?;

        let mut ssh_args2: Vec<&str> = vec![
            "-o", "BatchMode=yes",
            "-o", "ConnectTimeout=5",
            "-o", &cp,
        ];
        ssh_args2.push(&target);
        ssh_args2.push(&safe_cmd);

        let output = tokio::process::Command::new("ssh")
            .args(&ssh_args2)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
            .map_err(|e| format!("ssh: {}", e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let code = output.status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".into());
            let mut msg = format!("ssh cmd failed [exit {}]", code);
            if !stderr.trim().is_empty() { msg.push_str(&format!("\nstderr: {}", stderr.trim())); }
            if !stdout.trim().is_empty() { msg.push_str(&format!("\nstdout: {}", stdout.trim())); }
            return Err(msg);
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// Close an active SSH ControlMaster socket (macOS/Linux) or persistent mux subprocess (Windows).
pub(crate) async fn close_ssh_master(ssh_host: &str, ssh_user: &str) -> Result<(), String> {
    let control_path = ssh_control_path(ssh_user, ssh_host);
    #[cfg(unix)]
    {
        if std::path::Path::new(&control_path).exists() {
            let target = format!("{}@{}", ssh_user, ssh_host);
            let cp = format!("ControlPath={}", control_path);
            let _ = tokio::process::Command::new("ssh")
                .args(["-o", &cp, "-O", "exit", &target])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .await;
        }
    }
    #[cfg(windows)]
    {
        win_ssh_mux::kill(ssh_user, ssh_host).await;
    }
    let _ = tokio::fs::remove_file(&control_path).await;
    ssh_backoff_reset(&format!("{}@{}", ssh_user, ssh_host));
    log::info!("[close_ssh_master] closed socket for {}@{}", ssh_user, ssh_host);
    Ok(())
}

// Tray label tuple: (show, hide, stroll, quit). The `stroll` slot is
// populated for every language but only inserted into the tray menu on
// macOS — Phase 2 pet physics is currently macOS-only.
pub(crate) fn tray_labels(lang: &str) -> (&'static str, &'static str, &'static str, &'static str, &'static str) {
    match lang {
        "zh" => ("显示", "隐藏", "散步模式", "设置", "退出"),
        _ => ("Show", "Hide", "Stroll Mode", "Settings", "Quit"),
    }
}






pub(crate) async fn ssh_read_file(ssh_host: &str, ssh_user: &str, path: &str) -> Result<String, String> {
    // Use double quotes so ~ expands, but escape any embedded double quotes
    let escaped = path.replace('"', r#"\""#);
    ssh_exec(ssh_host, ssh_user, &format!("cat \"{}\"", escaped)).await
}

/// Check if an agent is active by reading the tail of the latest .jsonl file via SSH.
/// If the last message-type entry is a user message (no assistant response yet), agent is working.
pub(crate) async fn ssh_is_agent_active(ssh_host: &str, ssh_user: &str, agent_id: &str) -> bool {
    let agent_dir = if agent_id.is_empty() { "main" } else { agent_id };
    // Read the last 5 lines of the newest .jsonl file
    let cmd = format!(
        "f=$(ls -t $HOME/.openclaw/agents/{}/sessions/*.jsonl 2>/dev/null | head -1); [ -f \"$f\" ] && tail -5 \"$f\"",
        agent_dir
    );
    let output = match ssh_exec(ssh_host, ssh_user, &cmd).await {
        Ok(s) => s,
        Err(_) => return false,
    };
    // Walk backwards through lines to find the last message entry
    let lines: Vec<String> = output.lines().map(|l| l.to_string()).collect();
    check_agent_active_from_lines(&lines)
}

/// Check if a specific session file is active by reading its tail.
async fn ssh_is_session_file_active(ssh_host: &str, ssh_user: &str, session_file: &str) -> bool {
    let escaped = session_file.replace('"', r#"\""#);
    let cmd = format!("tail -5 \"{}\" 2>/dev/null", escaped);
    let output = match ssh_exec(ssh_host, ssh_user, &cmd).await {
        Ok(s) => s,
        Err(_) => return false,
    };
    for line in output.lines().rev() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            if val["type"].as_str() == Some("message") {
                let role = val["message"]["role"].as_str().unwrap_or("");
                let has_usage = val["message"]["usage"].is_object();
                return role == "user" || (role == "assistant" && !has_usage);
            }
        }
    }
    false
}

pub(crate) fn remote_sessions_json_path(agent_id: &str) -> String {
    let agent_dir = if agent_id.is_empty() { "main" } else { agent_id };
    format!("$HOME/.openclaw/agents/{}/sessions/sessions.json", agent_dir)
}

pub(crate) fn sessions_json_path(agent_id: &str) -> PathBuf {
    let home = home_dir_string();
    let agent_dir = if agent_id.is_empty() { "main" } else { agent_id };
    PathBuf::from(home).join(".openclaw").join("agents").join(agent_dir).join("sessions").join("sessions.json")
}






/// Compute collapsed mascot x position based on side preference.
pub(crate) fn collapsed_x(sx: f64, sw: f64, win_w: f64, position: &str, notch_offset: f64) -> f64 {
    if position == "left" {
        sx + sw / 2.0 - notch_offset - win_w
    } else {
        sx + sw / 2.0 + notch_offset
    }
}

// Bumped from 60x45 so the codex sprite-pet (rendered at ~86x93 CSS px due
// to the MINI_SPRITE_DISPLAY_MULTIPLIER=2 used in Mini.tsx) fits entirely
// inside the native window. Without the extra room the sprite gets clipped
// at the bottom/right edges of the OS-level mascot window.
pub(crate) const COLLAPSED_MASCOT_BASE_W: f64 = 96.0;
pub(crate) const COLLAPSED_MASCOT_BASE_H: f64 = 96.0;
// Vertical inset applied to the default mascot position so the sprite is
// always rendered below the macOS menu bar / notch (or the equivalent top
// chrome on Windows). Covers both notched (~38pt) and non-notched (~24pt)
// menu bars with extra breathing room.
pub(crate) const MASCOT_TOP_INSET: f64 = 120.0;
const MASCOT_SCALE_MIN: f64 = 1.0;
const MASCOT_SCALE_MAX: f64 = 3.0;
pub(crate) const LARGE_MASCOT_SIZE_MULTIPLIER: f64 = 3.0;

pub(crate) fn sanitized_mascot_scale(scale: Option<f64>) -> f64 {
    let scale = scale.unwrap_or(1.0);
    if !scale.is_finite() {
        return 1.0;
    }
    scale.max(MASCOT_SCALE_MIN).min(MASCOT_SCALE_MAX)
}

pub(crate) fn collapsed_mascot_window_size(scale: f64) -> (f64, f64) {
    (COLLAPSED_MASCOT_BASE_W * scale, COLLAPSED_MASCOT_BASE_H * scale)
}

pub(crate) fn large_collapsed_mascot_window_size(scale: f64, large_scale: f64) -> (f64, f64) {
    let lms = if large_scale.is_finite() && large_scale >= 1.0 && large_scale <= 6.0 { large_scale } else { LARGE_MASCOT_SIZE_MULTIPLIER };
    let size = 43.0 * scale * lms;
    (size, size)
}




pub(crate) fn current_sprite_pad() -> SpritePadFracs {
    SPRITE_PAD.lock().map(|g| *g).unwrap_or(SpritePadFracs {
        top: 0.40,
        right: 0.45,
        bottom: 0.30,
        left: 0.45,
        top_px: None,
        right_px: None,
        bottom_px: None,
        left_px: None,
    })
}




/// Move the mini window by a delta (dx, dy in CSS/logical points).
/// dy is in screen coordinates (positive = downward), converted to macOS (positive = upward).
///
/// On macOS the resulting origin is clamped to the screen's `visibleFrame`
/// (menu-bar / Dock / notch excluded). This is the authoritative safety
/// net for the pet physics loop: even at terminal velocity or during a
/// hard drag-throw, the window can never end up past a wall.




/// Background polling loop for efficiency-mode hover.
/// Checks the cursor position against two regions:
///  - **Collapsed**: a wide strip around the notch (notch_off*2 + 200 px,
///    50 px tall at the top of the screen) — much wider than the actual
///    window so the user can approach from either side.
///  - **Expanded**: the panel area (500 × 400 px, top-center).
pub(crate) fn efficiency_hover_poll(app: tauri::AppHandle) {
    use std::time::{Duration, Instant};
    EFFICIENCY_HOVER_THREAD_ALIVE.store(true, Ordering::SeqCst);
    let mut was_inside = false;
    let mut was_over_mascot = false;
    let mut last_enter_emit = Instant::now();
    // Drag state machine, driven entirely by NSEvent.pressedMouseButtons +
    // NSEvent.mouseLocation. The webview cannot observe mouseDown on a
    // non-key floating window, so the JS-side drag would otherwise need a
    // priming click. We mirror codex's approach: poll cursor + button,
    // translate the mini NSWindow ourselves, and emit walk-dir events to
    // the frontend so the codex sprite shows run-left/run-right.
    let mut drag_active = false;
    let mut last_cursor: (f64, f64) = (0.0, 0.0);
    let mut last_walk_dir: i32 = 0;
    let mut was_pressed = false;
    // Used only for run-left/right detection — measured between successive
    // poll iterations. Window translation itself is anchor-based and lives
    // in request_drag_apply (which reads the live cursor on main thread).

    // Drag-throw velocity sampling buffer (Phase 2 pet physics).
    // Holds a sliding window of (timestamp, dx, dy_topdown) entries while
    // the user drags the mascot. On release we average the most recent
    // ~80 ms of samples to derive an initial velocity for the falling
    // animation. Disabled by default; enabled by the frontend through
    // `set_throw_tracking` once the user picks a physics-capable pet
    // and stroll-mode is on.
    // `Instant` is already in scope from the function-top
    // `use std::time::{Duration, Instant};`.
    use std::collections::VecDeque;
    let mut throw_samples: VecDeque<(Instant, f64, f64)> = VecDeque::with_capacity(32);
    // 250ms is a wider window than the typical 80ms peak-velocity grab.
    // Users instinctively settle the cursor for a beat before releasing,
    // so a tighter window often averages mostly-zero samples.
    const THROW_SAMPLE_CAP: usize = 24;
    const THROW_AVG_WINDOW_MS: u128 = 250;
    const MAX_THROW_SPEED: f64 = 30.0;

    while EFFICIENCY_HOVER_ACTIVE.load(Ordering::SeqCst) {
        let info = NOTCH_SCREEN_INFO.lock().ok().and_then(|g| *g);
        let sleep_ms = if let Some((sx, sy, sw, sh, notch_off)) = info {
            let cursor = macos_cursor_position();
            let buttons = macos_pressed_mouse_buttons();
            let left_pressed = (buttons & 1) != 0;
            let is_expanded = EFFICIENCY_EXPANDED.load(Ordering::SeqCst);
            let frame = MINI_WINDOW_FRAME.lock().ok().and_then(|g| *g);

            let inside = if is_expanded {
                if let Some((fx, fy, fw, fh)) = frame {
                    cursor.0 >= fx && cursor.0 <= fx + fw
                        && cursor.1 >= fy && cursor.1 <= fy + fh
                } else {
                    false
                }
            } else {
                let rw = (notch_off * 2.0 + 10.0).max(80.0);
                let rh = frame
                    .map(|(_, _, _, fh)| fh.clamp(20.0, 28.0))
                    .unwrap_or(35.0);
                let rx = sx + (sw - rw) / 2.0;
                let ry = sy + sh - rh;
                cursor.0 >= rx && cursor.0 <= rx + rw
                    && cursor.1 >= ry && cursor.1 <= ry + rh
            };

            if inside && !was_inside {
                let _ = app.emit("efficiency-hover", true);
                last_enter_emit = Instant::now();
            } else if inside && was_inside && last_enter_emit.elapsed() > Duration::from_millis(300) {
                let _ = app.emit("efficiency-hover", true);
                last_enter_emit = Instant::now();
            } else if !inside && was_inside {
                let _ = app.emit("efficiency-hover", false);
            }
            was_inside = inside;

            // ── Mascot body hit-test ──
            // Use a tighter rect than the full 96x96 window: the codex
            // 192x208 cell paints the character roughly in its centre with
            // transparent margins (and the status badge lives in the
            // bottom-right corner). Hover/drag should only fire on the
            // visible body, so we inset to ~35% wide x 65% tall around the
            // upper-centre where the head/torso sit.
            let over_mascot = if is_expanded {
                false
            } else if let Some((fx, fy, fw, fh)) = frame {
                let l = fx + fw * 0.32;
                let r = fx + fw * 0.68;
                let b = fy + fh * 0.25; // NSEvent y axis grows upward
                let t = fy + fh * 0.90;
                cursor.0 >= l && cursor.0 <= r && cursor.1 >= b && cursor.1 <= t
            } else {
                false
            };

            // ── Drag state machine ──
            // Only engage in collapsed (mascot) state, never in expanded
            // panel mode (clicks inside the panel must keep their normal
            // webview behavior).
            if !is_expanded {
                if drag_active {
                    if left_pressed {
                        // Always request a fresh window-snap; the main-thread
                        // task reads cursor position itself, so even if many
                        // requests collapse into one, the window still ends
                        // up under the live cursor.
                        request_drag_apply(&app);
                        let dx = cursor.0 - last_cursor.0;
                        // macOS NSEvent y axis is bottom-up; flip to
                        // top-down so the throw velocity matches the
                        // frontend physics convention.
                        let dy_topdown = -(cursor.1 - last_cursor.1);
                        last_cursor = cursor;
                        let walk_dir = if dx > 0.5 { 1 } else if dx < -0.5 { -1 } else { last_walk_dir };
                        if walk_dir != last_walk_dir {
                            let _ = app.emit("mini-mascot-walk", walk_dir);
                            last_walk_dir = walk_dir;
                        }
                        if THROW_TRACKING_ENABLED.load(Ordering::SeqCst) {
                            let now = Instant::now();
                            throw_samples.push_back((now, dx, dy_topdown));
                            while throw_samples.len() > THROW_SAMPLE_CAP {
                                throw_samples.pop_front();
                            }
                        }
                    } else {
                        // Drag finished. Clear anchor + walk dir and notify
                        // the frontend so it can persist the new origin.
                        drag_active = false;
                        if let Ok(mut a) = drag_anchor().lock() {
                            *a = None;
                        }
                        if last_walk_dir != 0 {
                            let _ = app.emit("mini-mascot-walk", 0i32);
                            last_walk_dir = 0;
                        }
                        // Compute drag-release velocity from the most
                        // recent ~80 ms of samples. Older samples are
                        // dropped so a long pause before release doesn't
                        // dilute the final velocity.
                        if THROW_TRACKING_ENABLED.load(Ordering::SeqCst) && !throw_samples.is_empty() {
                            let cutoff = Instant::now();
                            // Average only over samples where the cursor
                            // actually moved. Users typically pause for
                            // ~50–150ms before releasing, so naively
                            // averaging the last fixed window picks up
                            // mostly zero samples and the throw lands
                            // at zero velocity.
                            let mut sum_dx = 0.0;
                            let mut sum_dy = 0.0;
                            let mut count = 0u32;
                            let mut total_seen = 0u32;
                            for (t, dx, dy) in throw_samples.iter().rev() {
                                if cutoff.duration_since(*t).as_millis() > THROW_AVG_WINDOW_MS {
                                    break;
                                }
                                total_seen += 1;
                                if dx.abs() < 0.5 && dy.abs() < 0.5 {
                                    continue;
                                }
                                sum_dx += *dx;
                                sum_dy += *dy;
                                count += 1;
                            }
                            if count > 0 {
                                let avg_dx = sum_dx / count as f64;
                                let avg_dy = sum_dy / count as f64;
                                let vx = avg_dx.clamp(-MAX_THROW_SPEED, MAX_THROW_SPEED);
                                let vy = avg_dy.clamp(-MAX_THROW_SPEED, MAX_THROW_SPEED);
                                log::info!(
                                    "[drag-throw] samples={}/{} avg_dx={:.2} avg_dy={:.2} → vx={:.2} vy={:.2}",
                                    count, total_seen, avg_dx, avg_dy, vx, vy,
                                );
                                let _ = app.emit(
                                    "mini-mascot-drag-throw",
                                    serde_json::json!({ "vx": vx, "vy": vy }),
                                );
                            } else {
                                log::info!(
                                    "[drag-throw] all {} samples in {}ms window were near-zero",
                                    total_seen, THROW_AVG_WINDOW_MS,
                                );
                            }
                        }
                        throw_samples.clear();
                        let _ = app.emit("mini-mascot-drag-end", ());
                    }
                } else if over_mascot && left_pressed && !was_pressed {
                    drag_active = true;
                    last_cursor = cursor;
                    // Reset the velocity sampling buffer; previous
                    // samples (from the last drag) must not bleed into
                    // the new throw.
                    throw_samples.clear();
                    // Capture the cursor-to-origin offset at drag start so
                    // the main-thread task can place the window absolutely
                    // each frame instead of summing deltas.
                    if let Some((fx, fy, _, _)) = frame {
                        if let Ok(mut a) = drag_anchor().lock() {
                            *a = Some((cursor.0 - fx, cursor.1 - fy));
                        }
                    }
                    // Cancel any active hover so the sprite immediately
                    // switches from `jumping` to its base/run state when
                    // the drag begins.
                    if was_over_mascot {
                        let _ = app.emit("mini-mascot-hover", false);
                        was_over_mascot = false;
                    }
                    // Stroll-mode physics needs an explicit drag-start
                    // signal so it can suspend the gravity tick while
                    // the user holds the mascot. The existing
                    // mini-mascot-walk event only fires on horizontal
                    // motion, so a click-and-hold without lateral drag
                    // would otherwise leave physics running underneath.
                    log::info!(
                        "[drag-start] cursor=({:.1},{:.1}) tracking={}",
                        cursor.0, cursor.1, THROW_TRACKING_ENABLED.load(Ordering::SeqCst),
                    );
                    let _ = app.emit("mini-mascot-drag-start", ());
                }
            } else if drag_active {
                drag_active = false;
                throw_samples.clear();
                if let Ok(mut a) = drag_anchor().lock() {
                    *a = None;
                }
                if last_walk_dir != 0 {
                    let _ = app.emit("mini-mascot-walk", 0i32);
                    last_walk_dir = 0;
                }
            }
            was_pressed = left_pressed;

            // Hover signal is suppressed while dragging so the sprite
            // shows run-left/run-right instead of jumping.
            let hover_signal = over_mascot && !drag_active;
            if hover_signal != was_over_mascot {
                let _ = app.emit("mini-mascot-hover", hover_signal);
                was_over_mascot = hover_signal;
            }

            // Adaptive polling: fastest while dragging (60fps) so the
            // window keeps up with the cursor; slower when just hovering;
            // very slow when far from the mascot to save battery.
            if drag_active {
                16
            } else if is_expanded || inside || over_mascot {
                30
            } else {
                let screen_top = sy + sh;
                let dist_from_top = screen_top - cursor.1;
                let near_mascot = frame
                    .map(|(fx, fy, fw, fh)| {
                        cursor.0 >= fx - 80.0
                            && cursor.0 <= fx + fw + 80.0
                            && cursor.1 >= fy - 80.0
                            && cursor.1 <= fy + fh + 80.0
                    })
                    .unwrap_or(false);
                if near_mascot || dist_from_top < 200.0 {
                    50
                } else {
                    500
                }
            }
        } else {
            500
        };
        std::thread::sleep(Duration::from_millis(sleep_ms));
    }
    EFFICIENCY_HOVER_THREAD_ALIVE.store(false, Ordering::SeqCst);
}




#[cfg(not(target_os = "macos"))]
fn macos_cursor_position() -> (f64, f64) {
    (0.0, 0.0)
}

// Non-macOS stubs: the efficiency hover / notch drag tracker is a macOS-only
// feature (driven by NSEvent), but the polling loop itself is not gated, so
// we provide no-op implementations on other platforms to keep the build
// happy. The poll loop never engages drag here because `NOTCH_SCREEN_INFO`
// stays unset on Windows/Linux.
#[cfg(not(target_os = "macos"))]
fn macos_pressed_mouse_buttons() -> usize {
    0
}

#[cfg(not(target_os = "macos"))]
fn request_drag_apply(_app: &tauri::AppHandle) {}

/// Resize the expanded mini window height while keeping it top-aligned.
/// macOS: bottom-left origin, so adjust y to keep the same top anchor.
/// Windows: top-left origin, so just resize height.



#[derive(Debug, Clone, serde::Deserialize)]
struct CursorWindowMeta {
    port: u16,
    #[serde(default)]
    focused: bool,
    #[serde(default, rename = "workspaceName")]
    workspace_name: String,
    #[serde(default, rename = "workspaceRoots")]
    workspace_roots: Vec<String>,
    #[serde(default, rename = "nativeHandle")]
    native_handle: Option<String>,
}

#[derive(Debug, Clone)]
struct CursorWindowBinding {
    port: u16,
    workspace_root: String,
    workspace_name: String,
    native_handle: Option<String>,
}

/// Compute the JSONL session file path (matching notchi's ConversationParser.sessionFilePath)
fn claude_session_file_path(session_id: &str, cwd: &str) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_default();
    // On Windows, Claude Code replaces all of / \ : . with "-" when computing
    // the project directory name (e.g. G:\Desktop\code → G--Desktop-code).
    // The colon after the drive letter (G:) must also be replaced.
    #[cfg(windows)]
    let project_dir = cwd.replace('/', "-").replace('\\', "-").replace(':', "-").replace('.', "-");
    #[cfg(not(windows))]
    let project_dir = cwd.replace('/', "-").replace('.', "-");
    home.join(".claude").join("projects").join(project_dir).join(format!("{}.jsonl", session_id))
}

fn collect_jsonl_files_recursive(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !root.exists() {
        return out;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                out.push(path);
            }
        }
    }
    out
}

pub(crate) fn collect_claude_project_jsonl_files() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let claude_projects = home.join(".claude").join("projects");
    if !claude_projects.exists() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Ok(project_dirs) = std::fs::read_dir(claude_projects) {
        for project_entry in project_dirs.flatten() {
            let project_dir = project_entry.path();
            if !project_dir.is_dir() {
                continue;
            }
            if let Ok(files) = std::fs::read_dir(project_dir) {
                for file_entry in files.flatten() {
                    let path = file_entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                        out.push(path);
                    }
                }
            }
        }
    }
    out
}

pub(crate) fn collect_codex_session_jsonl_files() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let codex_sessions = home.join(".Codex").join("sessions");
    collect_jsonl_files_recursive(&codex_sessions)
}

fn find_claude_session_file(session_id: &str) -> Option<PathBuf> {
    let target = format!("{}.jsonl", session_id);
    for path in collect_claude_project_jsonl_files() {
        if path.file_name().and_then(|n| n.to_str()) == Some(target.as_str()) {
            return Some(path);
        }
    }
    None
}

fn find_codex_session_file(session_id: &str) -> Option<PathBuf> {
    // Codex stores sessions as:
    //   ~/.Codex/sessions/YYYY/MM/DD/rollout-<timestamp>-<session_id>.jsonl
    // so we cannot derive the path from cwd; we must scan for a filename
    // containing the session id.
    for path in collect_codex_session_jsonl_files() {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.ends_with(".jsonl") && name.contains(session_id) {
            return Some(path);
        }
    }
    None
}

pub(crate) fn resolve_session_jsonl_path(session_id: &str, cwd: Option<&str>) -> Option<PathBuf> {
    // Prefer Claude's deterministic path when cwd is known, then fall back to
    // directory scans. This keeps existing behavior fast while adding Codex
    // compatibility.
    if let Some(cwd_str) = cwd {
        if !cwd_str.is_empty() {
            let by_cwd = claude_session_file_path(session_id, cwd_str);
            if by_cwd.exists() {
                return Some(by_cwd);
            }
        }
    }
    find_claude_session_file(session_id).or_else(|| find_codex_session_file(session_id))
}

/// Check if a JSONL file indicates an interrupted session
fn check_interrupted(path: &std::path::Path) -> bool {
    if let Ok(content) = std::fs::read_to_string(path) {
        // Determine interruption from the latest meaningful event.
        // This supports both Claude and Codex transcript formats.
        for line in content.lines().rev().take(120) {
            // Codex: explicit turn abort marker.
            if line.contains("\"type\":\"event_msg\"") && line.contains("\"type\":\"turn_aborted\"") {
                return true;
            }
            // Codex: tool call rejected by user (skip/deny).
            if line.contains("\"type\":\"function_call_output\"") {
                if line.contains("rejected by user")
                    || line.contains("Rejected(\\\"rejected by user\\\")")
                {
                    return true;
                }
                // A non-rejection function output means older interruption markers
                // no longer represent current state.
                return false;
            }
            // Any newer user message supersedes older interruption markers.
            if line.contains("\"type\":\"event_msg\"") && line.contains("\"type\":\"user_message\"") {
                return false;
            }
            if line.contains("\"type\":\"user\"") {
                return line.contains("[Request interrupted by user")
                    || line.contains("<turn_aborted>");
            }
        }
    }
    false
}

/// Determine whether a Stop event represents a user-aborted turn rather than a
/// normal completion. Cursor stop hooks expose a completion status, while some
/// clients only reveal the interruption via transcript markers.
pub(crate) fn stop_event_was_interrupted(event: &serde_json::Value, session_source: &str, claude_status: &str) -> bool {
    let status = claude_status.trim().to_ascii_lowercase();
    if session_source == "cursor" {
        if status == "completed" {
            return false;
        }
        if matches!(status.as_str(), "interrupted" | "cancelled" | "canceled" | "aborted" | "stopped") {
            return true;
        }
    }

    let stop_message = event.get("lastResponse")
        .or_else(|| event.get("last_assistant_message"))
        .or_else(|| event.get("codex_last_assistant_message"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if stop_message.contains("[Request interrupted by user")
        || stop_message.contains("<turn_aborted>")
        || stop_message.contains("turn_aborted")
        || stop_message.contains("rejected by user")
    {
        return true;
    }

    event.get("transcript_path")
        .and_then(|v| v.as_str())
        .filter(|p| !p.is_empty())
        .map(|p| check_interrupted(std::path::Path::new(p)))
        .unwrap_or(false)
}

// --- Session File Watcher (matching notchi's NotchiStateMachine) ---
use notify::{Watcher, RecursiveMode};

/// Debounce interval matching notchi's syncDebounce (100ms)
const WATCHER_DEBOUNCE_MS: u64 = 200;

pub(crate) fn start_session_file_watcher(
    session_id: String,
    jsonl_path: PathBuf,
    sessions: Arc<Mutex<HashMap<String, ClaudeSession>>>,
    app: tauri::AppHandle,
) {
    // Stop existing watcher for this session
    stop_session_file_watcher(&session_id);

    let sid = session_id.clone();
    let path_for_handler = jsonl_path.clone();

    // Record initial file size (to detect compact truncation)
    let initial_size = std::fs::metadata(&jsonl_path).map(|m| m.len()).unwrap_or(0);
    let last_size = Arc::new(Mutex::new(initial_size));

    let watcher_result = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        if let Ok(event) = res {
            // Only care about modifications
            if !event.kind.is_modify() { return; }

            let sessions2 = sessions.clone();
            let app2 = app.clone();
            let sid2 = sid.clone();
            let path2 = path_for_handler.clone();
            let last_size2 = last_size.clone();

            // Debounce: spawn a thread that waits before processing
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(WATCHER_DEBOUNCE_MS));

                let new_size = std::fs::metadata(&path2).map(|m| m.len()).unwrap_or(0);
                let mut prev = last_size2.lock().unwrap();
                *prev = new_size;

                let mut sessions_guard = sessions2.lock().unwrap();
                let session = match sessions_guard.get_mut(&sid2) {
                    Some(s) => s,
                    None => return,
                };

                let mut changed = false;

                // Interruption detection: active/waiting but file shows interrupted
                if matches!(session.status.as_str(), "processing" | "tool_running" | "waiting") {
                    if check_interrupted(&path2) {
                        log::info!("File watcher: interrupted session {}", sid2);
                        session.status = "stopped".to_string();
                        session.is_processing = false;
                        session.tool = None;
                        session.tool_input = None;
                        session.permission_suggestions = None;
                        changed = true;
                    }
                }


                if changed {
                    session.updated_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
                    let _ = app2.emit("claude-session-update", &sid2);
                    // Don't emit claude-task-complete here — the Stop hook event
                    // already emits it. This avoids double sound playback.
                }
            });
        }
    });

    match watcher_result {
        Ok(mut watcher) => {
            if let Err(e) = watcher.watch(&jsonl_path, RecursiveMode::NonRecursive) {
                log::error!("Failed to watch session file {:?}: {}", jsonl_path, e);
                return;
            }
            log::info!("Started file watcher for session {} at {:?}", session_id, jsonl_path);
            SESSION_WATCHERS.lock().unwrap().insert(session_id, watcher);
        }
        Err(e) => {
            log::error!("Failed to create file watcher: {}", e);
        }
    }
}

pub(crate) fn stop_session_file_watcher(session_id: &str) {
    if let Some(_watcher) = SESSION_WATCHERS.lock().unwrap().remove(session_id) {
        log::info!("Stopped file watcher for session {}", session_id);
        // Watcher is dropped, which stops it
    }
}

/// Check whether a process with the given PID is still alive.
/// Uses kill(pid, 0) on Unix — a zero-cost syscall that checks existence
/// without sending any signal. On Windows, uses OpenProcess.
pub(crate) fn is_pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // kill(pid, 0) returns 0 if the process exists and we have permission
        // to signal it; returns -1 with ESRCH if the process doesn't exist.
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
        use windows::Win32::Foundation::CloseHandle;
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
            match handle {
                Ok(h) => { let _ = CloseHandle(h); true }
                Err(_) => false,
            }
        }
    }
}


#[cfg(not(target_os = "macos"))]
pub(crate) fn get_active_ghostty_terminal_id() -> Option<String> { None }


#[cfg(not(target_os = "macos"))]
pub(crate) fn get_frontmost_app_name() -> String { String::new() }

pub(crate) fn is_cursor_frontmost_app(name: &str) -> bool {
    name == "Cursor" || name == "pawbae-app"
}

pub(crate) fn is_codex_frontmost_app(name: &str) -> bool {
    if name == "pawbae-app" || name == "Code" || name == "Visual Studio Code" {
        return true;
    }
    let lowered = name.to_ascii_lowercase();
    lowered == "codex" || lowered.contains("codex")
}

pub(crate) fn is_codex_host_terminal(name: &str) -> bool {
    name == "Code" || name == "Visual Studio Code" || name.eq_ignore_ascii_case("codex")
}

/// Check if the frontmost app matches the host terminal name.
/// `host_terminal` comes from process-chain detection (e.g. "Terminal",
/// "iTerm2", "Warp") while `frontmost` is the short app name from
/// NSWorkspace (e.g. "Terminal", "iTerm2", "Warp").
/// Also handles "pawbae-app" (our own panel can steal focus).
pub(crate) fn frontmost_matches_host_terminal(frontmost: &str, host_terminal: &str) -> bool {
    if frontmost == "pawbae-app" {
        return true;
    }
    if frontmost.eq_ignore_ascii_case(host_terminal) {
        return true;
    }
    // macOS Terminal.app reports as "Terminal" in both NSWorkspace and ps
    if host_terminal == "Apple_Terminal" && frontmost == "Terminal" {
        return true;
    }
    false
}


#[tauri::command]
async fn resolve_claude_permission(
    session_id: String,
    decision: String,
    state: tauri::State<'_, ClaudeState>,
) -> Result<(), String> {
    let tool_name = {
        let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
        sessions.get(&session_id).and_then(|s| s.tool.clone())
    };

    let response_json = match decision.as_str() {
        "deny" => {
            serde_json::json!({
                "continue": true,
                "suppressOutput": true,
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": { "behavior": "deny" }
                }
            }).to_string()
        }
        "allow_once" => {
            serde_json::json!({
                "continue": true,
                "suppressOutput": true,
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": { "behavior": "allow" }
                }
            }).to_string()
        }
        "allow_all" => {
            let rules = if let Some(name) = &tool_name {
                serde_json::json!([{ "toolName": name }])
            } else {
                serde_json::json!([])
            };
            serde_json::json!({
                "continue": true,
                "suppressOutput": true,
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {
                        "behavior": "allow",
                        "updatedPermissions": [{
                            "type": "addRules",
                            "destination": "session",
                            "rules": rules,
                            "behavior": "allow"
                        }]
                    }
                }
            }).to_string()
        }
        "auto_approve" => {
            serde_json::json!({
                "continue": true,
                "suppressOutput": true,
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {
                        "behavior": "allow",
                        "updatedPermissions": [{
                            "type": "setMode",
                            "destination": "session",
                            "mode": "bypassPermissions"
                        }]
                    }
                }
            }).to_string()
        }
        _ => return Err(format!("Unknown decision: {}", decision)),
    };

    let tx = {
        let mut map = state.pending_permissions.lock().map_err(|e| e.to_string())?;
        map.remove(&session_id)
    };

    if let Some(tx) = tx {
        tx.send(response_json).map_err(|_| "Failed to send permission response".to_string())?;
        log::info!("[resolve_permission] sent '{}' for session={}", decision, &session_id[..session_id.len().min(8)]);
    } else {
        log::warn!("[resolve_permission] no pending permission for session={}", &session_id[..session_id.len().min(8)]);
    }

    Ok(())
}





pub(crate) fn reassert_mini_floating(app: &tauri::AppHandle) {
    use tauri::Manager;
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let win_clone = win.clone();
    let _ = app.run_on_main_thread(move || {
        #[cfg(target_os = "macos")]
        {
            use objc2::runtime::AnyObject;
            use objc2::msg_send;
            if let Ok(ns_win) = win_clone.ns_window() {
                let obj = unsafe { &*(ns_win as *mut AnyObject) };
                unsafe {
                    let _: () = msg_send![obj, setLevel: 27isize];
                    let behavior: usize = (1 << 0) | (1 << 4) | (1 << 8) | (1 << 6);
                    let _: () = msg_send![obj, setCollectionBehavior: behavior];
                }
            }
        }
        let _ = win_clone.set_always_on_top(true);
    });
}

pub(crate) fn cwd_matches_workspace_root(cwd: &str, workspace_root: &str) -> bool {
    if cwd.is_empty() || workspace_root.is_empty() {
        return false;
    }
    if cwd == workspace_root {
        return true;
    }
    cwd.strip_prefix(workspace_root)
        .map(|rest| rest.starts_with('/') || rest.starts_with('\\'))
        .unwrap_or(false)
}

fn cursor_workspace_name_from_path(path_str: &str) -> String {
    std::path::Path::new(path_str)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string()
}

fn read_local_http_response(port: u16, request: String) -> Option<(u16, String)> {
    use std::io::{Read, Write};
    use std::net::{Shutdown, SocketAddr, TcpStream};

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(120))
        .ok()?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(200)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(200)));
    stream.write_all(request.as_bytes()).ok()?;
    let _ = stream.shutdown(Shutdown::Write);

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).ok()?;
    if buf.is_empty() {
        return None;
    }

    let response = String::from_utf8_lossy(&buf);
    let (headers, body) = response.split_once("\r\n\r\n")?;
    let status = headers.lines().next()?.split_whitespace().nth(1)?.parse::<u16>().ok()?;

    let is_chunked = headers.to_ascii_lowercase().contains("transfer-encoding: chunked");
    let decoded_body = if is_chunked {
        decode_chunked_body(body)
    } else {
        body.to_string()
    };

    Some((status, decoded_body))
}

fn decode_chunked_body(raw: &str) -> String {
    let mut result = String::new();
    let mut remaining = raw;
    loop {
        let remaining_trimmed = remaining.trim_start_matches("\r\n");
        let (size_str, rest) = match remaining_trimmed.split_once("\r\n") {
            Some(pair) => pair,
            None => break,
        };
        let chunk_size = match usize::from_str_radix(size_str.trim(), 16) {
            Ok(s) => s,
            Err(_) => break,
        };
        if chunk_size == 0 {
            break;
        }
        let chunk_data: String = rest.chars().take(chunk_size).collect();
        result.push_str(&chunk_data);
        remaining = &rest[chunk_data.len().min(rest.len())..];
    }
    result
}

fn get_cursor_window_meta(port: u16) -> Option<CursorWindowMeta> {
    let request = format!(
        "GET /window-meta HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    );
    let (status, body) = read_local_http_response(port, request)?;
    if status != 200 {
        return None;
    }
    serde_json::from_str::<CursorWindowMeta>(&body).ok()
}

fn post_cursor_window_action(port: u16, path: &str, body: &str) -> bool {
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body,
    );
    read_local_http_response(port, request)
        .map(|(status, _)| (200..300).contains(&status))
        .unwrap_or(false)
}

pub(crate) fn resolve_cursor_window_binding(
    cwd: &str,
    existing_port: Option<u16>,
    existing_native_handle: Option<&str>,
) -> Option<CursorWindowBinding> {
    #[derive(Debug)]
    struct Candidate {
        port: u16,
        workspace_root: String,
        workspace_name: String,
        native_handle: Option<String>,
        score: usize,
        focused: bool,
        keep_existing: bool,
        handle_match: bool,
    }

    log::info!("[cursor_bind_resolve] cwd={} existing_port={:?} existing_handle={:?}",
        cwd, existing_port, existing_native_handle);

    let mut candidates: Vec<Candidate> = Vec::new();
    for port in 23456..=23460u16 {
        let meta = match get_cursor_window_meta(port) {
            Some(meta) => meta,
            None => continue,
        };

        log::info!("[cursor_bind_resolve] port={} meta: focused={} workspace_name={} roots={:?} nativeHandle={:?}",
            meta.port, meta.focused, meta.workspace_name, meta.workspace_roots, meta.native_handle);

        let mut best_root: Option<String> = None;
        let mut best_score: usize = 0;
        for root in &meta.workspace_roots {
            if cwd_matches_workspace_root(cwd, root) {
                let score = root.len();
                if score >= best_score {
                    best_score = score;
                    best_root = Some(root.clone());
                }
            }
        }

        if let Some(workspace_root) = best_root {
            let handle_match = match (existing_native_handle, &meta.native_handle) {
                (Some(existing), Some(current)) => existing == current,
                _ => false,
            };
            candidates.push(Candidate {
                port: meta.port,
                workspace_root,
                workspace_name: if meta.workspace_name.is_empty() {
                    cursor_workspace_name_from_path(cwd)
                } else {
                    meta.workspace_name
                },
                native_handle: meta.native_handle,
                score: best_score,
                focused: meta.focused,
                keep_existing: existing_port == Some(meta.port),
                handle_match,
            });
        }
    }

    log::info!("[cursor_bind_resolve] {} candidates: {:?}",
        candidates.len(), candidates.iter().map(|c| format!("port={} score={} focused={} handle_match={} keep_existing={} handle={:?}",
            c.port, c.score, c.focused, c.handle_match, c.keep_existing, c.native_handle)).collect::<Vec<_>>());

    // If we have a native handle match, that wins unconditionally.
    if let Some(idx) = candidates.iter().position(|c| c.handle_match) {
        let c = &candidates[idx];
        log::info!("[cursor_bind_resolve] → native handle match: port={}", c.port);
        return Some(CursorWindowBinding {
            port: c.port,
            workspace_root: c.workspace_root.clone(),
            workspace_name: c.workspace_name.clone(),
            native_handle: c.native_handle.clone(),
        });
    }

    // Stick with existing bound port if still valid.
    if let Some(ep) = existing_port {
        if let Some(c) = candidates.iter().find(|c| c.port == ep) {
            log::info!("[cursor_bind_resolve] → keeping existing port={}", ep);
            return Some(CursorWindowBinding {
                port: c.port,
                workspace_root: c.workspace_root.clone(),
                workspace_name: c.workspace_name.clone(),
                native_handle: c.native_handle.clone(),
            });
        }
    }

    candidates.sort_by(|a, b| {
        b.score.cmp(&a.score)
            .then_with(|| b.focused.cmp(&a.focused))
            .then_with(|| a.port.cmp(&b.port))
    });

    let best = candidates.first()?;
    log::info!("[cursor_bind_resolve] → best candidate: port={} score={} focused={}", best.port, best.score, best.focused);

    Some(CursorWindowBinding {
        port: best.port,
        workspace_root: best.workspace_root.clone(),
        workspace_name: best.workspace_name.clone(),
        native_handle: best.native_handle.clone(),
    })
}


#[cfg(target_os = "macos")]
pub(crate) use crate::platform::macos::check_accessibility_permission;

#[cfg(not(target_os = "macos"))]
pub(crate) fn check_accessibility_permission() -> bool {
    true
}

// Re-export Windows helpers that command modules reach via `crate::*`.
#[cfg(target_os = "windows")]
pub(crate) use crate::platform::windows::hide_window_cmd;

// Re-export macOS-only helpers that command modules reach via `crate::*`.
#[cfg(target_os = "macos")]
pub(crate) use crate::platform::macos::{
    compute_frontmost_app_window_macos, find_terminal_app_for_pid,
    frontmost_app_window_cache, get_active_ghostty_terminal_id, get_frontmost_app_name,
    get_notch_offset, pet_context_schedule_restore_alpha, pet_passthrough_poll,
};


/// Focus the Cursor terminal tab for a given session.
/// Cursor hook payloads do not contain a stable terminal pid. The pid we see in
/// events changes from one hook invocation to the next, so it is not reliable
/// for jump-back. Instead we bind each session to a specific Cursor window by:
/// 1. Matching the session cwd against window metadata exposed by the extension
///    (`/window-meta` on ports 23456-23460).
/// 2. Reusing that bound port on click so we target one Cursor window instead
///    of broadcasting to all windows and hoping the right one wins.
/// 3. Raising the matching Cursor window on macOS by workspace name.
#[tauri::command]
async fn focus_cursor_terminal(session_id: String, state: tauri::State<'_, ClaudeState>) -> Result<String, String> {
    log::info!("[focus_cursor] called for session={}", &session_id[..session_id.len().min(8)]);

    let ax_ok = check_accessibility_permission();
    log::info!("[focus_cursor] accessibility_permission={}", ax_ok);

    let (cwd, existing_port, existing_workspace_root, existing_workspace_name, existing_native_handle) = {
        let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
        match sessions.get(&session_id) {
            Some(s) => (
                s.cwd.clone(),
                s.cursor_port,
                s.cursor_workspace_root.clone(),
                s.cursor_workspace_name.clone(),
                s.cursor_native_handle.clone(),
            ),
            None => (String::new(), None, None, None, None),
        }
    };

    let resolved_binding = if !cwd.is_empty() {
        resolve_cursor_window_binding(&cwd, existing_port, existing_native_handle.as_deref())
    } else {
        None
    };

    if let Some(binding) = &resolved_binding {
        if let Ok(mut sessions) = state.sessions.lock() {
            if let Some(session) = sessions.get_mut(&session_id) {
                session.cursor_port = Some(binding.port);
                session.cursor_workspace_root = Some(binding.workspace_root.clone());
                session.cursor_workspace_name = Some(binding.workspace_name.clone());
                session.cursor_native_handle = binding.native_handle.clone();
            }
        }
    }

    let port = resolved_binding.as_ref().map(|b| b.port).or(existing_port);
    let workspace_name = resolved_binding.as_ref().map(|b| b.workspace_name.clone())
        .or(existing_workspace_name)
        .or_else(|| existing_workspace_root.as_deref().map(cursor_workspace_name_from_path))
        .or_else(|| (!cwd.is_empty()).then(|| cursor_workspace_name_from_path(&cwd)))
        .unwrap_or_default();

    log::info!("[focus_cursor] session={} cwd={} port={:?} workspace_name={}",
        &session_id[..session_id.len().min(8)], cwd, port, workspace_name);

    #[cfg(target_os = "macos")]
    activate_cursor_workspace_window(&workspace_name);

    if let Some(port) = port {
        let focused = post_cursor_window_action(port, "/focus-window", "{}");
        log::info!("[focus_cursor] POST /focus-window to port {} → {}", port, focused);
        if focused {
            return Ok(format!("Focused Cursor window on port {}", port));
        }
        return Ok(format!("Activated Cursor window but /focus-window failed on port {}", port));
    }

    #[cfg(target_os = "macos")]
    activate_cursor_workspace_window(&workspace_name);

    Ok("Activated Cursor without a bound window".to_string())
}

/// Jump to the terminal running a Claude Code session.
/// Walks the parent process chain from the given PID to identify the terminal app,
/// then uses AppleScript (macOS) to activate and focus the matching window.
#[tauri::command]
async fn jump_to_claude_terminal(session_id: String, state: tauri::State<'_, ClaudeState>) -> Result<String, String> {
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let session = sessions.get(&session_id).ok_or("Session not found")?;
    let cwd = session.cwd.clone();
    let terminal_id = session.terminal_id.clone();
    let pid = session.pid;
    let source = session.source.clone();
    drop(sessions);

    #[cfg(target_os = "macos")]
    {
        let try_activate_app = |app_name: &str| -> bool {
            let script = format!(r#"tell application "{}" to activate"#, app_name.replace('"', "\\\""));
            if std::process::Command::new("osascript")
                .args(["-e", &script])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return true;
            }
            std::process::Command::new("open")
                .args(["-a", app_name])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };

        // Codex sessions should jump to the Codex app directly.
        // Do not route through Ghostty first; that causes the "terminal flash"
        // and may still require manual dock clicks to bring Codex frontmost.
        if source == "codex" {
            for app_name in ["Codex", "Code"] {
                if try_activate_app(app_name) {
                    return Ok(format!("Activated {}", app_name));
                }
            }
            // If Codex app activation fails (e.g. not installed as app bundle),
            // continue with terminal-based fallback paths below.
        }

        // Fast path: if we have a Ghostty terminal ID from hooks, jump directly
        // to that tab without depending on PID ancestry checks.
        if let Some(tid_raw) = terminal_id.as_deref() {
            if !tid_raw.is_empty() {
                let escaped_tid = tid_raw.replace('\\', "\\\\").replace('"', "\\\"");
                let script = format!(
                    r#"tell application "Ghostty"
    if not (it is running) then return ""
    set targetWindow to missing value
    set targetTab to missing value
    set targetTerminal to missing value
    repeat with aWindow in windows
        repeat with aTab in tabs of aWindow
            repeat with aTerminal in terminals of aTab
                try
                    if (id of aTerminal as text) is "{tid}" then
                        set targetWindow to aWindow
                        set targetTab to aTab
                        set targetTerminal to aTerminal
                        exit repeat
                    end if
                end try
            end repeat
            if targetTerminal is not missing value then exit repeat
        end repeat
        if targetTerminal is not missing value then exit repeat
    end repeat
    if targetTerminal is missing value then return ""
    activate
    delay 0.05
    if targetTab is not missing value then
        select tab targetTab
        delay 0.05
    end if
    if targetWindow is not missing value then
        set index of targetWindow to 1
    end if
    focus targetTerminal
    return "matched"
end tell"#,
                    tid = escaped_tid,
                );
                if let Ok(out) = std::process::Command::new("osascript").args(["-e", &script]).output() {
                    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if out.status.success() && stdout == "matched" {
                        return Ok("Jumped to Ghostty".to_string());
                    }
                }

                // Some Ghostty builds may format terminal IDs slightly differently.
                // Retry with a prefix contains-match to avoid false negatives.
                let tid_prefix = &tid_raw[..tid_raw.len().min(8)];
                if !tid_prefix.is_empty() {
                    let escaped_prefix = tid_prefix.replace('\\', "\\\\").replace('"', "\\\"");
                    let fallback_script = format!(
                        r#"tell application "Ghostty"
    if not (it is running) then return ""
    set targetWindow to missing value
    set targetTab to missing value
    set targetTerminal to missing value
    repeat with aWindow in windows
        repeat with aTab in tabs of aWindow
            repeat with aTerminal in terminals of aTab
                try
                    if (id of aTerminal as text) contains "{prefix}" then
                        set targetWindow to aWindow
                        set targetTab to aTab
                        set targetTerminal to aTerminal
                        exit repeat
                    end if
                end try
            end repeat
            if targetTerminal is not missing value then exit repeat
        end repeat
        if targetTerminal is not missing value then exit repeat
    end repeat
    if targetTerminal is missing value then return ""
    activate
    delay 0.05
    if targetTab is not missing value then
        select tab targetTab
        delay 0.05
    end if
    if targetWindow is not missing value then
        set index of targetWindow to 1
    end if
    focus targetTerminal
    return "matched"
end tell"#,
                        prefix = escaped_prefix,
                    );
                    if let Ok(out) = std::process::Command::new("osascript").args(["-e", &fallback_script]).output() {
                        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        if out.status.success() && stdout == "matched" {
                            return Ok("Jumped to Ghostty".to_string());
                        }
                    }
                }
            }
        }

        let pid = if let Some(p) = pid {
            p
        } else if source == "codex" {
            for app_name in ["Codex", "Ghostty", "Cursor"] {
                if try_activate_app(app_name) {
                    return Ok(format!("Activated {}", app_name));
                }
            }
            return Err("No PID tracked for this Codex session".to_string());
        } else {
            return Err("No PID tracked for this session".to_string());
        };
        // Walk parent process chain to find the terminal application
        let terminal_app = find_terminal_app_for_pid(pid);

        let tty = get_tty_for_pid(pid);
        let escaped_cwd = cwd.replace('\\', "\\\\").replace('"', "\\\"");
        let escaped_tty = tty.as_deref().unwrap_or("").replace('\\', "\\\\").replace('"', "\\\"");
        let escaped_sid = session_id.replace('\\', "\\\\").replace('"', "\\\"");

        match terminal_app.as_deref() {
            Some("Ghostty" | "ghostty") => {
                // Matching strategy (most → least precise):
                // 0. Stored terminal `id` captured at session start
                // 1. Session ID substring in tab title
                // 2. Working directory (ambiguous if multiple tabs share CWD)
                //
                // IMPORTANT: do NOT `activate` before matching — that would
                // bring Ghostty to front showing whatever tab was last
                // selected, giving a wrong-tab flash.
                let escaped_tid = terminal_id.as_deref().unwrap_or("").replace('\\', "\\\\").replace('"', "\\\"");
                let script = format!(
                    r#"tell application "Ghostty"
    if not (it is running) then return ""

    set targetWindow to missing value
    set targetTab to missing value
    set targetTerminal to missing value

    -- Pass 0: match by stored terminal id (most precise)
    if "{tid}" is not "" then
        repeat with aWindow in windows
            repeat with aTab in tabs of aWindow
                repeat with aTerminal in terminals of aTab
                    try
                        if (id of aTerminal as text) is "{tid}" then
                            set targetWindow to aWindow
                            set targetTab to aTab
                            set targetTerminal to aTerminal
                            exit repeat
                        end if
                    end try
                end repeat
                if targetTerminal is not missing value then exit repeat
            end repeat
            if targetTerminal is not missing value then exit repeat
        end repeat
    end if

    -- Pass 1: match by session ID in tab title
    if targetTerminal is missing value and "{sid}" is not "" then
        repeat with aWindow in windows
            repeat with aTab in tabs of aWindow
                repeat with aTerminal in terminals of aTab
                    try
                        if (name of aTerminal as text) contains "{sid_prefix}" then
                            set targetWindow to aWindow
                            set targetTab to aTab
                            set targetTerminal to aTerminal
                            exit repeat
                        end if
                    end try
                end repeat
                if targetTerminal is not missing value then exit repeat
            end repeat
            if targetTerminal is not missing value then exit repeat
        end repeat
    end if

    -- Pass 2: match by working directory (least precise)
    if targetTerminal is missing value and "{cwd}" is not "" then
        repeat with aWindow in windows
            repeat with aTab in tabs of aWindow
                repeat with aTerminal in terminals of aTab
                    try
                        if (working directory of aTerminal as text) is "{cwd}" then
                            set targetWindow to aWindow
                            set targetTab to aTab
                            set targetTerminal to aTerminal
                            exit repeat
                        end if
                    end try
                end repeat
                if targetTerminal is not missing value then exit repeat
            end repeat
            if targetTerminal is not missing value then exit repeat
        end repeat
    end if

    if targetTerminal is missing value then return ""

    -- Activate AFTER matching so the correct tab is shown immediately.
    activate
    delay 0.05
    if targetTab is not missing value then
        select tab targetTab
        delay 0.05
    end if
    if targetWindow is not missing value then
        set index of targetWindow to 1
    end if
    focus targetTerminal
    return "matched"
end tell"#,
                    tid = escaped_tid,
                    sid = escaped_sid,
                    sid_prefix = &escaped_sid[..escaped_sid.len().min(12)],
                    cwd = escaped_cwd,
                );
                let _ = std::process::Command::new("osascript")
                    .args(["-e", &script])
                    .output();
                Ok("Jumped to Ghostty".to_string())
            }
            Some("iTerm" | "iTerm2" | "iterm2") => {
                if !escaped_tty.is_empty() {
                    let script = format!(
                        r#"tell application "iTerm2"
    activate
    repeat with aWindow in windows
        repeat with aTab in tabs of aWindow
            repeat with aSession in sessions of aTab
                if tty of aSession is "{tty}" then
                    select aSession
                    tell aWindow to select
                    return "found"
                end if
            end repeat
        end repeat
    end repeat
end tell"#,
                        tty = escaped_tty
                    );
                    let _ = std::process::Command::new("osascript")
                        .args(["-e", &script])
                        .output();
                } else {
                    let _ = std::process::Command::new("osascript")
                        .args(["-e", r#"tell application "iTerm2" to activate"#])
                        .output();
                }
                Ok("Jumped to iTerm2".to_string())
            }
            Some("Terminal" | "Apple_Terminal") => {
                if !escaped_tty.is_empty() {
                    let script = format!(
                        r#"tell application "Terminal"
    activate
    repeat with aWindow in windows
        repeat with aTab in tabs of aWindow
            if tty of aTab is "{tty}" then
                set selected tab of aWindow to aTab
                set index of aWindow to 1
                return "found"
            end if
        end repeat
    end repeat
end tell"#,
                        tty = escaped_tty
                    );
                    let _ = std::process::Command::new("osascript")
                        .args(["-e", &script])
                        .output();
                } else {
                    let _ = std::process::Command::new("osascript")
                        .args(["-e", r#"tell application "Terminal" to activate"#])
                        .output();
                }
                Ok("Jumped to Terminal.app".to_string())
            }
            Some("Cursor") => {
                let _ = std::process::Command::new("open")
                    .args(["-a", "Cursor"])
                    .output();
                Ok("Jumped to Cursor".to_string())
            }
            Some(app_name) => {
                let script = format!(
                    r#"tell application "{}" to activate"#,
                    app_name.replace('"', "\\\"")
                );
                let _ = std::process::Command::new("osascript")
                    .args(["-e", &script])
                    .output();
                Ok(format!("Jumped to {}", app_name))
            }
            None => {
                if source == "codex" {
                    for app_name in ["Codex", "Ghostty", "Cursor"] {
                        if try_activate_app(app_name) {
                            return Ok(format!("Activated {}", app_name));
                        }
                    }
                }
                if !cwd.is_empty() {
                    let _ = std::process::Command::new("open").arg(&cwd).spawn();
                }
                Err("Could not identify the terminal application".to_string())
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        // On Windows/Linux, try to open the working directory
        if !cwd.is_empty() {
            let _ = std::process::Command::new("open").arg(&cwd).spawn();
        }
        Ok("Opened working directory".to_string())
    }
}




pub(crate) fn is_codex_internal_utility_session(session: &ClaudeSession) -> bool {
    if session.source != "codex" {
        return false;
    }

    let prompt = session.user_prompt.as_deref().unwrap_or("");
    if prompt.starts_with("You are a helpful assistant. You will be presented with a user prompt") {
        return true;
    }

    let last = session.last_response.as_deref().unwrap_or("").trim_start();
    last.starts_with("{\"title\":")
}

/// Process a Claude hook event (shared logic between Unix socket and TCP server).
/// Returns Some((session_id, hook_event)) if the event needs further handling
/// (e.g. PermissionRequest requires blocking the connection for a response).
fn start_cursor_socket_server(
    claude_state: Arc<Mutex<HashMap<String, ClaudeSession>>>,
    app: tauri::AppHandle,
) {
    #[cfg(unix)]
    {
        let socket_path = "/tmp/occlaw-cursor.sock";
        let _ = std::fs::remove_file(socket_path);
        let listener = match std::os::unix::net::UnixListener::bind(socket_path) {
            Ok(l) => l,
            Err(e) => { log::warn!("[cursor_socket] bind failed: {}", e); return; }
        };
        log::info!("[cursor_socket] listening on {}", socket_path);

        let state = Arc::clone(&claude_state);
        let app2 = app.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if let Ok(mut stream) = stream {
                    let state = Arc::clone(&state);
                    let app = app2.clone();
                    std::thread::spawn(move || {
                        use std::io::Read;
                        let mut buf = String::new();
                        let _ = stream.read_to_string(&mut buf);
                        if !buf.is_empty() {
                            // Cursor events never block (no PermissionRequest)
                            process_claude_event(&buf, &state, &app, Some("cursor"));
                        }
                    });
                }
            }
        });
    }

    #[cfg(windows)]
    {
        let listener = match std::net::TcpListener::bind("127.0.0.1:19284") {
            Ok(l) => l,
            Err(e) => { log::warn!("[cursor_socket] TCP bind failed: {}", e); return; }
        };
        log::info!("[cursor_socket] listening on 127.0.0.1:19284");

        let state = Arc::clone(&claude_state);
        let app2 = app.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if let Ok(mut stream) = stream {
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
                    let state = Arc::clone(&state);
                    let app = app2.clone();
                    std::thread::spawn(move || {
                        use std::io::Read;
                        let mut buf = String::new();
                        let _ = stream.read_to_string(&mut buf);
                        if !buf.is_empty() {
                            process_claude_event(&buf, &state, &app, Some("cursor"));
                        }
                    });
                }
            }
        });
    }
}

/// Start the Claude IPC server.
/// On macOS/Linux: Unix domain socket at /tmp/ooclaw-claude.sock
/// On Windows: TCP server on localhost:19283
fn start_claude_socket_server(
    claude_state: Arc<Mutex<HashMap<String, ClaudeSession>>>,
    pending_permissions: PendingPermissions,
    app_handle: tauri::AppHandle,
) {
    #[cfg(unix)]
    {
        let state = claude_state;
        let pending = pending_permissions;
        let app = app_handle;
        std::thread::spawn(move || {
            let sock_path = "/tmp/ooclaw-claude.sock";
            let _ = std::fs::remove_file(sock_path);

            let listener = match std::os::unix::net::UnixListener::bind(sock_path) {
                Ok(l) => l,
                Err(e) => { log::error!("Failed to bind claude socket: {}", e); return; }
            };
            log::info!("Claude socket server listening on {}", sock_path);

            for stream in listener.incoming() {
                match stream {
                    Ok(s) => {
                        let state = state.clone();
                        let app = app.clone();
                        let pending = pending.clone();
                        std::thread::spawn(move || {
                            use std::io::{Read, Write};
                            let mut s = s;
                            let mut buf = String::new();
                            let _ = s.read_to_string(&mut buf);
                            if let Some((session_id, hook_event)) = process_claude_event(&buf, &state, &app, None) {
                                if hook_event == "PermissionRequest" {
                                    let (tx, rx) = std::sync::mpsc::channel::<String>();
                                    {
                                        let mut map = pending.lock().unwrap();
                                        map.insert(session_id.clone(), tx);
                                    }
                                    log::info!("[claude_socket] blocking for PermissionRequest session={}", &session_id[..session_id.len().min(8)]);
                                    match rx.recv_timeout(std::time::Duration::from_secs(600)) {
                                        Ok(response_json) => {
                                            log::info!("[claude_socket] sending permission response for session={}", &session_id[..session_id.len().min(8)]);
                                            let _ = s.write_all(response_json.as_bytes());
                                            let _ = s.flush();
                                        }
                                        Err(_) => {
                                            log::warn!("[claude_socket] permission timeout for session={}", &session_id[..session_id.len().min(8)]);
                                        }
                                    }
                                    let mut map = pending.lock().unwrap();
                                    map.remove(&session_id);
                                }
                            }
                        });
                    }
                    Err(e) => { log::error!("Claude socket accept error: {}", e); }
                }
            }
        });
    }

    #[cfg(windows)]
    {
        let state = claude_state;
        let pending = pending_permissions;
        let app = app_handle;
        std::thread::spawn(move || {
            use std::net::TcpListener;
            let listener = match TcpListener::bind("127.0.0.1:19283") {
                Ok(l) => l,
                Err(e) => { log::error!("Failed to bind claude TCP socket: {}", e); return; }
            };
            log::info!("Claude TCP server listening on 127.0.0.1:19283");

            for stream in listener.incoming() {
                match stream {
                    Ok(mut s) => {
                        let state = state.clone();
                        let app = app.clone();
                        let pending = pending.clone();
                        std::thread::spawn(move || {
                            use std::io::{Read, Write};
                            s.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
                            let mut buf = Vec::new();
                            let mut chunk = [0u8; 4096];
                            loop {
                                match s.read(&mut chunk) {
                                    Ok(0) => break,
                                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                                    Err(e) => {
                                        if !buf.is_empty() { break; }
                                        log::warn!("[claude_tcp] read error with empty buf: {}", e);
                                        return;
                                    }
                                }
                            }
                            let text = String::from_utf8_lossy(&buf);
                            // Cursor + Codex support are dropped on Windows.
                            // Their hook scripts (or, in cursor's case, the
                            // bundled Claude Code extension) still occasionally
                            // reach this socket. Cursor payloads always carry
                            // `cursor_version`; Codex hooks always set
                            // `"source":"codex"`. Drop both outright so they
                            // cannot drive the completion sound or pollute the
                            // session list.
                            if text.contains("\"cursor_version\"") {
                                log::info!("[claude_tcp] dropping cursor-originated event on windows (len={})", text.len());
                                return;
                            }
                            if text.contains("\"source\":\"codex\"") || text.contains("\"source\": \"codex\"") {
                                log::info!("[claude_tcp] dropping codex-originated event on windows (len={})", text.len());
                                return;
                            }
                            if let Some((session_id, hook_event)) = process_claude_event(&text, &state, &app, None) {
                                if hook_event == "PermissionRequest" {
                                    let (tx, rx) = std::sync::mpsc::channel::<String>();
                                    {
                                        let mut map = pending.lock().unwrap();
                                        map.insert(session_id.clone(), tx);
                                    }
                                    s.set_read_timeout(None).ok();
                                    match rx.recv_timeout(std::time::Duration::from_secs(600)) {
                                        Ok(response_json) => {
                                            let _ = s.write_all(response_json.as_bytes());
                                            let _ = s.flush();
                                        }
                                        Err(_) => {
                                            log::warn!("[claude_tcp] permission timeout for session={}", &session_id[..session_id.len().min(8)]);
                                        }
                                    }
                                    let mut map = pending.lock().unwrap();
                                    map.remove(&session_id);
                                }
                            }
                        });
                    }
                    Err(e) => { log::error!("Claude TCP accept error: {}", e); }
                }
            }
        });
    }
}



fn asset_mime_for_path(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".webm") {
        "video/webm"
    } else if lower.ends_with(".mp4") {
        "video/mp4"
    } else if lower.ends_with(".mov") {
        "video/quicktime"
    } else {
        "application/octet-stream"
    }
}

fn build_asset_response(
    req: &tauri::http::Request<Vec<u8>>,
    path: &str,
    file_path: &std::path::Path,
    add_cors: bool,
    log_label: &str,
) -> tauri::http::Response<Vec<u8>> {
    match std::fs::read(file_path) {
        Ok(data) => {
            let mime = asset_mime_for_path(path);
            let total_len = data.len();
            let mut status = 200;
            let mut body = data;
            let mut content_range: Option<String> = None;

            // Serve byte ranges for media files so WKWebView/Safari can stream
            // video containers like HEVC .mov/.mp4 reliably.
            if total_len > 0 {
                if let Some(range_header) = req.headers().get("Range").or_else(|| req.headers().get("range")) {
                    if let Ok(range) = range_header.to_str() {
                        if let Some(spec) = range.strip_prefix("bytes=") {
                            let mut parts = spec.splitn(2, '-');
                            let start_part = parts.next().unwrap_or("");
                            let end_part = parts.next().unwrap_or("");
                            let parsed = if start_part.is_empty() {
                                end_part.parse::<usize>().ok().map(|suffix_len| {
                                    let suffix_len = suffix_len.min(total_len);
                                    let start = total_len.saturating_sub(suffix_len);
                                    (start, total_len.saturating_sub(1))
                                })
                            } else if let Ok(start) = start_part.parse::<usize>() {
                                let end = if end_part.is_empty() {
                                    total_len.saturating_sub(1)
                                } else {
                                    end_part
                                        .parse::<usize>()
                                        .unwrap_or(total_len.saturating_sub(1))
                                        .min(total_len.saturating_sub(1))
                                };
                                if start < total_len && start <= end {
                                    Some((start, end))
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            if let Some((start, end)) = parsed {
                                body = body[start..=end].to_vec();
                                status = 206;
                                content_range = Some(format!("bytes {}-{}/{}", start, end, total_len));
                            }
                        }
                    }
                }
            }

            let mut resp = tauri::http::Response::builder()
                .status(status)
                .header("Content-Type", mime)
                .header("Content-Length", body.len().to_string())
                .header("Accept-Ranges", "bytes");
            if let Some(content_range) = content_range {
                resp = resp.header("Content-Range", content_range);
            }
            if add_cors {
                resp = resp.header("Access-Control-Allow-Origin", "*");
            }
            resp.body(body).unwrap()
        }
        Err(e) => {
            log::warn!("[{}] 404: {} err={}", log_label, file_path.display(), e);
            tauri::http::Response::builder()
                .status(404)
                .body(Vec::new())
                .unwrap()
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "windows")]
    {
        // WebView2 hardware video decode can drop VP9 alpha; force software decode.
        let key = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";
        let flag = "--disable-accelerated-video-decode";
        let merged = match std::env::var(key) {
            Ok(existing) if !existing.contains(flag) && !existing.trim().is_empty() => format!("{} {}", existing, flag),
            Ok(existing) if existing.contains(flag) => existing,
            _ => flag.to_string(),
        };
        std::env::set_var(key, merged);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .register_uri_scheme_protocol("localasset", |ctx, req| {
            let raw_path = req.uri().path();
            let path = percent_decode_str(raw_path).decode_utf8_lossy();
            let resource_dir = ctx.app_handle().path().resource_dir().unwrap_or_default();
            let file_path = resource_dir.join("assets").join("builtin").join(path.trim_start_matches('/'));
            log::info!("[localasset] request={} resolved={}", raw_path, file_path.display());
            build_asset_response(&req, path.as_ref(), &file_path, cfg!(target_os = "windows"), "localasset")
        })
        .register_uri_scheme_protocol("codexpet", |_ctx, req| {
            // Custom codex pets the user dropped into `~/.codex/pets`.
            // Avatars are loaded through this protocol so the picker can
            // display sprites that live outside the bundled assets dir.
            let raw_path = req.uri().path();
            let path = percent_decode_str(raw_path).decode_utf8_lossy();
            let root = codex_pets_dir().unwrap_or_default();
            let file_path = root.join(path.trim_start_matches('/'));
            build_asset_response(&req, path.as_ref(), &file_path, cfg!(target_os = "windows"), "codexpet")
        })
        .setup(|app| {
            // Fix PATH so openclaw (Node.js script) and node are both reachable
            fix_path();

            // Install Claude + Codex hooks on every startup (idempotent)
            if let Err(e) = tauri::async_runtime::block_on(install_claude_hooks()) {
                log::warn!("Failed to install Claude hooks on startup: {}", e);
            }
            // Install Cursor hooks + terminal-focus extension on startup (idempotent)
            if let Err(e) = tauri::async_runtime::block_on(install_cursor_hooks()) {
                log::warn!("Failed to install Cursor hooks on startup: {}", e);
            }

            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .build(),
            )?;

            // Run the WKWebView swizzle AFTER the log plugin is initialized so
            // its [first-mouse] / IME log lines are actually visible in the
            // tauri-plugin-log stream. Order vs window creation is fine —
            // setup() runs after the mini webview already exists.
            #[cfg(target_os = "macos")]
            install_wry_webview_ime_fix();

            // Init speech recognition thread and register global shortcut
            #[cfg(target_os = "macos")]
            {
                speech::init_speech_thread(app.handle().clone());
                log::info!("[voice] speech thread started, registering shortcut");
                use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
                if let Err(e) = app.global_shortcut().on_shortcut("ctrl+shift+v", move |_app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        log::info!("[voice] shortcut pressed, recording={}", speech::is_recording());
                        if speech::is_recording() {
                            let _ = speech::stop_recording();
                        } else {
                            let _ = speech::start_recording();
                        }
                    }
                }) {
                    log::warn!("[voice] failed to register shortcut: {}", e);
                }
                log::info!("[voice] shortcut registered, setup continuing");
            }

            // Hide from Dock, show only in menu bar (macOS only)
            #[cfg(target_os = "macos")]
            {
                use objc2::runtime::{AnyClass, AnyObject};
                use objc2::msg_send;
                unsafe {
                    let ns_app_cls = AnyClass::get(c"NSApplication").unwrap();
                    let ns_app: *mut AnyObject = msg_send![ns_app_cls, sharedApplication];
                    // NSApplicationActivationPolicyAccessory = 1
                    let _: () = msg_send![ns_app, setActivationPolicy: 1i64];
                }
            }

            // Set window properties, seed screen/frame info for the hover
            // poll thread, and show.
            #[cfg(target_os = "macos")]
            if let Some(win) = app.get_webview_window("main") {
                let win_clone = win.clone();
                let _ = app.handle().run_on_main_thread(move || {
                    use objc2::runtime::AnyObject;
                    use objc2::msg_send;
                    use objc2_foundation::{NSRect, NSPoint, NSSize};

                    if let Ok(ns_win) = win_clone.ns_window() {
                        let obj = unsafe { &*(ns_win as *mut AnyObject) };
                        unsafe {
                            let _: () = msg_send![obj, setLevel: 27isize];
                            let behavior: usize = (1 << 0) | (1 << 4) | (1 << 8) | (1 << 6);
                            let _: () = msg_send![obj, setCollectionBehavior: behavior];
                            let _: () = msg_send![obj, setAcceptsMouseMovedEvents: true];

                            // Seed NOTCH_SCREEN_INFO + MINI_WINDOW_FRAME so the
                            // efficiency hover/drag poll thread can work from the
                            // first tick (otherwise it silently no-ops until the
                            // panel is toggled via set_mini_expanded).
                            let screen: *mut AnyObject = msg_send![obj, screen];
                            if !screen.is_null() {
                                let sf: NSRect = msg_send![&*screen, frame];
                                let notch_off = get_notch_offset(screen);
                                if let Ok(mut info) = NOTCH_SCREEN_INFO.lock() {
                                    *info = Some((sf.origin.x, sf.origin.y, sf.size.width, sf.size.height, notch_off));
                                }
                            }
                            let wf: NSRect = msg_send![obj, frame];
                            if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                                *f = Some((wf.origin.x, wf.origin.y, wf.size.width, wf.size.height));
                            }
                        }
                    }
                });
                let _ = win.show();
            }

            // Windows: position mini window at top-center of primary monitor
            #[cfg(target_os = "windows")]
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_always_on_top(true);
                let _ = win.set_skip_taskbar(true);
                if let Ok(Some(monitor)) = win.primary_monitor() {
                    let screen = monitor.size();
                    let scale = monitor.scale_factor();
                    let sw = screen.width as f64 / scale;
                    let x = sw / 2.0 + 40.0;
                    let _ = win.set_position(tauri::LogicalPosition::new(x, MASCOT_TOP_INSET));
                }
                let _ = win.show();
            }

            // Windows: move window off-screen when a fullscreen app is on the SAME
            // monitor as the mini window.  We avoid hide()/show() because show()
            // triggers a focus event which causes the panel to expand.
            #[cfg(target_os = "windows")]
            {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    use windows::Win32::Graphics::Gdi::{HMONITOR, MonitorFromPoint, MONITOR_DEFAULTTONEAREST};
                    use windows::Win32::Foundation::POINT;

                    let mut was_hidden = false;
                    let mut saved_pos: Option<tauri::LogicalPosition<f64>> = None;
                    let mut hidden_monitor: Option<HMONITOR> = None;
                    // Debounce counter: require several consecutive non-fullscreen
                    // polls before restoring, so brief foreground changes (mouse
                    // movement, overlay popups) during video playback don't cause
                    // the pet to flicker.
                    let mut non_fs_streak: u32 = 0;
                    const RESTORE_THRESHOLD: u32 = 4; // 4 × 500ms = 2s
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        let fs_monitor = fullscreen_foreground_monitor();

                        if let Some(win) = app_handle.get_webview_window("main") {
                            let tracked_monitor = if was_hidden {
                                hidden_monitor
                            } else if let Ok(pos) = win.outer_position() {
                                Some(unsafe {
                                    MonitorFromPoint(
                                        POINT { x: pos.x, y: pos.y },
                                        MONITOR_DEFAULTTONEAREST,
                                    )
                                })
                            } else {
                                None
                            };
                            let same_monitor = matches!(
                                (fs_monitor, tracked_monitor),
                                (Some(fs_mon), Some(mini_mon)) if mini_mon == fs_mon
                            );

                            if same_monitor {
                                non_fs_streak = 0;
                                if !was_hidden {
                                    log::info!("[fullscreen] detected fullscreen app on same monitor, moving mini off-screen");
                                    FULLSCREEN_HIDING.store(true, std::sync::atomic::Ordering::SeqCst);
                                    if let Ok(pos) = win.outer_position() {
                                        hidden_monitor = Some(unsafe {
                                            MonitorFromPoint(
                                                POINT { x: pos.x, y: pos.y },
                                                MONITOR_DEFAULTTONEAREST,
                                            )
                                        });
                                    }
                                    if let Ok(Some(pos)) = win.outer_position().map(|p| {
                                        win.current_monitor().ok().flatten().map(|m| {
                                            let s = m.scale_factor();
                                            tauri::LogicalPosition::new(p.x as f64 / s, p.y as f64 / s)
                                        })
                                    }) {
                                        saved_pos = Some(pos);
                                    }
                                    let _ = win.set_always_on_top(false);
                                    let _ = win.set_position(tauri::LogicalPosition::new(-9999.0_f64, -9999.0_f64));
                                    was_hidden = true;
                                }
                            } else if was_hidden {
                                non_fs_streak += 1;
                                if non_fs_streak >= RESTORE_THRESHOLD {
                                    log::info!("[fullscreen] fullscreen exited or on different monitor, restoring mini position");
                                    FULLSCREEN_HIDING.store(false, std::sync::atomic::Ordering::SeqCst);
                                    if let Some(pos) = saved_pos.take() {
                                        let _ = win.set_position(pos);
                                    }
                                    let _ = win.set_always_on_top(true);
                                    was_hidden = false;
                                    hidden_monitor = None;
                                    non_fs_streak = 0;
                                }
                            }
                        }
                    }
                });
            }

            // Start Claude Code socket server
            {
                let claude_state = app.state::<ClaudeState>();
                let sessions_arc = Arc::clone(&claude_state.sessions);
                let pending_arc = Arc::clone(&claude_state.pending_permissions);
                start_claude_socket_server(sessions_arc, pending_arc, app.handle().clone());
            }

            // Start Cursor socket server (shares ClaudeState for unified session tracking)
            // Cursor integration is disabled on Windows, so skip the server there.
            #[cfg(not(target_os = "windows"))]
            {
                let claude_state = app.state::<ClaudeState>();
                let sessions_arc = Arc::clone(&claude_state.sessions);
                start_cursor_socket_server(sessions_arc, app.handle().clone());
            }

            // System tray — use saved language, fallback to system language
            let initial_lang = {
                let store_path = app.path().app_data_dir().ok().map(|p| p.join("settings.json"));
                let mut lang = None;
                if let Some(ref sp) = store_path {
                    if let Ok(data) = std::fs::read_to_string(sp) {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&data) {
                            lang = val.get("pawbae-lang").and_then(|v| v.as_str()).map(|s| s.to_string());
                        }
                    }
                }
                lang.unwrap_or_else(|| {
                    let sys = std::env::var("LANG").unwrap_or_default().to_lowercase();
                    if sys.starts_with("zh") { "zh".into() }
                    else { "en".into() }
                })
            };
            let (show_label, hide_label, stroll_label, settings_label, quit_label) = tray_labels(&initial_lang);
            let _ = stroll_label;
            let show = MenuItem::with_id(app, "show", show_label, true, None::<&str>)?;
            let hide = MenuItem::with_id(app, "hide", hide_label, true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", settings_label, true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", quit_label, true, None::<&str>)?;
            #[cfg(target_os = "macos")]
            let menu = {
                let stroll = CheckMenuItem::with_id(
                    app,
                    "stroll",
                    stroll_label,
                    true,
                    STROLL_MODE_ENABLED.load(Ordering::SeqCst),
                    None::<&str>,
                )?;
                Menu::with_items(app, &[&show, &hide, &stroll, &settings, &quit])?
            };
            #[cfg(not(target_os = "macos"))]
            let menu = Menu::with_items(app, &[&show, &hide, &settings, &quit])?;

            // Use dedicated tray icon (logo-mini: white cat silhouette on transparent bg)
            // instead of the app icon, so it renders correctly in macOS menu bar / Windows tray
            let tray_icon_bytes = include_bytes!("../icons/tray-icon.png");
            let tray_icon = tauri::image::Image::from_bytes(tray_icon_bytes)
                .expect("failed to load tray icon");
            TrayIconBuilder::with_id("main")
                .icon(tray_icon)
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(win) = app.get_webview_window("main") {
                            #[cfg(target_os = "windows")]
                            {
                                FULLSCREEN_HIDING.store(false, std::sync::atomic::Ordering::SeqCst);
                                if let Ok(Some(monitor)) = win.primary_monitor() {
                                    let scale = monitor.scale_factor();
                                    let sw = monitor.size().width as f64 / scale;
                                    let ui = win_ui_scale(&monitor);
                                    let x = sw / 2.0 + (80.0 * ui).round();
                                    let _ = win.set_position(tauri::LogicalPosition::new(x, 0.0));
                                }
                                let _ = win.set_always_on_top(true);
                            }
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "hide" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.hide();
                        }
                    }
                    #[cfg(target_os = "macos")]
                    "stroll" => {
                        // Toggle the global stroll-mode flag, persist it
                        // through the frontend (which owns settings.json),
                        // and broadcast the new value so Mini.tsx can flip
                        // the physics loop on/off without a polling read.
                        let prev = STROLL_MODE_ENABLED.load(Ordering::SeqCst);
                        let next = !prev;
                        STROLL_MODE_ENABLED.store(next, Ordering::SeqCst);
                        // If the user disables stroll, also drop throw
                        // tracking so we stop sampling drag velocities.
                        if !next {
                            THROW_TRACKING_ENABLED.store(false, Ordering::SeqCst);
                        }
                        let _ = app.emit("stroll-mode-changed", next);
                    }
                    "settings" => {
                        if let Some(win) = app.get_webview_window("main") {
                            #[cfg(target_os = "windows")]
                            {
                                FULLSCREEN_HIDING.store(false, std::sync::atomic::Ordering::SeqCst);
                                if let Ok(Some(monitor)) = win.primary_monitor() {
                                    let scale = monitor.scale_factor();
                                    let sw = monitor.size().width as f64 / scale;
                                    let ui = win_ui_scale(&monitor);
                                    let x = sw / 2.0 + (80.0 * ui).round();
                                    let _ = win.set_position(tauri::LogicalPosition::new(x, 0.0));
                                }
                                let _ = win.set_always_on_top(true);
                            }
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                        let _ = app.emit("tray-open-settings", ());
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_status, send_chat, open_detail_panel, get_agents, get_health, get_agent_metrics, interrupt_agent, get_agent_extra_info, open_mini, close_mini, set_mini_expanded, set_mini_size, set_efficiency_hover_tracking, resize_mini_height, move_mini_by, get_mini_origin, get_mini_monitor_rect, get_pet_floor_info, get_frontmost_app_window, set_sprite_pad_fractions, set_mini_origin, set_ime_mode, get_agent_sessions, get_session_preview, get_session_messages, get_active_sessions, proxy_post, play_sound, get_claude_sessions, get_claude_conversation, install_claude_hooks, install_cursor_hooks, remove_claude_session, resolve_claude_permission, get_claude_stats, open_url, activate_app, focus_cursor_terminal, check_ax_permission, request_ax_permission, jump_to_claude_terminal, check_for_update, run_update, close_ssh, read_local_file, exit_app, get_ssh_key_info, reset_ssh, get_ui_scale, list_custom_codex_pets, open_codex_pets_dir, import_codex_pet, pick_codex_pet_folder, reassert_floating, spawn_demo_mascot, close_demo_mascot, close_demo_mascots, debug_log, update_tray_language, set_pet_mode_window, set_pet_context_menu, set_pet_pomodoro_active, get_now_playing, get_system_idle_time, set_stroll_mode, set_throw_tracking, voice_toggle, voice_is_recording])
        .manage(ActiveAgentPid { pid: Mutex::new(None) })
        .manage(ClaudeState { sessions: Arc::new(Mutex::new(HashMap::new())), pending_permissions: Arc::new(Mutex::new(HashMap::new())) })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
