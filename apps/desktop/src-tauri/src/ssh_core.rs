//! SSH ControlMaster wrapper: backoff, ensure_master, exec, read_file, is_agent_active.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::agent_gateway::check_agent_active_from_lines;
use crate::app_init::home_dir_string;
use crate::state::{lock_or_recover, SshBackoffState, SshState};

#[cfg(target_os = "windows")]
use crate::platform::windows::{hide_window_tokio_cmd, win_ssh_mux};

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn ssh_backoff_remaining(ssh: &SshState, host_key: &str) -> Option<u64> {
    let map = lock_or_recover(&ssh.backoff);
    let state = map.get(host_key)?;
    if state.fail_count == 0 {
        return None;
    }
    let cooldown = std::cmp::min(15u64 * 2u64.pow(state.fail_count.saturating_sub(1)), 300);
    let elapsed = unix_now().saturating_sub(state.fail_epoch);
    if elapsed < cooldown {
        Some(cooldown - elapsed)
    } else {
        None
    }
}

fn ssh_backoff_record_failure(ssh: &SshState, host_key: &str) {
    let mut map = lock_or_recover(&ssh.backoff);
    let state = map.entry(host_key.to_string()).or_insert(SshBackoffState {
        fail_count: 0,
        fail_epoch: 0,
    });
    state.fail_count += 1;
    state.fail_epoch = unix_now();
}

pub(crate) fn ssh_backoff_reset(ssh: &SshState, host_key: &str) {
    let mut map = lock_or_recover(&ssh.backoff);
    map.remove(host_key);
}
/// Get the SSH control socket path for a given host.
/// On macOS/Linux: /tmp/pawbae-ssh-user@host:22
/// On Windows: returns a path in %TEMP% (used only as a "marker" since ControlMaster
/// is not supported; the marker file tracks whether a connection was recently validated).
fn ssh_control_path(ssh_user: &str, ssh_host: &str) -> String {
    #[cfg(unix)]
    {
        format!("/tmp/pawbae-ssh-{}@{}:22", ssh_user, ssh_host)
    }
    #[cfg(windows)]
    {
        let temp = std::env::temp_dir();
        temp.join(format!("pawbae-ssh-{}@{}.marker", ssh_user, ssh_host))
            .to_string_lossy()
            .to_string()
    }
}
/// Ensure an SSH ControlMaster socket is established (called once, reused by all ssh_exec).
/// On Windows, ControlMaster is not available — we just validate the connection once
/// and create a marker file. Each ssh_exec call will open its own SSH connection.
/// Implements exponential backoff on connection failure (15s, 30s, 60s, … capped at 300s)
/// to avoid flooding the server with reconnection attempts.
async fn ensure_ssh_master(ssh: &SshState, ssh_host: &str, ssh_user: &str) -> Result<(), String> {
    let host_key = format!("{}@{}", ssh_user, ssh_host);
    if let Some(remaining) = ssh_backoff_remaining(ssh, &host_key) {
        return Err(format!(
            "SSH connection to {} backing off, retry in {}s",
            host_key, remaining
        ));
    }

    let control_path = ssh_control_path(ssh_user, ssh_host);
    // Fast path: socket/marker already exists, reuse the master connection.
    if std::path::Path::new(&control_path).exists() {
        return Ok(());
    }

    // Per-host lock so only one task establishes the master at a time.
    use std::sync::OnceLock;
    use tokio::sync::Mutex as TokioMutex;
    static LOCKS: OnceLock<Mutex<HashMap<String, std::sync::Arc<TokioMutex<()>>>>> =
        OnceLock::new();
    let lock = {
        let mut locks = lock_or_recover(LOCKS.get_or_init(|| Mutex::new(HashMap::new())));
        locks
            .entry(host_key.clone())
            .or_insert_with(|| Arc::new(TokioMutex::new(())))
            .clone()
    };
    let _guard = lock.lock().await;
    // Re-check after acquiring the lock
    if std::path::Path::new(&control_path).exists() {
        return Ok(());
    }

    #[cfg(unix)]
    {
        let cp = format!("ControlPath={}", control_path);
        let child = tokio::process::Command::new("ssh")
            .args([
                "-o",
                "StrictHostKeyChecking=no",
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=10",
                "-o",
                "ControlMaster=yes",
                "-o",
                &cp,
                "-o",
                "ControlPersist=600",
                "-o",
                "ServerAliveInterval=15",
                "-o",
                "ServerAliveCountMax=3",
                "-fN",
                &host_key,
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("ssh master spawn: {}", e))?;

        let child_id = child.id();

        let result =
            tokio::time::timeout(std::time::Duration::from_secs(15), child.wait_with_output())
                .await;

        let output = match result {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => {
                ssh_backoff_record_failure(ssh, &host_key);
                return Err(format!("ssh master wait: {}", e));
            }
            Err(_) => {
                if let Some(pid) = child_id {
                    unsafe {
                        libc::kill(pid as i32, libc::SIGKILL);
                    }
                }
                ssh_backoff_record_failure(ssh, &host_key);
                return Err(format!("ssh master to {} timed out after 15s", host_key));
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            ssh_backoff_record_failure(ssh, &host_key);
            let count = lock_or_recover(&ssh.backoff)
                .get(&host_key)
                .map(|s| s.fail_count)
                .unwrap_or(0);
            let code = output
                .status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".into());
            log::warn!(
                "[ssh] connection to {} failed (attempt {}), entering backoff",
                host_key,
                count
            );
            return Err(format!("SSH master failed [exit {}]: {}", code, stderr));
        }

        // Wait for the socket file to appear
        for _ in 0..30 {
            if std::path::Path::new(&control_path).exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        if !std::path::Path::new(&control_path).exists() {
            ssh_backoff_record_failure(ssh, &host_key);
            return Err(format!("ssh master socket for {} never appeared", host_key));
        }
    }

    #[cfg(windows)]
    {
        // Windows: use persistent SSH subprocess multiplexer instead of per-command
        // connections. This avoids the TCP+SSH handshake overhead on every call and
        // prevents hitting server-side MaxStartups limits.
        if let Err(e) = win_ssh_mux::ensure(ssh_user, ssh_host).await {
            ssh_backoff_record_failure(ssh, &host_key);
            let count = lock_or_recover(&ssh.backoff)
                .get(&host_key)
                .map(|s| s.fail_count)
                .unwrap_or(0);
            log::warn!(
                "[ssh] connection to {} failed (attempt {}), entering backoff",
                host_key,
                count
            );
            return Err(format!("SSH connection failed: {}", e));
        }
        // Create marker file so the fast-path check at the top works.
        let _ = std::fs::write(&control_path, "connected");
    }

    // Detect which key was used by querying ssh config for this host.
    let mut ssh_g_cmd = tokio::process::Command::new("ssh");
    ssh_g_cmd
        .args(["-G", &host_key])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    hide_window_tokio_cmd(&mut ssh_g_cmd);
    if let Ok(cfg_output) = ssh_g_cmd.output().await {
        let cfg = String::from_utf8_lossy(&cfg_output.stdout);
        for line in cfg.lines() {
            if let Some(path) = line.strip_prefix("identityfile ") {
                let expanded = path.replace("~", &home_dir_string());
                if std::path::Path::new(&expanded).exists() {
                    log::info!("[ssh] {} will use key: {}", host_key, expanded);
                    lock_or_recover(&ssh.key_used).insert(host_key.clone(), expanded);
                    break;
                }
            }
        }
    }

    ssh_backoff_reset(ssh, &host_key);
    Ok(())
}
/// Execute a command on remote host via SSH.
/// On macOS/Linux: reuses ControlMaster socket for fast multiplexed connections.
/// On Windows: routes through a persistent SSH subprocess (win_ssh_mux) so all
///   commands share a single TCP connection instead of opening one per call.
/// If the command fails (e.g. stale socket), removes the socket and retries once.
pub(crate) async fn ssh_exec(
    ssh: &SshState,
    ssh_host: &str,
    ssh_user: &str,
    cmd: &str,
) -> Result<String, String> {
    ensure_ssh_master(ssh, ssh_host, ssh_user).await?;
    let safe_cmd = format!(
        "export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:$PATH && {}",
        cmd
    );

    #[cfg(windows)]
    {
        match win_ssh_mux::exec(ssh_user, ssh_host, &safe_cmd).await {
            Ok(out) => Ok(out),
            Err(e)
                if e.contains("transport error")
                    || e.contains("connection lost")
                    || e.contains("process exited")
                    || e.contains("not connected")
                    || e.contains("timed out") =>
            {
                log::warn!("[ssh] transport error, removing marker and retrying: {}", e);
                let _ = tokio::fs::remove_file(&ssh_control_path(ssh_user, ssh_host)).await;
                ensure_ssh_master(ssh, ssh_host, ssh_user).await?;
                return win_ssh_mux::exec(ssh_user, ssh_host, &safe_cmd).await;
            }
            Err(e) => Err(e),
        }
    }

    #[cfg(unix)]
    {
        let target = format!("{}@{}", ssh_user, ssh_host);
        let control_path = ssh_control_path(ssh_user, ssh_host);
        let cp = format!("ControlPath={}", control_path);

        let mut ssh_args: Vec<&str> =
            vec!["-o", "BatchMode=yes", "-o", "ConnectTimeout=5", "-o", &cp];
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
            if !stderr.trim().is_empty() {
                msg.push_str(&format!("\nstderr: {}", stderr.trim()));
            }
            if !stdout.trim().is_empty() {
                msg.push_str(&format!("\nstdout: {}", stdout.trim()));
            }
            return Err(msg);
        }

        log::warn!("[ssh] transport error (exit 255), removing stale socket and retrying");
        let _ = tokio::fs::remove_file(&control_path).await;
        ensure_ssh_master(ssh, ssh_host, ssh_user).await?;

        let mut ssh_args2: Vec<&str> =
            vec!["-o", "BatchMode=yes", "-o", "ConnectTimeout=5", "-o", &cp];
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
            let code = output
                .status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".into());
            let mut msg = format!("ssh cmd failed [exit {}]", code);
            if !stderr.trim().is_empty() {
                msg.push_str(&format!("\nstderr: {}", stderr.trim()));
            }
            if !stdout.trim().is_empty() {
                msg.push_str(&format!("\nstdout: {}", stdout.trim()));
            }
            return Err(msg);
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
/// Close an active SSH ControlMaster socket (macOS/Linux) or persistent mux subprocess (Windows).
pub(crate) async fn close_ssh_master(
    ssh: &SshState,
    ssh_host: &str,
    ssh_user: &str,
) -> Result<(), String> {
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
    ssh_backoff_reset(ssh, &format!("{}@{}", ssh_user, ssh_host));
    log::info!(
        "[close_ssh_master] closed socket for {}@{}",
        ssh_user,
        ssh_host
    );
    Ok(())
}
pub(crate) async fn ssh_read_file(
    ssh: &SshState,
    ssh_host: &str,
    ssh_user: &str,
    path: &str,
) -> Result<String, String> {
    let escaped = path.replace('"', r#"\""#);
    ssh_exec(ssh, ssh_host, ssh_user, &format!("cat \"{}\"", escaped)).await
}

pub(crate) async fn ssh_is_agent_active(
    ssh: &SshState,
    ssh_host: &str,
    ssh_user: &str,
    agent_id: &str,
) -> bool {
    let agent_dir = if agent_id.is_empty() {
        "main"
    } else {
        agent_id
    };
    let cmd = format!(
        "f=$(ls -t $HOME/.openclaw/agents/{}/sessions/*.jsonl 2>/dev/null | head -1); [ -f \"$f\" ] && tail -5 \"$f\"",
        agent_dir
    );
    let output = match ssh_exec(ssh, ssh_host, ssh_user, &cmd).await {
        Ok(s) => s,
        Err(_) => return false,
    };
    let lines: Vec<String> = output.lines().map(|l| l.to_string()).collect();
    check_agent_active_from_lines(&lines)
}
