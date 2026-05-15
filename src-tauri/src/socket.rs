//! IPC socket servers + the resolve_claude_permission command.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::Manager;

use crate::commands::hook::process_claude_event;
use crate::state::{ClaudeSession, ClaudeState, PendingPermissions};

/// Spawn the Claude IPC socket server and (on non-Windows) the Cursor socket server.
pub(crate) fn init(app: &tauri::App) {
    let claude_state = app.state::<ClaudeState>();
    let sessions_arc = Arc::clone(&claude_state.sessions);
    let pending_arc = Arc::clone(&claude_state.pending_permissions);
    start_claude_socket_server(sessions_arc, pending_arc, app.handle().clone());

    // Cursor integration is disabled on Windows, so skip the server there.
    #[cfg(not(target_os = "windows"))]
    {
        let sessions_arc = Arc::clone(&claude_state.sessions);
        start_cursor_socket_server(sessions_arc, app.handle().clone());
    }
}

#[tauri::command]
pub async fn resolve_claude_permission(
    session_id: String,
    decision: String,
    state: tauri::State<'_, ClaudeState>,
) -> Result<(), String> {
    let tool_name = {
        let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
        sessions.get(&session_id).and_then(|s| s.tool.clone())
    };

    let response_json = match decision.as_str() {
        "deny" => serde_json::json!({
            "continue": true,
            "suppressOutput": true,
            "hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "decision": { "behavior": "deny" }
            }
        })
        .to_string(),
        "allow_once" => serde_json::json!({
            "continue": true,
            "suppressOutput": true,
            "hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "decision": { "behavior": "allow" }
            }
        })
        .to_string(),
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
            })
            .to_string()
        }
        "auto_approve" => serde_json::json!({
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
        })
        .to_string(),
        _ => return Err(format!("Unknown decision: {}", decision)),
    };

    let tx = {
        let mut map = state
            .pending_permissions
            .lock()
            .map_err(|e| e.to_string())?;
        map.remove(&session_id)
    };

    if let Some(tx) = tx {
        tx.send(response_json)
            .map_err(|_| "Failed to send permission response".to_string())?;
        log::info!(
            "[resolve_permission] sent '{}' for session={}",
            decision,
            &session_id[..session_id.len().min(8)]
        );
    } else {
        log::warn!(
            "[resolve_permission] no pending permission for session={}",
            &session_id[..session_id.len().min(8)]
        );
    }

    Ok(())
}
/// Process a Claude hook event (shared logic between Unix socket and TCP server).
/// Returns Some((session_id, hook_event)) if the event needs further handling
/// (e.g. PermissionRequest requires blocking the connection for a response).
#[cfg(not(target_os = "windows"))]
pub(crate) fn start_cursor_socket_server(
    claude_state: Arc<Mutex<HashMap<String, ClaudeSession>>>,
    app: tauri::AppHandle,
) {
    #[cfg(unix)]
    {
        let socket_path = "/tmp/occlaw-cursor.sock";
        let _ = std::fs::remove_file(socket_path);
        let listener = match std::os::unix::net::UnixListener::bind(socket_path) {
            Ok(l) => l,
            Err(e) => {
                log::warn!("[cursor_socket] bind failed: {}", e);
                return;
            }
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
            Err(e) => {
                log::warn!("[cursor_socket] TCP bind failed: {}", e);
                return;
            }
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
pub(crate) fn start_claude_socket_server(
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
                Err(e) => {
                    log::error!("Failed to bind claude socket: {}", e);
                    return;
                }
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
                            if let Some((session_id, hook_event)) =
                                process_claude_event(&buf, &state, &app, None)
                            {
                                if hook_event == "PermissionRequest" {
                                    let (tx, rx) = std::sync::mpsc::channel::<String>();
                                    {
                                        let mut map = pending.lock().unwrap();
                                        map.insert(session_id.clone(), tx);
                                    }
                                    log::info!(
                                        "[claude_socket] blocking for PermissionRequest session={}",
                                        &session_id[..session_id.len().min(8)]
                                    );
                                    match rx.recv_timeout(std::time::Duration::from_secs(600)) {
                                        Ok(response_json) => {
                                            log::info!("[claude_socket] sending permission response for session={}", &session_id[..session_id.len().min(8)]);
                                            let _ = s.write_all(response_json.as_bytes());
                                            let _ = s.flush();
                                        }
                                        Err(_) => {
                                            log::warn!(
                                                "[claude_socket] permission timeout for session={}",
                                                &session_id[..session_id.len().min(8)]
                                            );
                                        }
                                    }
                                    let mut map = pending.lock().unwrap();
                                    map.remove(&session_id);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        log::error!("Claude socket accept error: {}", e);
                    }
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
                Err(e) => {
                    log::error!("Failed to bind claude TCP socket: {}", e);
                    return;
                }
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
                            s.set_read_timeout(Some(std::time::Duration::from_secs(5)))
                                .ok();
                            let mut buf = Vec::new();
                            let mut chunk = [0u8; 4096];
                            loop {
                                match s.read(&mut chunk) {
                                    Ok(0) => break,
                                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                                    Err(e) => {
                                        if !buf.is_empty() {
                                            break;
                                        }
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
                            if text.contains("\"source\":\"codex\"")
                                || text.contains("\"source\": \"codex\"")
                            {
                                log::info!("[claude_tcp] dropping codex-originated event on windows (len={})", text.len());
                                return;
                            }
                            if let Some((session_id, hook_event)) =
                                process_claude_event(&text, &state, &app, None)
                            {
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
                                            log::warn!(
                                                "[claude_tcp] permission timeout for session={}",
                                                &session_id[..session_id.len().min(8)]
                                            );
                                        }
                                    }
                                    let mut map = pending.lock().unwrap();
                                    map.remove(&session_id);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        log::error!("Claude TCP accept error: {}", e);
                    }
                }
            }
        });
    }
}
