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



#[tauri::command]
async fn get_frontmost_app_window(
    #[allow(unused_variables)] app: tauri::AppHandle,
) -> Result<Option<AppWindowInfo>, String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(cached) = frontmost_app_window_cache::try_fresh() {
            return Ok(cached);
        }
        // NSScreen reads are safest on the main thread; CGWindowList
        // itself is thread-safe but we batch with the screen read so a
        // single main-thread hop covers both.
        let (tx, rx) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let result = unsafe { compute_frontmost_app_window_macos() };
            frontmost_app_window_cache::store(result.clone());
            let _ = tx.send(result);
        }).map_err(|e| e.to_string())?;
        match rx.recv_timeout(std::time::Duration::from_millis(200)) {
            Ok(v) => Ok(v),
            // Timeout — treat as "no window right now". Better to skip
            // a tick than block the physics loop.
            Err(_) => Ok(None),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(None)
    }
}

fn current_sprite_pad() -> SpritePadFracs {
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

/// Frontend pushes runtime-measured pad values here so the Rust
/// safety-net clamp agrees with the frontend edge math. Any field can
/// be `None` to leave that side at its current value.
///
/// The `*_px` fields are *absolute* CSS-pixel offsets between each
/// visible sprite edge and the corresponding window edge. They are
/// the preferred overrides when the frontend can measure them from
/// the DOM. When set, they override the corresponding fraction in
/// `move_mini_by`'s clamp.
///
/// `reset_px` clears every px override before applying the rest of
/// the update — the frontend calls this on every physics-enable so
/// the previous pet's measurements don't leak into the new pet's
/// first physics tick.
#[tauri::command]
async fn set_sprite_pad_fractions(
    top: Option<f64>,
    right: Option<f64>,
    bottom: Option<f64>,
    left: Option<f64>,
    top_px: Option<f64>,
    right_px: Option<f64>,
    bottom_px: Option<f64>,
    left_px: Option<f64>,
    reset_px: Option<bool>,
) -> Result<(), String> {
    let mut g = SPRITE_PAD.lock().map_err(|e| e.to_string())?;
    if reset_px.unwrap_or(false) {
        g.top_px = None;
        g.right_px = None;
        g.bottom_px = None;
        g.left_px = None;
    }
    // Clamp each fraction to a sane range. A frac < 0 would lift the
    // window past the floor; a frac > 0.95 indicates a measurement
    // failure (essentially empty sprite). Silently ignore bad values
    // so a noisy frontend can't move the cat off-screen.
    if let Some(v) = top    { if v.is_finite() && v >= 0.0 && v <= 0.95 { g.top    = v; } }
    if let Some(v) = right  { if v.is_finite() && v >= 0.0 && v <= 0.95 { g.right  = v; } }
    if let Some(v) = bottom { if v.is_finite() && v >= 0.0 && v <= 0.95 { g.bottom = v; } }
    if let Some(v) = left   { if v.is_finite() && v >= 0.0 && v <= 0.95 { g.left   = v; } }
    // Absolute CSS pixels. Reject NaN / negative / insanely large
    // values so a buggy frontend can't push the cat off-screen.
    let validate_px = |v: f64| -> Option<f64> {
        if v.is_finite() && v >= 0.0 && v <= 1000.0 { Some(v) } else { None }
    };
    if let Some(v) = top_px    { if let Some(px) = validate_px(v) { g.top_px    = Some(px); } }
    if let Some(v) = right_px  { if let Some(px) = validate_px(v) { g.right_px  = Some(px); } }
    if let Some(v) = bottom_px { if let Some(px) = validate_px(v) { g.bottom_px = Some(px); } }
    if let Some(v) = left_px   { if let Some(px) = validate_px(v) { g.left_px   = Some(px); } }
    Ok(())
}

/// Pet-physics floor info, packed for one IPC roundtrip per cache TTL.
/// Y values are in macOS bottom-up logical pixels.
#[derive(serde::Serialize)]
struct PetFloorInfo {
    /// Floor Y when the mascot's center-x is inside `dock_x_range`. This
    /// is the top of the Dock (== `visibleFrame.origin.y`).
    on_dock_y: f64,
    /// Floor Y when the mascot's center-x is outside the Dock x-range.
    /// This is the actual bottom of the screen (`screen.frame.origin.y`).
    off_dock_y: f64,
    /// Horizontal extent of the Dock window in screen coords, or None
    /// when no Dock is on screen (auto-hide engaged, no Dock, etc.).
    dock_x_range: Option<(f64, f64)>,
}

#[tauri::command]
async fn get_pet_floor_info(app: tauri::AppHandle) -> Result<PetFloorInfo, String> {
    let win = app.get_webview_window("main").ok_or("mini window not found")?;
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let win_clone = win.clone();
        app.run_on_main_thread(move || {
            use objc2::runtime::{AnyClass, AnyObject};
            use objc2::msg_send;
            use objc2_foundation::NSRect;
            if let Ok(ns_win) = win_clone.ns_window() {
                let obj = unsafe { &*(ns_win as *mut AnyObject) };
                let screen: *mut AnyObject = unsafe { msg_send![obj, screen] };
                let (frame, visible): (Option<NSRect>, Option<NSRect>) = unsafe {
                    if screen.is_null() {
                        match AnyClass::get(c"NSScreen") {
                            Some(cls) => {
                                let main: *mut AnyObject = msg_send![cls, mainScreen];
                                if main.is_null() { (None, None) } else {
                                    (
                                        Some(msg_send![&*main, frame]),
                                        Some(msg_send![&*main, visibleFrame]),
                                    )
                                }
                            }
                            None => (None, None),
                        }
                    } else {
                        (
                            Some(msg_send![&*screen, frame]),
                            Some(msg_send![&*screen, visibleFrame]),
                        )
                    }
                };
                // Intentionally do NOT call CGWindowList here: that surface
                // is the path that triggers a Screen Recording permission
                // prompt on recent macOS versions, and we want the pet to
                // work zero-permission. Frontend treats `dock_x_range:
                // None` as "the entire visibleFrame width is the platform"
                // — the pet still sits on `visibleFrame.origin.y` (= top
                // of Dock) because that's what NSScreen gives us for free.
                let dock: Option<(f64, f64, f64, f64)> = None;
                let _ = tx.send((frame, visible, dock));
            }
        }).map_err(|e| e.to_string())?;
        if let Ok((frame, visible, dock)) = rx.recv_timeout(std::time::Duration::from_secs(1)) {
            let off_dock_y = frame.map(|f| f.origin.y).unwrap_or(0.0);
            let on_dock_y = visible.map(|v| v.origin.y).unwrap_or(off_dock_y);
            let dock_x_range = dock.map(|(x, _, w, _)| (x, x + w));
            return Ok(PetFloorInfo { on_dock_y, off_dock_y, dock_x_range });
        }
    }
    #[allow(unreachable_code)]
    Ok(PetFloorInfo { on_dock_y: 0.0, off_dock_y: 0.0, dock_x_range: None })
}

/// Move the mini window by a delta (dx, dy in CSS/logical points).
/// dy is in screen coordinates (positive = downward), converted to macOS (positive = upward).
///
/// On macOS the resulting origin is clamped to the screen's `visibleFrame`
/// (menu-bar / Dock / notch excluded). This is the authoritative safety
/// net for the pet physics loop: even at terminal velocity or during a
/// hard drag-throw, the window can never end up past a wall.

/// Start or stop cursor-position polling for efficiency-mode hover detection.
///
/// On macOS the mini window sits in the menu-bar / notch area. The system
/// menu bar intercepts mouse-move events, so the webview never receives
/// `mouseenter` / `mouseleave` DOM events there.  This command spawns a
/// lightweight background thread (50 ms poll) that reads `NSEvent.mouseLocation`
/// and compares it against the notch region (collapsed) or the panel region
/// (expanded).  It emits `"efficiency-hover"` events (`true` = entered,
/// `false` = left) so the frontend can open / close the panel.
#[tauri::command]
async fn set_efficiency_hover_tracking(app: tauri::AppHandle, active: bool) -> Result<(), String> {
    EFFICIENCY_HOVER_ACTIVE.store(active, Ordering::SeqCst);
    if active && !EFFICIENCY_HOVER_THREAD_ALIVE.load(Ordering::SeqCst) {
        let app2 = app.clone();
        std::thread::spawn(move || efficiency_hover_poll(app2));
    }
    Ok(())
}

/// Frontend pushes the persisted stroll-mode flag back to Rust at
/// startup so the tray check-state matches what was last toggled.
/// Also called when the user changes pet-physics availability (e.g.
/// switches to a non-physics pet) — in that case the frontend disables
/// throw tracking too.
#[tauri::command]
fn set_stroll_mode(_app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    STROLL_MODE_ENABLED.store(enabled, Ordering::SeqCst);
    if !enabled {
        THROW_TRACKING_ENABLED.store(false, Ordering::SeqCst);
    }
    Ok(())
}

/// Toggle drag-velocity sampling in the macOS NSEvent drag loop. The
/// frontend turns this on whenever stroll-mode is enabled AND the
/// selected pet declares physics. When off the drag loop skips the
/// per-tick VecDeque push, so legacy pets pay no perf cost.
#[tauri::command]
fn set_throw_tracking(enabled: bool) -> Result<(), String> {
    log::info!("[stroll] set_throw_tracking({})", enabled);
    THROW_TRACKING_ENABLED.store(enabled, Ordering::SeqCst);
    Ok(())
}

/// Background polling loop for efficiency-mode hover.
/// Checks the cursor position against two regions:
///  - **Collapsed**: a wide strip around the notch (notch_off*2 + 200 px,
///    50 px tall at the top of the screen) — much wider than the actual
///    window so the user can approach from either side.
///  - **Expanded**: the panel area (500 × 400 px, top-center).
fn efficiency_hover_poll(app: tauri::AppHandle) {
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
#[tauri::command]
async fn set_pet_mode_window(
    app: tauri::AppHandle,
    active: bool,
    mascot_scale: Option<f64>,
    large_mascot_scale: Option<f64>,
) -> Result<(), String> {
    let win = app.get_webview_window("main").ok_or("mini window not found")?;
    let mascot_scale = sanitized_mascot_scale(mascot_scale);
    let large_mascot_scale = large_mascot_scale.unwrap_or(LARGE_MASCOT_SIZE_MULTIPLIER);

    if active {
        // Expand window to menu-ready size (mascot area + padding for buttons).
        #[cfg(target_os = "macos")]
        {
            let win_clone = win.clone();
            app.run_on_main_thread(move || {
                use objc2::runtime::{AnyClass, AnyObject};
                use objc2::msg_send;
                use objc2_foundation::{NSRect, NSPoint, NSSize};
                if let Ok(ns_win) = win_clone.ns_window() {
                    let obj = unsafe { &*(ns_win as *mut AnyObject) };
                    let current: NSRect = unsafe { msg_send![obj, frame] };
                    let screen_info: Option<(f64, f64, f64, f64)> = unsafe {
                        let screen: *mut AnyObject = msg_send![obj, screen];
                        if screen.is_null() {
                            let cls = AnyClass::get(c"NSScreen");
                            cls.and_then(|c| {
                                let ms: *mut AnyObject = msg_send![c, mainScreen];
                                if ms.is_null() { None } else {
                                    let sf: NSRect = msg_send![&*ms, frame];
                                    Some((sf.origin.x, sf.origin.y, sf.size.width, sf.size.height))
                                }
                            })
                        } else {
                            let sf: NSRect = msg_send![&*screen, frame];
                            Some((sf.origin.x, sf.origin.y, sf.size.width, sf.size.height))
                        }
                    };
                    if let Some((sx, sy, sw, sh)) = screen_info {
                        let left_pad = 180.0;
                        let top_pad = 100.0;
                        let win_w = (current.size.width + left_pad).min(sw);
                        let win_h = (current.size.height + top_pad).min(sh);
                        // Keep bottom-right corner fixed (mascot stays there).
                        let mut x = current.origin.x + current.size.width - win_w;
                        let y = current.origin.y;
                        x = x.max(sx).min(sx + sw - win_w);
                        let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(win_w, win_h));
                        unsafe {
                            // Start with clicks passing through until the poll takes over.
                            let _: () = msg_send![obj, setIgnoresMouseEvents: true];
                            let _: () = msg_send![obj, setFrame: frame, display: true, animate: false];
                            let _: () = msg_send![obj, setLevel: 27isize];
                            let _: () = msg_send![obj, orderFrontRegardless];
                        }
                        if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                            *f = Some((x, y, win_w, win_h));
                        }
                    }
                }
            }).map_err(|e| e.to_string())?;
        }
        #[cfg(target_os = "windows")]
        {
            if let Ok(Some(monitor)) = win.current_monitor() {
                let scale = monitor.scale_factor();
                if let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) {
                    let current_x = pos.x as f64 / scale;
                    let current_y = pos.y as f64 / scale;
                    let current_w = size.width as f64 / scale;
                    let current_h = size.height as f64 / scale;
                    let sw = monitor.size().width as f64 / scale;
                    let sh = monitor.size().height as f64 / scale;
                    let left_pad = 180.0;
                    let top_pad = 100.0;
                    let win_w = (current_w + left_pad).min(sw);
                    let win_h = (current_h + top_pad).min(sh);
                    // Keep bottom-right corner fixed so mascot stays anchored.
                    let x = (current_x + current_w - win_w).max(0.0).min(sw - win_w);
                    let y = current_y.max(0.0).min(sh - win_h);
                    let _ = win.set_size(tauri::LogicalSize::new(win_w, win_h));
                    let _ = win.set_position(tauri::LogicalPosition::new(x, y));
                }
            }
            if !FULLSCREEN_HIDING.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = win.set_always_on_top(true);
                let _ = win.show();
            }
        }

        // Start the click-through poll thread.
        PET_PASSTHROUGH_ACTIVE.store(true, Ordering::SeqCst);
        #[cfg(target_os = "macos")]
        if !PET_PASSTHROUGH_THREAD_ALIVE.load(Ordering::SeqCst) {
            let app2 = app.clone();
            std::thread::spawn(move || pet_passthrough_poll(app2, mascot_scale, large_mascot_scale));
        }
        #[cfg(target_os = "windows")]
        if !PET_PASSTHROUGH_THREAD_ALIVE.load(Ordering::SeqCst) {
            let app2 = app.clone();
            std::thread::spawn(move || pet_passthrough_poll_windows(app2, mascot_scale, large_mascot_scale));
        }
    } else {
        // Stop the poll thread.
        PET_PASSTHROUGH_ACTIVE.store(false, Ordering::SeqCst);
        PET_CONTEXT_MENU_OPEN.store(false, Ordering::SeqCst);
        PET_POMODORO_ACTIVE.store(false, Ordering::SeqCst);

        // Shrink back to collapsed mascot size and re-enable mouse events.
        #[cfg(target_os = "macos")]
        {
            let win_clone = win.clone();
            app.run_on_main_thread(move || {
                use objc2::runtime::AnyObject;
                use objc2::msg_send;
                use objc2_foundation::{NSRect, NSPoint, NSSize};
                if let Ok(ns_win) = win_clone.ns_window() {
                    let obj = unsafe { &*(ns_win as *mut AnyObject) };
                    let current: NSRect = unsafe { msg_send![obj, frame] };
                    let (win_w, win_h) = large_collapsed_mascot_window_size(mascot_scale, large_mascot_scale);
                    // Collapse towards bottom-right corner.
                    let x = current.origin.x + current.size.width - win_w;
                    let y = current.origin.y;
                    let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(win_w, win_h));
                    unsafe {
                        let _: () = msg_send![obj, setIgnoresMouseEvents: false];
                        let _: () = msg_send![obj, setFrame: frame, display: true, animate: false];
                    }
                    if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                        *f = Some((x, y, win_w, win_h));
                    }
                }
            }).map_err(|e| e.to_string())?;
        }
        #[cfg(target_os = "windows")]
        {
            if let Ok(Some(monitor)) = win.current_monitor() {
                let scale = monitor.scale_factor();
                if let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) {
                    let current_x = pos.x as f64 / scale;
                    let current_y = pos.y as f64 / scale;
                    let current_w = size.width as f64 / scale;
                    let (win_w, win_h) = large_collapsed_mascot_window_size(mascot_scale, large_mascot_scale);
                    // Collapse towards bottom-right corner.
                    let x = current_x + current_w - win_w;
                    let y = current_y;
                    let _ = win.set_size(tauri::LogicalSize::new(win_w, win_h));
                    let _ = win.set_position(tauri::LogicalPosition::new(x, y));
                }
            }
        }
    }
    Ok(())
}

/// Tell the pet-mode pass-through poll whether a pomodoro timer is active.
/// When true, the entire mascot window stays interactive so the bottom-
/// anchored Pomodoro stop button receives clicks instead of having them
/// pass through (it sits in the centered hitbox's bottom inset region).
#[tauri::command]
async fn set_pet_pomodoro_active(active: bool) -> Result<(), String> {
    PET_POMODORO_ACTIVE.store(active, Ordering::SeqCst);
    Ok(())
}

/// Tell the pet-mode pass-through poll whether the context menu is open.
/// When `side` is `"right"` the window is widened rightward by 180 px
/// (left edge stays put).  The frontend sets the mascot CSS to
/// `right: 180` so it does not move on screen — it stays at exactly
/// the same pixel position.  Menu buttons render in the new 180 px area
/// via `overflow: visible` + `left: mascotSize + 14`.
#[tauri::command]
async fn set_pet_context_menu(app: tauri::AppHandle, open: bool, side: Option<String>) -> Result<(), String> {
    PET_CONTEXT_MENU_OPEN.store(open, Ordering::SeqCst);

    #[cfg(target_os = "macos")]
    {
        let right_pad = 180.0_f64;
        if open && side.as_deref() == Some("right") {
            if let Some(win) = app.get_webview_window("main") {
                let win_clone = win.clone();
                let (tx, rx) = std::sync::mpsc::channel::<()>();
                let _ = app.run_on_main_thread(move || {
                    use objc2::runtime::AnyObject;
                    use objc2::msg_send;
                    use objc2_foundation::{NSRect, NSPoint, NSSize};
                    if let Ok(ns_win) = win_clone.ns_window() {
                        let obj = unsafe { &*(ns_win as *mut AnyObject) };
                        let current: NSRect = unsafe { msg_send![obj, frame] };
                        if let Ok(mut saved) = PET_MENU_RESTORE_FRAME.lock() {
                            *saved = Some((
                                current.origin.x,
                                current.origin.y,
                                current.size.width,
                                current.size.height,
                            ));
                        }
                        // Widen rightward — left edge stays fixed, mascot
                        // keeps its screen position via CSS right: 180.
                        let new_w = current.size.width + right_pad;
                        let frame = NSRect::new(
                            NSPoint::new(current.origin.x, current.origin.y),
                            NSSize::new(new_w, current.size.height),
                        );
                        unsafe {
                            let _: () = msg_send![obj, setAlphaValue: 0.0f64];
                            let _: () = msg_send![obj, setFrame: frame, display: true, animate: false];
                        }
                        if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                            *f = Some((current.origin.x, current.origin.y, new_w, current.size.height));
                        }
                        pet_context_schedule_restore_alpha(ns_win as *mut std::ffi::c_void);
                    }
                    let _ = tx.send(());
                });
                let _ = rx.recv();
            }
        } else if !open {
            if let Some(win) = app.get_webview_window("main") {
                let win_clone = win.clone();
                let (tx, rx) = std::sync::mpsc::channel::<()>();
                let _ = app.run_on_main_thread(move || {
                    use objc2::runtime::AnyObject;
                    use objc2::msg_send;
                    use objc2_foundation::{NSRect, NSPoint, NSSize};
                    if let Ok(ns_win) = win_clone.ns_window() {
                        let obj = unsafe { &*(ns_win as *mut AnyObject) };
                        if let Ok(mut saved) = PET_MENU_RESTORE_FRAME.lock() {
                            if let Some((_x, _y, w, h)) = *saved {
                                let current: NSRect = unsafe { msg_send![obj, frame] };
                                let frame = NSRect::new(
                                    // Keep current position (user may have dragged while menu open),
                                    // only restore size.
                                    NSPoint::new(current.origin.x, current.origin.y),
                                    NSSize::new(w, h),
                                );
                                unsafe {
                                    let _: () = msg_send![obj, setAlphaValue: 0.0f64];
                                    let _: () = msg_send![obj, setFrame: frame, display: true, animate: false];
                                }
                                if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                                    *f = Some((current.origin.x, current.origin.y, w, h));
                                }
                                *saved = None;
                                pet_context_schedule_restore_alpha(ns_win as *mut std::ffi::c_void);
                            }
                        }
                    }
                    let _ = tx.send(());
                });
                let _ = rx.recv();
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let right_pad = 180.0_f64;
        if open && side.as_deref() == Some("right") {
            if let Some(win) = app.get_webview_window("main") {
                if let Ok(Some(monitor)) = win.current_monitor() {
                    let scale = monitor.scale_factor();
                    if let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) {
                        let current_x = pos.x as f64 / scale;
                        let current_y = pos.y as f64 / scale;
                        let current_w = size.width as f64 / scale;
                        let current_h = size.height as f64 / scale;
                        if let Ok(mut saved) = PET_MENU_RESTORE_FRAME.lock() {
                            if saved.is_none() {
                                *saved = Some((current_x, current_y, current_w, current_h));
                            }
                        }
                        // Widen rightward — left edge stays fixed, mascot keeps
                        // screen position via CSS right: 180.
                        let new_w = current_w + right_pad;
                        let _ = win.set_size(tauri::LogicalSize::new(new_w, current_h));
                        if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                            *f = Some((current_x, current_y, new_w, current_h));
                        }
                    }
                }
            }
        } else if !open {
            if let Some(win) = app.get_webview_window("main") {
                if let Ok(mut saved) = PET_MENU_RESTORE_FRAME.lock() {
                    if let Some((_x, _y, w, h)) = *saved {
                        let (current_x, current_y) = match (win.outer_position(), win.current_monitor()) {
                            (Ok(pos), Ok(Some(monitor))) => {
                                let scale = monitor.scale_factor();
                                (pos.x as f64 / scale, pos.y as f64 / scale)
                            }
                            _ => (0.0, 0.0),
                        };
                        let _ = win.set_size(tauri::LogicalSize::new(w, h));
                        if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                            *f = Some((current_x, current_y, w, h));
                        }
                        *saved = None;
                    }
                }
            }
        }
    }

    Ok(())
}



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
fn stop_event_was_interrupted(event: &serde_json::Value, session_source: &str, claude_status: &str) -> bool {
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

fn start_session_file_watcher(
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

fn stop_session_file_watcher(session_id: &str) {
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




/// Spawn a demo-mode mini mascot window. Each window runs the bundled
/// frontend with `?demo=1&pet=<id>` query params, which routes to a
/// minimal mascot-only React tree. Used by the dev-mode "演示模式" toggle
/// to drop multiple animated mascots on screen for demo recordings.
#[tauri::command]
async fn spawn_demo_mascot(app: tauri::AppHandle, pet_id: String) -> Result<String, String> {
    use std::sync::atomic::AtomicU64;
    static DEMO_COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = DEMO_COUNTER.fetch_add(1, Ordering::SeqCst);
    let label = format!("demo-mascot-{}", n);

    let url = format!("index.html#/mini?demo=1&pet={}", pet_id);
    let win = tauri::WebviewWindowBuilder::new(
        &app,
        label.clone(),
        tauri::WebviewUrl::App(url.into()),
    )
    .title("PawBae demo mascot")
    .inner_size(96.0, 96.0)
    .min_inner_size(96.0, 96.0)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .build()
    .map_err(|e| e.to_string())?;

    // Position the demo window in a known-good area near the top-right
    // of the screen, stepping each subsequent spawn by one collapsed
    // mascot width so they line up next to each other. Avoiding the
    // main mini window's frame keeps us correct even when the user is
    // currently in settings (where the main window is 600px wide and
    // would otherwise push the demos off-screen).
    const DEMO_STEP_W: f64 = 96.0;
    #[cfg(target_os = "macos")]
    {
        let win_clone = win.clone();
        let _ = app.run_on_main_thread(move || {
            use objc2::msg_send;
            use objc2::runtime::{AnyClass, AnyObject};
            use objc2_foundation::{NSPoint, NSRect, NSSize};
            if let Ok(demo_ns) = win_clone.ns_window() {
                let demo_obj = unsafe { &*(demo_ns as *mut AnyObject) };

                // Pull the active screen frame from NSScreen so we can
                // anchor relative to the visible area rather than guessing.
                let screen_frame: Option<NSRect> = unsafe {
                    AnyClass::get(c"NSScreen").and_then(|cls| {
                        let screens: *mut AnyObject = msg_send![cls, screens];
                        if screens.is_null() {
                            return None;
                        }
                        let count: usize = msg_send![&*screens, count];
                        if count == 0 {
                            return None;
                        }
                        let screen: *mut AnyObject = msg_send![&*screens, objectAtIndex: 0usize];
                        if screen.is_null() {
                            return None;
                        }
                        let frame: NSRect = msg_send![&*screen, frame];
                        Some(frame)
                    })
                };
                let Some(sf) = screen_frame else { return };

                // Right-aligned baseline anchor: ~120pt below the menu
                // bar on the right edge, then step left by one mascot
                // width per spawn.
                let baseline_x = sf.origin.x + sf.size.width - DEMO_STEP_W * 2.0;
                let baseline_y = sf.origin.y + sf.size.height - DEMO_STEP_W - MASCOT_TOP_INSET;
                let x = baseline_x - (n as f64) * DEMO_STEP_W;
                let new_origin = NSPoint::new(x.max(sf.origin.x), baseline_y);
                let new_frame = NSRect::new(new_origin, NSSize::new(DEMO_STEP_W, DEMO_STEP_W));

                unsafe {
                    let _: () = msg_send![demo_obj, setLevel: 27isize];
                    let _: () = msg_send![demo_obj, setFrame: new_frame, display: true, animate: false];
                    let behavior: usize = (1 << 0) | (1 << 4) | (1 << 8) | (1 << 6);
                    let _: () = msg_send![demo_obj, setCollectionBehavior: behavior];
                    let _: () = msg_send![demo_obj, setAcceptsMouseMovedEvents: true];
                }
            }
        });
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(Some(monitor)) = win.current_monitor() {
            let scale = monitor.scale_factor();
            let mp = monitor.position();
            let mx = mp.x as f64 / scale;
            let my = mp.y as f64 / scale;
            let sw = monitor.size().width as f64 / scale;
            let baseline_x = mx + sw - DEMO_STEP_W * 2.0;
            let baseline_y = my + MASCOT_TOP_INSET;
            let x = (baseline_x - (n as f64) * DEMO_STEP_W).max(mx);
            let _ = win.set_position(tauri::LogicalPosition::new(x, baseline_y));
        }
        let _ = win.set_always_on_top(true);
    }
    let _ = win.show();
    Ok(label)
}

/// Close a single spawned demo mascot window by label.
#[tauri::command]
async fn close_demo_mascot(app: tauri::AppHandle, label: String) -> Result<bool, String> {
    if !label.starts_with("demo-mascot-") {
        return Err("invalid demo mascot label".into());
    }
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.close();
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Close every spawned demo mascot window, leaving only the main mini.
#[tauri::command]
async fn close_demo_mascots(app: tauri::AppHandle) -> Result<u32, String> {
    let mut closed = 0u32;
    let labels: Vec<String> = app
        .webview_windows()
        .keys()
        .filter(|l| l.starts_with("demo-mascot-"))
        .cloned()
        .collect();
    for label in labels {
        if let Some(win) = app.get_webview_window(&label) {
            let _ = win.close();
            closed += 1;
        }
    }
    Ok(closed)
}

/// Open the platform's native folder picker so the user can choose a
/// codex pet directory to import. Returns the absolute path or `null` if
/// the user cancelled. Implemented with `osascript` on macOS and
/// PowerShell's `FolderBrowserDialog` on Windows so we don't need to add
/// `tauri-plugin-dialog` just for this one flow.
// macOS occasionally demotes our floating mini window back to the normal
// NSWindow level after a foreign helper (osascript, NSOpenPanel-driven
// pickers, etc.) takes focus. Re-apply level 27 (status) and reassert
// always-on-top so the mascot/settings panel stays on top of everything.
//
// All AppKit work is dispatched to the main thread — calling NSWindow
// methods from the Tauri command (runtime) thread trips AppKit's
// main-thread assertions and aborts the app with SIGTERM.
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

fn cwd_matches_workspace_root(cwd: &str, workspace_root: &str) -> bool {
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

fn resolve_cursor_window_binding(
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
pub(crate) use crate::platform::macos::{get_active_ghostty_terminal_id, get_frontmost_app_name};


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




#[tauri::command]
async fn install_claude_hooks() -> Result<(), String> {
    let home = dirs::home_dir().ok_or("no home dir")?;
    let claude_dir = home.join(".claude");
    let hooks_dir = claude_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir).map_err(|e| e.to_string())?;

    // Write hook script — platform-specific
    #[cfg(unix)]
    let hook_path = hooks_dir.join("ooclaw-hook.sh");
    #[cfg(windows)]
    let hook_path = hooks_dir.join("ooclaw-hook.ps1");

    #[cfg(unix)]
    {
        let hook_script = r#"#!/bin/bash
# ooclaw Claude Code hook - forwards events to /tmp/ooclaw-claude.sock
SOCKET_PATH="/tmp/ooclaw-claude.sock"
[ -S "$SOCKET_PATH" ] || exit 0

# Detect non-interactive (claude -p / --print) sessions
IS_INTERACTIVE=true
for CHECK_PID in $PPID $(ps -o ppid= -p $PPID 2>/dev/null | tr -d ' '); do
    if ps -o args= -p "$CHECK_PID" 2>/dev/null | grep -qE '(^| )(-p|--print)( |$)'; then
        IS_INTERACTIVE=false
        break
    fi
done
export OOCLAW_INTERACTIVE=$IS_INTERACTIVE
# $PPID is the PID of the process that spawned this bash (i.e. Claude Code).
# Forwarded to pawbae so it can detect when CC exits (Ctrl+C / SIGKILL)
# and clear stale "waiting" sessions.
export CC_PID=$PPID

# Capture Ghostty terminal ID once per CC session (cached per CC PID).
# The hook runs inside the CC terminal, so the focused tab is the right one.
_TID_CACHE="/tmp/ooclaw-tid-$PPID"
if [ -f "$_TID_CACHE" ]; then
    export GHOSTTY_TID=$(cat "$_TID_CACHE" 2>/dev/null)
else
    export GHOSTTY_TID=$(osascript -e 'try
tell application "Ghostty" to return id of first terminal of selected tab of front window as text
end try' 2>/dev/null || echo "")
    [ -n "$GHOSTTY_TID" ] && echo "$GHOSTTY_TID" > "$_TID_CACHE" 2>/dev/null
fi

/usr/bin/python3 -c "
import json, os, socket, sys

try:
    input_data = json.load(sys.stdin)
except:
    sys.exit(0)

hook_event = input_data.get('hook_event_name', '')

status_map = {
    'UserPromptSubmit': 'processing',
    'PreCompact': 'compacting',
    'SessionStart': 'waiting_for_input',
    'SessionEnd': 'ended',
    'PreToolUse': 'running_tool',
    'PostToolUse': 'processing',
    'PermissionRequest': 'waiting_for_input',
    'Stop': 'waiting_for_input',
    'SubagentStop': 'waiting_for_input',
}

output = {
    'sessionId': input_data.get('session_id', ''),
    'cwd': input_data.get('cwd', ''),
    'event': hook_event,
    'claudeStatus': input_data.get('status', status_map.get(hook_event, 'unknown')),
    'interactive': os.environ.get('OOCLAW_INTERACTIVE', 'true') == 'true',
    'pid': int(os.environ.get('CC_PID', '0')) or None,
}

# Ghostty terminal ID for precise tab jumping
_tid = os.environ.get('GHOSTTY_TID', '')
if _tid:
    output['terminalId'] = _tid

if hook_event == 'UserPromptSubmit':
    prompt = input_data.get('prompt', '')
    if prompt:
        output['userPrompt'] = prompt[:200]

tool = input_data.get('tool_name', '')
if tool:
    output['tool'] = tool

tool_input = input_data.get('tool_input', {})
if tool_input:
    # For Write/Edit, build a slim JSON with complete structure so the
    # frontend can parse it and show file name + numbered code lines.
    if tool in ('Write', 'Edit'):
        slim = {}
        if tool_input.get('file_path'):
            slim['file_path'] = tool_input['file_path']
        c = tool_input.get('content') or tool_input.get('new_string') or tool_input.get('old_string') or ''
        if c:
            slim['content'] = c[:5000]
        output['toolInput'] = json.dumps(slim)
    elif tool == 'Bash':
        slim = {}
        if tool_input.get('command'):
            slim['command'] = tool_input['command'][:500]
        if tool_input.get('description'):
            slim['description'] = tool_input['description'][:200]
        output['toolInput'] = json.dumps(slim)
    else:
        output['toolInput'] = json.dumps(tool_input)[:300]

if hook_event == 'Stop':
    msg = input_data.get('last_assistant_message', '')
    if msg:
        output['lastResponse'] = msg[:2000]

if hook_event == 'PermissionRequest':
    output['permission_suggestions'] = input_data.get('permission_suggestions', [])

try:
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect('$SOCKET_PATH')
    sock.sendall(json.dumps(output).encode())
    if hook_event == 'PermissionRequest':
        sock.shutdown(socket.SHUT_WR)
        response = b''
        while True:
            chunk = sock.recv(4096)
            if not chunk:
                break
            response += chunk
        sock.close()
        if response:
            sys.stdout.write(response.decode('utf-8', errors='replace'))
            sys.stdout.flush()
    else:
        sock.close()
except:
    pass
"
"#;
        std::fs::write(&hook_path, hook_script).map_err(|e| e.to_string())?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }

    #[cfg(windows)]
    {
        // Windows hook: uses PowerShell directly (no .cmd wrapper).
        // Claude Code runs hooks via /usr/bin/bash (Git Bash) on Windows,
        // so .cmd files and backslash paths don't work. We write a .ps1 file
        // and register the command as "powershell.exe ... -File '<forward-slash-path>'"
        // in settings.json so bash can invoke it correctly.
        // Simplified hook: forward raw CC JSON directly to the TCP server.
        // Do NOT parse/reconstruct JSON in PowerShell — large payloads (Stop events
        // with last_assistant_message containing full response text) get truncated by
        // [Console]::In.ReadToEnd(), breaking ConvertFrom-Json. The Rust side accepts
        // both processed (sessionId, event) and raw CC field names (session_id, hook_event_name).
        // Forward raw CC JSON to TCP. Use explicit Socket.Shutdown(Send) to ensure
        // the server receives EOF immediately — TcpClient.Dispose()/Close() alone on
        // Windows may delay the FIN packet, causing the server's read to hang or timeout
        // with incomplete data.
        let ps1_script = r#"$ErrorActionPreference = 'SilentlyContinue'
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
try {
    $raw = [Console]::In.ReadToEnd()
    if ([string]::IsNullOrWhiteSpace($raw)) { exit 0 }
    $ccPid = (Get-Process -Id $PID).Parent.Parent.Id
    if ($ccPid -and $raw.StartsWith('{')) {
        $raw = '{"pid":' + $ccPid + ',' + $raw.Substring(1)
    }
    $isPermission = $raw -match '"hook_event_name"\s*:\s*"PermissionRequest"'
    $client = [System.Net.Sockets.TcpClient]::new('127.0.0.1', 19283)
    $stream = $client.GetStream()
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($raw)
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Flush()
    $client.Client.Shutdown([System.Net.Sockets.SocketShutdown]::Send)
    if ($isPermission) {
        $reader = [System.IO.StreamReader]::new($stream, [System.Text.Encoding]::UTF8)
        $response = $reader.ReadToEnd()
        if ($response) {
            [Console]::Out.Write($response)
            [Console]::Out.Flush()
        }
        $reader.Close()
    }
    $client.Close()
} catch {}
"#;
        std::fs::write(&hook_path, ps1_script).map_err(|e| e.to_string())?;
    }

    // Update ~/.claude/settings.json to register hooks
    let settings_path = claude_dir.join("settings.json");
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // On Windows, Claude Code runs hooks via bash (Git Bash), so the command
    // must be bash-compatible. We call powershell.exe with forward-slash path.
    #[cfg(windows)]
    let hook_path_str = format!(
        "powershell.exe -NoProfile -ExecutionPolicy Bypass -File '{}'",
        hook_path.to_string_lossy().replace('\\', "/")
    );
    #[cfg(not(windows))]
    let hook_path_str = hook_path.to_string_lossy().to_string();
    let hooks = settings.as_object_mut().ok_or("settings not object")?
        .entry("hooks").or_insert(serde_json::json!({}))
        .as_object_mut().ok_or("hooks not object")?;

    // Hook registration configs matching notchi's HookInstaller approach
    let hook_entry = serde_json::json!([{"type": "command", "command": hook_path_str}]);
    let without_matcher = vec![serde_json::json!({"hooks": hook_entry})];
    let with_matcher = vec![serde_json::json!({"matcher": "*", "hooks": hook_entry})];
    let pre_compact = vec![
        serde_json::json!({"matcher": "auto", "hooks": hook_entry}),
        serde_json::json!({"matcher": "manual", "hooks": hook_entry}),
    ];

    let hook_configs: Vec<(&str, &Vec<serde_json::Value>)> = vec![
        ("UserPromptSubmit", &without_matcher),
        ("PreToolUse", &with_matcher),
        ("PostToolUse", &with_matcher),
        ("PermissionRequest", &with_matcher),
        ("PreCompact", &pre_compact),
        ("Stop", &without_matcher),
        ("SubagentStop", &without_matcher),
        ("SessionStart", &without_matcher),
        ("SessionEnd", &without_matcher),
    ];

    // Detect both old (.cmd path) and new (powershell.exe ... .ps1) hook entries for cleanup
    let has_our_hook = |entry: &serde_json::Value| -> bool {
        let is_ours = |cmd: &str| -> bool {
            cmd == hook_path_str || cmd.contains("ooclaw-hook")
        };
        entry.get("command").and_then(|c| c.as_str()).map_or(false, |c| is_ours(c))
        || entry.get("hooks").and_then(|hs| hs.as_array()).map_or(false, |hs| {
            hs.iter().any(|inner| inner.get("command").and_then(|c| c.as_str()).map_or(false, |c| is_ours(c)))
        })
    };

    for (event, configs) in hook_configs {
        let event_hooks = hooks.entry(event).or_insert(serde_json::json!([]));
        let arr = event_hooks.as_array_mut().ok_or("not array")?;
        arr.retain(|h| !has_our_hook(h));
        for config in configs {
            arr.push(config.clone());
        }
    }

    std::fs::write(&settings_path, serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    // Keep Codex desktop integration in sync with Claude integration.
    // Frontend still invokes `install_claude_hooks`, so we install both
    // hook systems here to avoid requiring frontend API changes.
    install_codex_hooks().await?;

    Ok(())
}

async fn install_codex_hooks() -> Result<(), String> {
    let home = dirs::home_dir().ok_or("no home dir")?;
    let codex_dir = home.join(".Codex");
    let hooks_dir = codex_dir.join("hooks");

    // Codex support is dropped on Windows. Same as the cursor branch above:
    // proactively delete any previously-installed hook script and strip our
    // entries from hooks.json so the codex CLI cannot reach the pawbae
    // socket on this machine anymore.
    #[cfg(windows)]
    {
        let _ = std::fs::remove_file(hooks_dir.join("ooclaw-codex-hook.ps1"));
        // Codex's home is conventionally `.codex` on Windows but the install
        // path used `.Codex` historically — the file system is case-
        // insensitive so we clean the same dir, but also catch the
        // lowercase variant explicitly in case both ever exist.
        let alt = home.join(".codex").join("hooks").join("ooclaw-codex-hook.ps1");
        if alt.exists() {
            let _ = std::fs::remove_file(&alt);
        }
        for hooks_json_path in [codex_dir.join("hooks.json"), home.join(".codex").join("hooks.json")] {
            if !hooks_json_path.exists() { continue; }
            let Ok(content) = std::fs::read_to_string(&hooks_json_path) else { continue; };
            let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&content) else { continue; };
            if let Some(hooks) = config.get_mut("hooks").and_then(|v| v.as_object_mut()) {
                let event_names: Vec<String> = hooks.keys().cloned().collect();
                for name in event_names {
                    if let Some(arr) = hooks.get_mut(&name).and_then(|v| v.as_array_mut()) {
                        arr.retain(|entry| {
                            let cmd_match = entry.get("command").and_then(|c| c.as_str())
                                .map(|c| c.contains("ooclaw-codex-hook"))
                                .unwrap_or(false);
                            let nested_match = entry.get("hooks").and_then(|hs| hs.as_array())
                                .map(|hs| hs.iter().any(|inner| {
                                    inner.get("command").and_then(|c| c.as_str())
                                        .map(|c| c.contains("ooclaw-codex-hook"))
                                        .unwrap_or(false)
                                }))
                                .unwrap_or(false);
                            !(cmd_match || nested_match)
                        });
                        if arr.is_empty() {
                            hooks.remove(&name);
                        }
                    }
                }
            }
            if let Ok(json_str) = serde_json::to_string_pretty(&config) {
                let _ = std::fs::write(&hooks_json_path, json_str);
            }
        }
        log::info!("[codex_hooks] codex support disabled on windows; cleaned previously installed hooks");
        return Ok(());
    }

    #[cfg(not(windows))]
    std::fs::create_dir_all(&hooks_dir).map_err(|e| e.to_string())?;

    #[cfg(unix)]
    let hook_path = hooks_dir.join("ooclaw-codex-hook.sh");
    #[cfg(windows)]
    let hook_path = hooks_dir.join("ooclaw-codex-hook.ps1");

    #[cfg(unix)]
    {
        let hook_script = r#"#!/bin/bash
# ooclaw Codex hook - forwards events to /tmp/ooclaw-claude.sock
SOCKET_PATH="/tmp/ooclaw-claude.sock"
[ -S "$SOCKET_PATH" ] || { echo '{}'; exit 0; }
export CC_PID=$PPID

# Capture Ghostty terminal ID once per Codex process so stop-time active-tab
# checks and click-to-jump can target the exact tab.
_TID_CACHE="/tmp/ooclaw-tid-$PPID"
if [ -f "$_TID_CACHE" ]; then
    export GHOSTTY_TID=$(cat "$_TID_CACHE" 2>/dev/null)
else
    export GHOSTTY_TID=$(osascript -e 'try
tell application "Ghostty" to return id of first terminal of selected tab of front window as text
end try' 2>/dev/null || echo "")
    [ -n "$GHOSTTY_TID" ] && echo "$GHOSTTY_TID" > "$_TID_CACHE" 2>/dev/null
fi

/usr/bin/python3 -c "
import json, os, socket, sys

raw = sys.stdin.read()
if not raw.strip():
    print('{}')
    sys.exit(0)

try:
    data = json.loads(raw)
except:
    print('{}')
    sys.exit(0)

if not isinstance(data, dict):
    print('{}')
    sys.exit(0)

if not data.get('source'):
    data['source'] = 'codex'

if not data.get('pid'):
    try:
        pid = int(os.environ.get('CC_PID', '0'))
        if pid > 0:
            data['pid'] = pid
    except:
        pass

tid = os.environ.get('GHOSTTY_TID', '')
if tid and not data.get('terminalId'):
    data['terminalId'] = tid

hook_event = data.get('hook_event_name') or data.get('event') or data.get('codex_event_type') or ''
if hook_event and not data.get('hook_event_name'):
    data['hook_event_name'] = hook_event

# Codex may omit cwd in some events. Fall back to process cwd so session
# records still have a stable workspace path.
if not data.get('cwd') and not data.get('workdir'):
    try:
        data['cwd'] = os.getcwd()
    except:
        pass

payload = json.dumps(data)

try:
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect('$SOCKET_PATH')
    sock.sendall(payload.encode('utf-8'))

    if hook_event == 'PermissionRequest':
        sock.shutdown(socket.SHUT_WR)
        response = b''
        while True:
            chunk = sock.recv(4096)
            if not chunk:
                break
            response += chunk
        sock.close()
        if response:
            sys.stdout.write(response.decode('utf-8', errors='replace'))
        else:
            sys.stdout.write('{}')
    else:
        sock.shutdown(socket.SHUT_WR)
        sock.close()
        sys.stdout.write('{}')
except:
    sys.stdout.write('{}')
"
"#;
        std::fs::write(&hook_path, hook_script).map_err(|e| e.to_string())?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }

    #[cfg(windows)]
    {
        // On Windows, keep the hook simple: forward Codex JSON to the existing
        // pawbae TCP hook server. `process_claude_event` handles both Codex
        // and Claude field variants.
        let ps1_script = r#"$ErrorActionPreference = 'SilentlyContinue'
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
try {
    $raw = [Console]::In.ReadToEnd()
    if ([string]::IsNullOrWhiteSpace($raw)) {
        [Console]::Out.Write('{}')
        exit 0
    }

    $obj = $null
    try { $obj = $raw | ConvertFrom-Json } catch {}
    if ($obj -ne $null) {
        $ccPid = (Get-Process -Id $PID).Parent.Parent.Id
        if (-not $obj.source) { $obj.source = 'codex' }
        if ($ccPid -and -not $obj.pid) { $obj | Add-Member -NotePropertyName pid -NotePropertyValue $ccPid -Force }
        if (-not $obj.hook_event_name -and $obj.codex_event_type) { $obj.hook_event_name = $obj.codex_event_type }
        if (-not $obj.cwd -and -not $obj.workdir) { $obj.cwd = (Get-Location).Path }
        $raw = $obj | ConvertTo-Json -Compress -Depth 20
    }

    $hookName = ''
    if ($obj -ne $null -and $obj.hook_event_name) { $hookName = [string]$obj.hook_event_name }

    $client = [System.Net.Sockets.TcpClient]::new('127.0.0.1', 19283)
    $stream = $client.GetStream()
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($raw)
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Flush()
    $client.Client.Shutdown([System.Net.Sockets.SocketShutdown]::Send)

    if ($hookName -eq 'PermissionRequest') {
        $reader = [System.IO.StreamReader]::new($stream, [System.Text.Encoding]::UTF8)
        $response = $reader.ReadToEnd()
        if ($response) { [Console]::Out.Write($response) } else { [Console]::Out.Write('{}') }
        $reader.Close()
    } else {
        [Console]::Out.Write('{}')
    }
    [Console]::Out.Flush()
    $client.Close()
} catch {
    try { [Console]::Out.Write('{}'); [Console]::Out.Flush() } catch {}
}
"#;
        std::fs::write(&hook_path, ps1_script).map_err(|e| e.to_string())?;
    }

    let hooks_json_path = codex_dir.join("hooks.json");
    let mut config: serde_json::Value = if hooks_json_path.exists() {
        let content = std::fs::read_to_string(&hooks_json_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    if config.get("hooks").is_none() {
        config["hooks"] = serde_json::json!({});
    }
    let hooks = config["hooks"].as_object_mut().ok_or("hooks is not an object")?;

    #[cfg(windows)]
    let hook_command = format!(
        "powershell.exe -NoProfile -ExecutionPolicy Bypass -File '{}'",
        hook_path.to_string_lossy().replace('\\', "/"),
    );
    #[cfg(not(windows))]
    let hook_command = hook_path.to_string_lossy().to_string();

    let has_our_hook = |entry: &serde_json::Value| -> bool {
        let is_ours = |cmd: &str| -> bool {
            cmd == hook_command || cmd.contains("ooclaw-codex-hook")
        };
        entry.get("command").and_then(|c| c.as_str()).map_or(false, |c| is_ours(c))
            || entry.get("hooks").and_then(|hs| hs.as_array()).map_or(false, |hs| {
                hs.iter().any(|inner| inner.get("command").and_then(|c| c.as_str()).map_or(false, |c| is_ours(c)))
            })
    };

    let hook_def = serde_json::json!({"type": "command", "command": hook_command});
    let event_names = [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "PermissionRequest",
        "Stop",
        "StopFailure",
        "SubagentStop",
    ];
    for event_name in event_names {
        let arr = hooks.entry(event_name.to_string()).or_insert(serde_json::json!([]));
        let list = arr.as_array_mut().ok_or("hook event is not an array")?;
        list.retain(|entry| !has_our_hook(entry));
        list.push(serde_json::json!({"hooks": [hook_def.clone()]}));
    }

    let json_str = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&hooks_json_path, json_str).map_err(|e| e.to_string())?;

    Ok(())
}

fn codex_requires_escalation(event: &serde_json::Value) -> bool {
    fn read_bool(v: &serde_json::Value, keys: &[&str]) -> bool {
        keys.iter()
            .filter_map(|k| v.get(k))
            .any(|x| x.as_bool().unwrap_or(false))
    }

    fn read_string<'a>(v: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
        keys.iter().find_map(|k| v.get(k).and_then(|x| x.as_str()))
    }

    fn has_explicit_escalation_markers(v: &serde_json::Value) -> bool {
        let sandbox_mode = read_string(v, &["sandbox_permissions", "sandboxPermissions"])
            .unwrap_or("");
        if sandbox_mode.eq_ignore_ascii_case("require_escalated")
            || sandbox_mode.eq_ignore_ascii_case("escalated")
        {
            return true;
        }
        if read_bool(
            v,
            &[
                "with_escalated_permissions",
                "withEscalatedPermissions",
                "requires_approval",
                "requiresApproval",
                "approval_required",
                "approvalRequired",
            ],
        ) {
            return true;
        }
        let justification = read_string(v, &["justification"]).unwrap_or("").trim();
        !justification.is_empty()
    }

    fn parse_tool_input(event: &serde_json::Value) -> Option<serde_json::Value> {
        let tool_input = event.get("tool_input").or_else(|| event.get("toolInput"))?;
        if tool_input.is_object() {
            return Some(tool_input.clone());
        }
        if let Some(raw) = tool_input.as_str() {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(raw) {
                return Some(parsed);
            }
        }
        None
    }

    // Hard guard: this helper exists only for Codex events. CC's
    // PreToolUse payload may carry overlapping field names (e.g. a future
    // CC release adding a `justification` field), and previous iterations
    // of the looser checks below already mis-classified CC's Bash calls
    // as needing approval. Bail out immediately for anything that isn't
    // unambiguously a Codex event so the function name and behaviour
    // stay aligned, no matter what gets added inside it later.
    let is_codex_event = event.get("turn_id").is_some()
        || read_string(event, &["source"]).unwrap_or("").eq_ignore_ascii_case("codex");
    if !is_codex_event {
        return false;
    }

    // Preferred path: explicit approval/escalation fields.
    if has_explicit_escalation_markers(event) {
        return true;
    }
    let parsed_tool_input = parse_tool_input(event);
    if let Some(tool_input) = parsed_tool_input.as_ref() {
        if has_explicit_escalation_markers(tool_input) {
            return true;
        }
    }

    // Fallback for Codex payloads that omit explicit flags:
    // PreToolUse(Bash) in default permission mode with an obvious
    // out-of-workspace write command almost always means approval UI.
    let tool_name = read_string(event, &["tool", "tool_name"]).unwrap_or("");
    let permission_mode = read_string(event, &["permission_mode", "permissionMode"]).unwrap_or("");
    if !(tool_name == "Bash" && permission_mode == "default") {
        return false;
    }

    let command = parsed_tool_input
        .as_ref()
        .and_then(|ti| read_string(ti, &["command"]))
        .unwrap_or("");
    if command.is_empty() {
        return false;
    }
    command.contains("$HOME/")
        || command.contains("/Users/")
        || command.contains("Desktop/")
        || command.contains(" cat > ")
        || command.contains(" > ")
        || command.contains("<<'EOF'")
        || command.contains("<<EOF")
}

fn is_codex_internal_utility_event(event: &serde_json::Value) -> bool {
    let permission_mode = event.get("permission_mode")
        .or_else(|| event.get("permissionMode"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if permission_mode != "bypassPermissions" {
        return false;
    }

    let prompt = event.get("prompt")
        .or_else(|| event.get("userPrompt"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if prompt.starts_with("You are a helpful assistant. You will be presented with a user prompt") {
        return true;
    }

    let transcript_is_null = event.get("transcript_path").map(|v| v.is_null()).unwrap_or(false);
    let source = event.get("source").and_then(|v| v.as_str()).unwrap_or("");
    let model = event.get("model").and_then(|v| v.as_str()).unwrap_or("");
    if transcript_is_null && (source == "startup" || model == "gpt-5.4-mini") {
        return true;
    }

    let last_message = event.get("last_assistant_message")
        .or_else(|| event.get("codex_last_assistant_message"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start();
    if last_message.starts_with("{\"title\":") {
        return true;
    }

    false
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
fn process_claude_event(
    buf: &str,
    state: &Arc<Mutex<HashMap<String, ClaudeSession>>>,
    app: &tauri::AppHandle,
    source_override: Option<&str>,
) -> Option<(String, String)> {
    log::info!("[claude_event] raw buf len={} content={}", buf.len(), &buf[..buf.len().min(500)]);
    if let Ok(event) = serde_json::from_str::<serde_json::Value>(buf) {
        // Accept both processed field names (sessionId, event, claudeStatus) from the old
        // hook format AND raw CC field names (session_id, hook_event_name, status).
        // On Windows the hook now forwards raw CC JSON directly to avoid truncation issues
        // with large payloads (Stop events contain last_assistant_message with full response text).
        let session_id = event.get("sessionId")
            .or_else(|| event.get("session_id"))
            .or_else(|| event.get("conversation_id"))
            .and_then(|v| v.as_str()).unwrap_or("").to_string();
        if session_id.is_empty() { log::warn!("[claude_event] empty sessionId, ignoring"); return None; }

        let raw_hook_event = event.get("event")
            .or_else(|| event.get("hook_event_name"))
            .or_else(|| event.get("codex_event_type"))
            .and_then(|v| v.as_str()).unwrap_or("").to_string();
        // Normalize Cursor's camelCase event names to CC's PascalCase.
        // Cursor and CC have different hook event sets:
        //   Cursor: beforeSubmitPrompt, stop, beforeShellExecution, afterShellExecution,
        //           beforeMCPExecution, afterMCPExecution, afterFileEdit, beforeReadFile,
        //           afterAgentThought, afterAgentResponse
        //   CC:     UserPromptSubmit, Stop, PreToolUse, PostToolUse, SessionStart, etc.
        let hook_event = match raw_hook_event.as_str() {
            "beforeSubmitPrompt" => "UserPromptSubmit".to_string(),
            "hook-user-prompt-submit" => "UserPromptSubmit".to_string(),
            "sessionStart" => "SessionStart".to_string(),
            "sessionEnd" => "SessionEnd".to_string(),
            "agentStop" => "Stop".to_string(),
            "StopFailure" | "stopFailure" => "Stop".to_string(),
            "preToolUse" => "PreToolUse".to_string(),
            "postToolUse" | "postToolUseFailure" => "PostToolUse".to_string(),
            "subagentStart" => "PreToolUse".to_string(),
            "subagentStop" => "SubagentStop".to_string(),
            "preCompact" => "PreCompact".to_string(),
            // Cursor-specific tool events → map to PreToolUse/PostToolUse
            "beforeShellExecution" | "beforeMCPExecution" | "beforeReadFile" => "PreToolUse".to_string(),
            "afterShellExecution" | "afterMCPExecution" | "afterFileEdit" => "PostToolUse".to_string(),
            "afterAgentThought" | "afterAgentResponse" => "PostToolUse".to_string(),
            "stop" => "Stop".to_string(),
            other => other.to_string(),
        };

        // Codex desktop may emit internal utility sessions (for example title
        // generation). These should not appear in the session list or trigger
        // completion notifications.
        if is_codex_internal_utility_event(&event) {
            if let Ok(mut sessions) = state.lock() {
                sessions.remove(&session_id);
            }
            stop_session_file_watcher(&session_id);
            log::info!(
                "[claude_event] ignore internal codex utility session={} event={}",
                session_id,
                hook_event
            );
            return None;
        }

        let claude_status = event.get("claudeStatus").or_else(|| event.get("status"))
            .and_then(|v| v.as_str()).unwrap_or("unknown").to_string();

        let is_processing = claude_status != "waiting_for_input";

        let user_prompt = event.get("userPrompt").or_else(|| event.get("prompt"))
            .and_then(|v| v.as_str()).unwrap_or("");
        let is_local_slash = if user_prompt.starts_with('/') {
            let cmd = user_prompt.split_whitespace().next().unwrap_or("");
            matches!(cmd, "/clear" | "/compact" | "/help" | "/cost" | "/status" | "/vim" | "/fast" | "/model" | "/login" | "/logout")
        } else { false };

        let pretool_needs_waiting = hook_event == "PreToolUse" && codex_requires_escalation(&event);
        let mut status = match hook_event.as_str() {
            "UserPromptSubmit" => {
                if is_local_slash { "stopped".to_string() } else { "processing".to_string() }
            }
            "PreCompact" => "compacting".to_string(),
            "PreToolUse" => {
                let tool = event.get("tool")
                    .or_else(|| event.get("tool_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                // Different clients may report interactive choice tools with
                // slightly different names. Treat both as waiting states so
                // the selection popup can be shown consistently.
                if tool == "AskUserQuestion" || tool == "AskQuestion" || pretool_needs_waiting {
                    "waiting".to_string()
                } else {
                    "tool_running".to_string()
                }
            }
            "PostToolUse" => "processing".to_string(),
            "Stop" => "stopped".to_string(),
            "SubagentStop" => "processing".to_string(),
            "SessionEnd" => "ended".to_string(),
            "PermissionRequest" => "waiting".to_string(),
            "SessionStart" => {
                if is_processing { "processing".to_string() } else { "stopped".to_string() }
            }
            _ => {
                if !is_processing { "stopped".to_string() } else { claude_status.clone() }
            }
        };

        // Guard: if CC's own status is "waiting_for_input" but our event-derived
        // status says "processing"/"tool_running", something is out of sync.
        // Override to "stopped" — EXCEPT for UserPromptSubmit, where CC's status
        // field may still say "waiting_for_input" because the hook fires before
        // CC's internal state transitions. A new prompt always means processing.
        if !is_processing
            && matches!(status.as_str(), "processing" | "tool_running")
            && hook_event != "UserPromptSubmit"
        {
            log::info!("[claude_event] guard override: {} → stopped (is_processing=false)", status);
            status = "stopped".to_string();
        }
        log::info!("[claude_event] session={} event={} claude_status={} is_processing={} → final_status={}",
            &session_id[..session_id.len().min(8)], hook_event, claude_status, is_processing, status);

        let was_processing;
        let was_compacting;
        let pending_agents;
        let session_source: String;
        let stop_was_interrupted;

        {
            let mut sessions = state.lock().unwrap();
            let prev_status = sessions.get(&session_id).map(|s| s.status.clone()).unwrap_or_default();
            was_processing = matches!(prev_status.as_str(), "processing" | "tool_running" | "compacting");
            was_compacting = prev_status == "compacting";

            if hook_event == "SessionEnd" {
                session_source = sessions.get(&session_id).map(|s| s.source.clone()).unwrap_or_else(|| "cc".to_string());
                sessions.remove(&session_id);
                pending_agents = 0;
                stop_was_interrupted = false;
            } else {
                // Determine source: explicit override from socket server, or from JSON, or default "cc"
                let source = source_override
                    .map(|s| s.to_string())
                    .or_else(|| event.get("source").and_then(|v| v.as_str()).map(|s| s.to_string()))
                    .unwrap_or_else(|| "cc".to_string());
                let session = sessions.entry(session_id.clone()).or_insert_with(|| ClaudeSession {
                    session_id: session_id.clone(),
                    cwd: String::new(),
                    status: "idle".to_string(),
                    tool: None,
                    tool_input: None,
                    user_prompt: None,
                    interactive: true,
                    updated_at: 0,
                    is_processing: false,
                    pid: None,
                    pending_agents: 0,
                    last_response: None,
                    last_failure: false,
                    is_active_tab: false,
                    source: source.clone(),
                    permission_suggestions: None,
                    terminal_id: None,
                    host_terminal: None,
                    cursor_port: None,
                    cursor_workspace_root: None,
                    cursor_workspace_name: None,
                    cursor_native_handle: None,
                });
                // Only upgrade source, never downgrade:
                // cc < codex < cursor.
                // Once a session is identified as codex/cursor, later generic
                // CC events (source=cc) for the same sessionId must not
                // overwrite it, otherwise active-tab/staleness logic regresses.
                let source_rank = |s: &str| -> u8 {
                    match s {
                        "cc" => 1,
                        "codex" => 2,
                        "cursor" => 3,
                        _ => 0,
                    }
                };
                if source_rank(&source) >= source_rank(&session.source) {
                    session.source = source.clone();
                }

                // Track pending sub-agents:
                // - PreToolUse with tool=Agent → a sub-agent is being launched
                // - SubagentStop → a sub-agent has completed
                // Sound only plays on Stop when pending_agents == 0 (all agents done).
                let tool_name = event.get("tool").or_else(|| event.get("tool_name"))
                    .and_then(|v| v.as_str()).unwrap_or("");
                if hook_event == "UserPromptSubmit" {
                    // New user prompt = fresh start. Reset counter in case previous
                    // agents were killed or SubagentStop was never delivered.
                    session.pending_agents = 0;
                } else if (hook_event == "PreToolUse" && tool_name == "Agent") || raw_hook_event == "subagentStart" {
                    session.pending_agents += 1;
                    log::info!("[claude_event] session={} Agent launched, pending_agents={}",
                        &session_id[..session_id.len().min(8)], session.pending_agents);
                } else if hook_event == "SubagentStop" {
                    session.pending_agents = session.pending_agents.saturating_sub(1);
                    log::info!("[claude_event] session={} SubagentStop, pending_agents={}",
                        &session_id[..session_id.len().min(8)], session.pending_agents);
                }

                session.status = status.clone();
                session.is_processing = is_processing;
                let incoming_cwd = event.get("cwd")
                    .or_else(|| event.get("workdir"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !incoming_cwd.is_empty() || session.cwd.is_empty() {
                    session.cwd = incoming_cwd.to_string();
                }
                session.interactive = event.get("interactive").and_then(|v| v.as_bool()).unwrap_or(true);
                session.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;

                if session.source == "cursor" && !session.cwd.is_empty() {
                    // Cursor hook payloads do not expose a stable window ID or terminal PID.
                    // Instead we bind the session to the extension port whose workspace roots
                    // best match the session cwd. We do this on first sighting and whenever a
                    // new prompt starts so a re-opened / re-focused window can rebind cleanly.
                    let needs_rebind = hook_event == "UserPromptSubmit"
                        || session.cursor_port.is_none()
                        || session.cursor_workspace_root.as_ref()
                            .map(|root| !cwd_matches_workspace_root(&session.cwd, root))
                            .unwrap_or(false);

                    if needs_rebind {
                        if let Some(binding) = resolve_cursor_window_binding(
                            &session.cwd,
                            session.cursor_port,
                            session.cursor_native_handle.as_deref(),
                        ) {
                            if session.cursor_port != Some(binding.port)
                                || session.cursor_workspace_root.as_deref() != Some(binding.workspace_root.as_str()) {
                                log::info!(
                                    "[cursor_bind] session={} port={} workspace_root={} workspace_name={} native_handle={:?}",
                                    &session_id[..session_id.len().min(8)],
                                    binding.port,
                                    binding.workspace_root,
                                    binding.workspace_name,
                                    binding.native_handle,
                                );
                            }
                            session.cursor_port = Some(binding.port);
                            session.cursor_workspace_root = Some(binding.workspace_root);
                            session.cursor_workspace_name = Some(binding.workspace_name);
                            session.cursor_native_handle = binding.native_handle;
                        } else {
                            log::info!(
                                "[cursor_bind] session={} unresolved cwd={}",
                                &session_id[..session_id.len().min(8)],
                                session.cwd,
                            );
                        }
                    }
                }

                if let Some(t) = event.get("tool").or_else(|| event.get("tool_name")).and_then(|v| v.as_str()) {
                    if !t.is_empty() { session.tool = Some(t.to_string()); }
                }
                if let Some(tool_input_val) = event.get("toolInput").or_else(|| event.get("tool_input")) {
                    let tool_input_text = tool_input_val
                        .as_str()
                        .map(|s| s.to_string())
                        .or_else(|| serde_json::to_string(tool_input_val).ok());
                    if let Some(t) = tool_input_text {
                        if !t.is_empty() {
                            session.tool_input = Some(t);
                        }
                    }
                }
                if let Some(t) = event.get("userPrompt")
                    .or_else(|| event.get("prompt"))
                    .and_then(|v| v.as_str()) {
                    if !t.is_empty() { session.user_prompt = Some(t.to_string()); }
                }
                // Store CC process PID from hook event for stale-session detection
                if let Some(p) = event.get("pid").and_then(|v| v.as_u64()) {
                    let pid_u32 = p as u32;
                    session.pid = Some(pid_u32);
                    #[cfg(target_os = "macos")]
                    if session.host_terminal.is_none() && session.source != "cursor" {
                        session.host_terminal = find_terminal_app_for_pid(pid_u32);
                        log::info!("[claude_event] session={} host_terminal={:?}",
                            &session_id[..session_id.len().min(8)], session.host_terminal);
                        if session.source == "cc"
                            && session
                                .host_terminal
                                .as_deref()
                                .map(is_codex_host_terminal)
                                .unwrap_or(false)
                        {
                            session.source = "codex".to_string();
                        }
                    }
                }

                // Store Ghostty terminal ID from hook event for precise tab jumping.
                // The hook captures this from inside the CC terminal, so it's
                // always the correct tab — even for pre-existing sessions.
                if session.terminal_id.is_none() {
                    if let Some(tid) = event.get("terminalId").and_then(|v| v.as_str()) {
                        if !tid.is_empty() {
                            log::info!("[claude_event] session={} stored terminal_id={}",
                                &session_id[..session_id.len().min(8)], tid);
                            session.terminal_id = Some(tid.to_string());
                        }
                    }
                }

                if hook_event == "Stop" || hook_event == "SubagentStop" {
                    session.tool = None;
                    session.tool_input = None;
                }

                // Store AI's last response for the completion reminder popup.
                // Clear on new prompt so stale responses don't linger.
                //
                // For Cursor: afterAgentResponse fires before stop and carries
                // the actual response text. We stash it here so the Stop handler
                // can use it instead of a placeholder.
                if raw_hook_event == "afterAgentResponse" {
                    if let Some(resp) = event.get("lastResponse").and_then(|v| v.as_str()) {
                        if !resp.is_empty() {
                            session.last_response = Some(resp.to_string());
                        }
                    }
                }

                // Check at Stop time (real-time, not polling) whether the user
                // is already looking at this terminal tab. If so, skip setting
                // last_response so the completion popup never triggers.
                if hook_event == "Stop" {
                    let interrupted = stop_event_was_interrupted(&event, &session.source, &claude_status);
                    let failed_stop = interrupted
                        || matches!(raw_hook_event.as_str(), "StopFailure" | "stopFailure")
                        || event.get("failure").and_then(|v| v.as_bool()).unwrap_or(false)
                        || event.get("failed").and_then(|v| v.as_bool()).unwrap_or(false)
                        || event.get("error").is_some();
                    session.last_failure = failed_stop;
                    // CC: check if the user is looking at this session's Ghostty tab
                    // Cursor: check if Cursor (or PawBae) is the frontmost app.
                    // If a terminal ID is missing (older hooks / non-Ghostty),
                    // fall back to host-terminal checks where available.
                    let frontmost = get_frontmost_app_name();
                    let is_ghostty_session = matches!(
                        session.host_terminal.as_deref(),
                        Some("Ghostty" | "ghostty")
                    );
                    let is_tab_active = if session.source == "cursor" {
                        is_cursor_frontmost_app(&frontmost)
                    } else if session.source == "codex" {
                        let ghostty_match = is_ghostty_session
                            && session.terminal_id.as_ref()
                                .and_then(|tid| get_active_ghostty_terminal_id().map(|a| a == *tid))
                                .unwrap_or(false);
                        ghostty_match || is_codex_frontmost_app(&frontmost)
                    } else if is_ghostty_session {
                        session.terminal_id.as_ref()
                            .and_then(|tid| get_active_ghostty_terminal_id().map(|a| a == *tid))
                            .unwrap_or(false)
                    } else if let Some(ht) = session.host_terminal.as_deref() {
                        frontmost_matches_host_terminal(&frontmost, ht)
                    } else {
                        false
                    };
                    if is_tab_active || interrupted {
                        session.last_response = None;
                    } else {
                        // Prefer lastResponse from the event itself (CC's Stop has it),
                        // then fall back to any value pre-stored by afterAgentResponse,
                        // then use a placeholder for Cursor/Codex so the popup
                        // still triggers when stop payload omits assistant text.
                        let resp_from_event = event.get("lastResponse")
                            .or_else(|| event.get("last_assistant_message"))
                            .or_else(|| event.get("codex_last_assistant_message"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        if resp_from_event.is_some() {
                            session.last_response = resp_from_event;
                        } else if session.last_response.is_none()
                            && (session.source == "cursor" || session.source == "codex")
                        {
                            session.last_response = Some("✓".to_string());
                        }
                        // else: keep existing last_response from afterAgentResponse
                    }
                    stop_was_interrupted = interrupted;
                } else if hook_event == "UserPromptSubmit" {
                    session.last_response = None;
                    session.last_failure = false;
                    stop_was_interrupted = false;
                } else {
                    stop_was_interrupted = false;
                }

                if hook_event == "PermissionRequest" {
                    session.permission_suggestions = event.get("permission_suggestions")
                        .or_else(|| event.get("permissionSuggestions"))
                        .cloned();
                } else {
                    session.permission_suggestions = None;
                }

                pending_agents = session.pending_agents;
                session_source = session.source.clone();
            }
        }

        let _ = app.emit("claude-session-update", &session_id);

        // Only emit completion sound on explicit Stop or PermissionRequest events.
        // Previously we checked status transitions, but guard overrides on PostToolUse
        // could falsely trigger "stopped" mid-task when CC's status field lags behind.
        // Also suppress sound while sub-agents are still running (pending_agents > 0).
        // Each PreToolUse(Agent) increments the counter, each SubagentStop decrements it.
        // Sound only plays when all sub-agents have completed.
        let is_wait_event = hook_event == "PermissionRequest"
            || (hook_event == "PreToolUse" && status == "waiting");
        let is_completion_stop = hook_event == "Stop" && pending_agents == 0 && !stop_was_interrupted;
        if was_processing && !was_compacting
            && (is_completion_stop || is_wait_event) {
            let is_waiting = is_wait_event;
            let _ = app.emit("claude-task-complete", serde_json::json!({"sessionId": session_id, "waiting": is_waiting, "source": session_source}));
        }

        let cwd_str = event.get("cwd")
            .or_else(|| event.get("workdir"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        log::info!("[claude_event] session={} event={} status={} cwd={}", session_id, hook_event, status, cwd_str);
        if hook_event == "UserPromptSubmit" {
            if let Some(jsonl_path) = resolve_session_jsonl_path(&session_id, Some(&cwd_str)) {
                log::info!(
                    "[claude_event] session file path: {} exists={}",
                    jsonl_path.display(),
                    jsonl_path.exists()
                );
                if jsonl_path.exists() {
                    start_session_file_watcher(
                        session_id.clone(),
                        jsonl_path,
                        state.clone(),
                        app.clone(),
                    );
                }
            }
        } else if hook_event == "Stop" || hook_event == "SubagentStop" || hook_event == "SessionEnd" {
            stop_session_file_watcher(&session_id);
        }

        return Some((session_id, hook_event));
    } else if let Err(e) = serde_json::from_str::<serde_json::Value>(buf) {
        let tail: String = buf.chars().rev().take(300).collect::<String>().chars().rev().collect();
        log::warn!("[claude_event] JSON parse failed: err={}, len={}, tail=...{}", e, buf.len(), tail);
    }
    None
}

// ─── Cursor Integration ───────────────────────────────────────────────

/// Install hooks for Cursor IDE.
/// Creates ~/.cursor/hooks/occlaw-cursor-hook.sh and registers it in
/// ~/.cursor/hooks.json for all Cursor hook events.
#[tauri::command]
async fn install_cursor_hooks() -> Result<(), String> {
    let home = dirs::home_dir().ok_or("no home dir")?;
    let cursor_dir = home.join(".cursor");
    let hooks_dir = cursor_dir.join("hooks");

    // Cursor support is dropped on Windows. Instead of installing hooks we
    // actively clean up anything a previous pawbae build might have left
    // behind so the user can really stop hearing the completion sound.
    #[cfg(windows)]
    {
        let _ = std::fs::remove_file(hooks_dir.join("occlaw-cursor-hook.ps1"));
        let hooks_json_path = cursor_dir.join("hooks.json");
        if hooks_json_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&hooks_json_path) {
                if let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(hooks) = config.get_mut("hooks").and_then(|v| v.as_object_mut()) {
                        let marker = "occlaw-cursor-hook";
                        // Strip any pawbae entry from every event bucket and
                        // drop now-empty buckets so the file stays tidy.
                        let event_names: Vec<String> = hooks.keys().cloned().collect();
                        for name in event_names {
                            if let Some(arr) = hooks.get_mut(&name).and_then(|v| v.as_array_mut()) {
                                arr.retain(|entry| {
                                    !entry.get("command").and_then(|c| c.as_str())
                                        .map(|c| c.contains(marker))
                                        .unwrap_or(false)
                                });
                                if arr.is_empty() {
                                    hooks.remove(&name);
                                }
                            }
                        }
                    }
                    if let Ok(json_str) = serde_json::to_string_pretty(&config) {
                        let _ = std::fs::write(&hooks_json_path, json_str);
                    }
                }
            }
        }
        let ext_dir = home.join(".cursor").join("extensions").join("pawbae.terminal-focus-1.0.0");
        if ext_dir.exists() {
            let _ = std::fs::remove_dir_all(&ext_dir);
        }
        log::info!("[cursor_hooks] cursor support disabled on windows; cleaned previously installed hooks");
        return Ok(());
    }

    #[cfg(not(windows))]
    std::fs::create_dir_all(&hooks_dir).map_err(|e| e.to_string())?;

    // ── Write hook script (Unix) ──
    #[cfg(unix)]
    {
        let socket_path = "/tmp/occlaw-cursor.sock";
        let hook_script = format!(r##"#!/bin/bash
# occlaw Cursor hook — forwards events to {socket}
SOCKET_PATH="{socket}"
[ -S "$SOCKET_PATH" ] || {{ echo '{{}}'; exit 0; }}
export CC_PID=$PPID

/usr/bin/python3 -c "
import json, os, socket, sys

try:
    input_data = json.load(sys.stdin)
except:
    print('{{}}')
    sys.exit(0)

hook_event = input_data.get('hook_event_name', '')
if not hook_event:
    print('{{}}')
    sys.exit(0)

session_id = input_data.get('session_id', '') or input_data.get('conversation_id', '') or 'default'
cwd = input_data.get('cwd', '')
if not cwd:
    roots = input_data.get('workspace_roots', [])
    if roots:
        cwd = roots[0]

output = {{}}
output['sessionId'] = session_id
output['event'] = hook_event
output['source'] = 'cursor'
if cwd:
    output['cwd'] = cwd

# Map tool info — Cursor events use different field names than CC:
#   beforeShellExecution: command, cwd
#   beforeMCPExecution: tool_name, tool_input
#   afterFileEdit: file_path, edits
#   beforeReadFile: file_path, content
tool_name = input_data.get('tool_name', '')
if hook_event == 'beforeShellExecution' or hook_event == 'afterShellExecution':
    output['tool'] = 'Shell'
    cmd = input_data.get('command', '')
    if cmd:
        output['toolInput'] = json.dumps({{'command': cmd[:500]}})
elif hook_event in ('beforeMCPExecution', 'afterMCPExecution'):
    output['tool'] = tool_name or 'MCP'
    ti = input_data.get('tool_input', {{}})
    if ti:
        output['toolInput'] = json.dumps(ti)[:300]
elif hook_event == 'afterFileEdit':
    output['tool'] = 'Edit'
    fp = input_data.get('file_path', '')
    edits = input_data.get('edits', [])
    slim = {{}}
    if fp:
        slim['file_path'] = fp
    if edits:
        combined = '\\n'.join(e.get('new_string', '')[:1000] for e in edits[:3])
        slim['content'] = combined[:5000]
    output['toolInput'] = json.dumps(slim)
elif hook_event == 'beforeReadFile':
    output['tool'] = 'Read'
    fp = input_data.get('file_path', '')
    if fp:
        output['toolInput'] = json.dumps({{'file_path': fp}})
elif tool_name:
    output['tool'] = tool_name
    ti = input_data.get('tool_input', {{}})
    if ti:
        output['toolInput'] = json.dumps(ti)[:300]

# Stop event: extract status and last response
if hook_event == 'stop':
    status = input_data.get('status', '')
    if status:
        output['claudeStatus'] = status
    transcript_path = input_data.get('transcript_path', '')
    if transcript_path:
        output['transcript_path'] = transcript_path
    msg = input_data.get('last_assistant_message', '')
    if msg:
        output['lastResponse'] = msg[:2000]

# afterAgentResponse: Cursor sends the AI's response text here
# (stop event doesn't include it). Forward it so Rust can store it.
if hook_event == 'afterAgentResponse':
    text = input_data.get('text', '')
    if text:
        output['lastResponse'] = text[:2000]

# UserPromptSubmit: extract prompt text
if hook_event == 'beforeSubmitPrompt':
    prompt = input_data.get('prompt', '')
    if prompt:
        output['userPrompt'] = prompt[:200]

# PID for stale-session detection
cc_pid = os.environ.get('CC_PID', '')
if cc_pid:
    try:
        output['pid'] = int(cc_pid)
    except:
        pass

# Send to socket
try:
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect('$SOCKET_PATH')
    sock.sendall(json.dumps(output).encode())
    sock.shutdown(socket.SHUT_WR)
    sock.close()
except:
    pass

# Required stdout for Cursor:
#   beforeSubmitPrompt → gating hook, needs {{'continue': true}}
#   beforeShellExecution, beforeMCPExecution → permission hooks, need {{'permission': 'allow'}}
#   beforeReadFile → permission hook, needs {{'permission': 'allow'}}
#   everything else → {{}}
if hook_event == 'beforeSubmitPrompt':
    print(json.dumps({{'continue': True}}))
elif hook_event in ('beforeShellExecution', 'beforeMCPExecution', 'beforeReadFile'):
    print(json.dumps({{'permission': 'allow'}}))
else:
    print('{{}}')
"
"##, socket = socket_path);

        let hook_path = hooks_dir.join("occlaw-cursor-hook.sh");
        std::fs::write(&hook_path, hook_script).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
    }

    // ── Write hook script (Windows) ──
    #[cfg(windows)]
    {
        let hook_script = r#"$ErrorActionPreference = 'SilentlyContinue'
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
$raw = [Console]::In.ReadToEnd()
if (-not $raw) { Write-Output '{}'; exit 0 }
$ccPid = (Get-Process -Id $PID).Parent.Parent.Id
if ($ccPid -and $raw.StartsWith('{')) {
    $raw = '{"pid":' + $ccPid + ',"source":"cursor",' + $raw.Substring(1)
} else {
    $raw = '{"source":"cursor",' + $raw.Substring(1)
}
try {
    $client = [System.Net.Sockets.TcpClient]::new('127.0.0.1', 19284)
    $stream = $client.GetStream()
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($raw)
    $stream.Write($bytes, 0, $bytes.Length)
    $client.Client.Shutdown([System.Net.Sockets.SocketShutdown]::Send)
    $hookName = ($raw | ConvertFrom-Json).hook_event_name
    if ($hookName -eq 'beforeSubmitPrompt') {
        Write-Output '{"continue":true}'
    } elseif ($hookName -eq 'beforeShellExecution' -or $hookName -eq 'beforeMCPExecution' -or $hookName -eq 'beforeReadFile') {
        Write-Output '{"permission":"allow"}'
    } else {
        Write-Output '{}'
    }
    $client.Close()
} catch {
    Write-Output '{}'
}
"#;
        let hook_path = hooks_dir.join("occlaw-cursor-hook.ps1");
        std::fs::write(&hook_path, hook_script).map_err(|e| e.to_string())?;
    }

    // ── Register hooks in ~/.cursor/hooks.json ──
    let hooks_json_path = cursor_dir.join("hooks.json");
    let mut config: serde_json::Value = if hooks_json_path.exists() {
        let content = std::fs::read_to_string(&hooks_json_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    config["version"] = serde_json::json!(1);
    if config.get("hooks").is_none() {
        config["hooks"] = serde_json::json!({});
    }

    #[cfg(unix)]
    let hook_command = hooks_dir.join("occlaw-cursor-hook.sh").to_string_lossy().to_string();
    #[cfg(windows)]
    let hook_command = format!("powershell.exe -NoProfile -ExecutionPolicy Bypass -File '{}'",
        hooks_dir.join("occlaw-cursor-hook.ps1").to_string_lossy());

    // Cursor's actual supported hook events (as of 2026-04):
    // - beforeShellExecution, beforeMCPExecution: permission hooks (need {"permission":"allow"})
    // - afterFileEdit, beforeReadFile: notification hooks
    // - beforeSubmitPrompt: gating hook (needs {"continue":true})
    // - stop: notification hook
    // NOTE: preToolUse/postToolUse/sessionStart/sessionEnd/subagentStart/subagentStop
    // are NOT supported by Cursor — those are Claude Code events.
    let cursor_events = [
        "beforeSubmitPrompt", "stop",
        "beforeShellExecution", "afterShellExecution",
        "beforeMCPExecution", "afterMCPExecution",
        "afterFileEdit", "beforeReadFile",
        "afterAgentThought", "afterAgentResponse",
    ];
    let marker = "occlaw-cursor-hook";

    let hooks = config["hooks"].as_object_mut().ok_or("hooks is not an object")?;

    // Clean up our hook from old event names that Cursor doesn't actually support.
    // Previous versions incorrectly registered CC-only events like preToolUse, sessionStart, etc.
    let stale_events = [
        "sessionStart", "sessionEnd", "preToolUse", "postToolUse",
        "postToolUseFailure", "subagentStart", "subagentStop", "preCompact",
    ];
    for stale in &stale_events {
        if let Some(arr) = hooks.get_mut(*stale).and_then(|v| v.as_array_mut()) {
            arr.retain(|entry| {
                !entry.get("command").and_then(|c| c.as_str())
                    .map(|c| c.contains(marker))
                    .unwrap_or(false)
            });
        }
    }

    for event_name in &cursor_events {
        let arr = hooks.entry(event_name.to_string())
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .ok_or("hook event is not an array")?;

        let existing_idx = arr.iter().position(|entry| {
            entry.get("command").and_then(|c| c.as_str())
                .map(|c| c.contains(marker))
                .unwrap_or(false)
        });

        let entry = serde_json::json!({"command": hook_command});
        if let Some(idx) = existing_idx {
            arr[idx] = entry;
        } else {
            arr.push(entry);
        }
    }

    let json_str = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&hooks_json_path, json_str).map_err(|e| e.to_string())?;

    log::info!("[cursor_hooks] installed hooks to {:?}", hooks_json_path);

    // ── Sync pawbae terminal-focus extension for Cursor ──
    // The extension exposes a tiny localhost API per Cursor window:
    // - GET  /window-meta  → workspace roots + focus state + bound port
    // - POST /focus-window → surface that specific Cursor window
    // We intentionally overwrite the installed files on every startup so
    // extension changes take effect after the user reloads Cursor windows.
    let ext_id = "pawbae.terminal-focus";
    let ext_dir = home.join(".cursor").join("extensions").join(format!("{}-1.0.0", ext_id));
    log::info!("[cursor_hooks] syncing terminal-focus extension...");

    // Locate extension source with multiple fallbacks:
    // - repo/dev layout
    // - unpacked release binary layout
    // - macOS app bundle Resources
    let ext_source = {
        let mut candidates = Vec::new();

        if let Ok(exe) = std::env::current_exe() {
            let mut dir = exe.parent();
            for _ in 0..10 {
                if let Some(d) = dir {
                    let repo_candidate = d.join("extensions").join("cursor");
                    if repo_candidate.join("extension.js").exists() {
                        candidates.push(repo_candidate);
                        break;
                    }

                    let bundled_candidate = d.join("Resources").join("extensions").join("cursor");
                    if bundled_candidate.join("extension.js").exists() {
                        candidates.push(bundled_candidate);
                        break;
                    }

                    dir = d.parent();
                } else {
                    break;
                }
            }
        }

        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let repo_candidate = PathBuf::from(manifest_dir)
                .join("..")
                .join("..")
                .join("extensions")
                .join("cursor");
            if repo_candidate.join("extension.js").exists() {
                candidates.push(repo_candidate);
            }
        }

        candidates.into_iter().next()
    };

    if let Some(src) = ext_source {
        if let Err(e) = std::fs::create_dir_all(&ext_dir) {
            log::warn!("[cursor_hooks] failed to create extension dir: {}", e);
        } else {
            let files = ["package.json", "extension.js", "icon.png", "README.md"];
            let mut ok = true;
            for fname in &files {
                let from = src.join(fname);
                let to = ext_dir.join(fname);
                if let Err(e) = std::fs::copy(&from, &to) {
                    log::warn!("[cursor_hooks] failed to copy {}: {}", fname, e);
                    ok = false;
                }
            }
            if ok {
                // If the user previously uninstalled this extension in Cursor,
                // Cursor records it in ~/.cursor/extensions/.obsolete and keeps
                // hiding it even when files are copied back. Clear that flag.
                let obsolete_path = home.join(".cursor").join("extensions").join(".obsolete");
                let ext_folder_name = format!("{}-1.0.0", ext_id);
                if obsolete_path.exists() {
                    match std::fs::read_to_string(&obsolete_path) {
                        Ok(content) => {
                            if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(obj) = v.as_object_mut() {
                                    if obj.remove(&ext_folder_name).is_some() {
                                        match serde_json::to_string(obj) {
                                            Ok(s) => {
                                                if let Err(e) = std::fs::write(&obsolete_path, s) {
                                                    log::warn!("[cursor_hooks] failed to update .obsolete: {}", e);
                                                } else {
                                                    log::info!("[cursor_hooks] removed obsolete flag for {}", ext_folder_name);
                                                }
                                            }
                                            Err(e) => {
                                                log::warn!("[cursor_hooks] failed to serialize .obsolete: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("[cursor_hooks] failed to read .obsolete: {}", e);
                        }
                    }
                }

                // Ensure Cursor extension registry includes this local extension.
                // Some Cursor builds rely on extensions.json for listing/loading.
                let extensions_json_path = home.join(".cursor").join("extensions").join("extensions.json");
                let ext_version = "1.0.0";
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let registry_entry = serde_json::json!({
                    "identifier": { "id": ext_id },
                    "version": ext_version,
                    "location": {
                        "$mid": 1,
                        "path": ext_dir.to_string_lossy().to_string(),
                        "scheme": "file"
                    },
                    "relativeLocation": format!("{}-{}", ext_id, ext_version),
                    "metadata": {
                        "installedTimestamp": now_ms,
                        "pinned": false,
                        "source": "vsix"
                    }
                });
                let mut updated_registry = false;
                let mut registry_val: serde_json::Value = if extensions_json_path.exists() {
                    match std::fs::read_to_string(&extensions_json_path) {
                        Ok(content) => serde_json::from_str(&content).unwrap_or(serde_json::json!([])),
                        Err(_) => serde_json::json!([]),
                    }
                } else {
                    serde_json::json!([])
                };
                if !registry_val.is_array() {
                    registry_val = serde_json::json!([]);
                }
                if let Some(arr) = registry_val.as_array_mut() {
                    let mut found = false;
                    for item in arr.iter_mut() {
                        let item_id = item.get("identifier")
                            .and_then(|v| v.get("id"))
                            .and_then(|v| v.as_str());
                        if item_id == Some(ext_id) {
                            *item = registry_entry.clone();
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        arr.push(registry_entry);
                    }
                    updated_registry = true;
                }
                if updated_registry {
                    match serde_json::to_string(&registry_val) {
                        Ok(s) => {
                            if let Err(e) = std::fs::write(&extensions_json_path, s) {
                                log::warn!("[cursor_hooks] failed to update extensions.json: {}", e);
                            } else {
                                log::info!("[cursor_hooks] registered extension {} in extensions.json", ext_id);
                            }
                        }
                        Err(e) => {
                            log::warn!("[cursor_hooks] failed to serialize extensions.json: {}", e);
                        }
                    }
                }
                log::info!("[cursor_hooks] terminal-focus extension synced at {:?}", ext_dir);
            }
        }
    } else {
        log::warn!("[cursor_hooks] extension source not found, skipping sync");
    }

    Ok(())
}

/// Start the Cursor IPC server.
/// On macOS/Linux: Unix domain socket at /tmp/occlaw-cursor.sock
/// On Windows: TCP server on localhost:19284
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
