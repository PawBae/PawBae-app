//! Tauri SSH commands: key info, reset, close, and local file reads.

#[cfg(unix)]
use std::path::PathBuf;

use crate::ssh_core::{close_ssh_master, ssh_backoff_reset};
use crate::state::{ssh_backoff_map, ssh_key_map};

#[cfg(target_os = "windows")]
use crate::platform::windows::win_ssh_mux;

/// Returns the SSH key path that was used to authenticate a connection,
/// or null if unknown (e.g. socket was already established before this session).
#[tauri::command]
pub fn get_ssh_key_info(ssh_host: String, ssh_user: String) -> Option<String> {
    let key = format!("{}@{}", ssh_user, ssh_host);
    ssh_key_map().lock().unwrap().get(&key).cloned()
}
/// Reset backoff, gracefully close the existing SSH master process, and
/// remove the socket — so the next connection starts completely fresh.
/// Called before user-initiated "test connection" to avoid making the user
/// wait out a backoff timer or fight a stale/conflicting master process.
#[tauri::command]
pub async fn reset_ssh(ssh_host: String, ssh_user: String) {
    let host_key = format!("{}@{}", ssh_user, ssh_host);
    ssh_backoff_reset(&host_key);
    // Gracefully shut down the existing master process via `-O exit`,
    // then remove the socket file. This prevents orphaned ssh processes
    // from piling up and conflicting with the new master.
    let _ = close_ssh_master(&ssh_host, &ssh_user).await;
    // Clear cached key info since we're starting fresh
    ssh_key_map().lock().unwrap().remove(&host_key);
    log::info!(
        "[reset_ssh] cleared backoff, killed master, and reset for {}",
        host_key
    );
}
#[tauri::command]
pub async fn close_ssh(ssh_host: Option<String>, ssh_user: Option<String>) -> Result<(), String> {
    let sh = ssh_host.unwrap_or_default();
    let su = ssh_user.unwrap_or_default();
    if sh.is_empty() || su.is_empty() {
        // Clean up all stale SSH sockets/markers
        #[cfg(unix)]
        let scan_dir = PathBuf::from("/tmp");
        #[cfg(windows)]
        let scan_dir = std::env::temp_dir();

        if let Ok(mut entries) = tokio::fs::read_dir(&scan_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("pawbae-ssh-") {
                    let _ = tokio::fs::remove_file(entry.path()).await;
                    log::info!("[close_ssh] removed stale socket/marker: {}", name);
                }
            }
        }
        #[cfg(windows)]
        {
            win_ssh_mux::kill_all().await;
        }
        // Clear all backoff entries
        ssh_backoff_map().lock().unwrap().clear();
        return Ok(());
    }
    close_ssh_master(&sh, &su).await
}
#[tauri::command]
pub async fn read_local_file(path: String) -> Result<String, String> {
    use base64::Engine;
    let data = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("read failed: {e}"))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&data))
}
