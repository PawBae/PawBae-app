//! Cursor IDE bindings + the focus_cursor_terminal / jump_to_claude_terminal commands.

use std::path::PathBuf;

#[cfg(target_os = "macos")]
use crate::platform::macos::check_accessibility_permission;
#[cfg(not(target_os = "macos"))]
use crate::platform::common::check_accessibility_permission;
use crate::state::ClaudeState;

#[cfg(target_os = "macos")]
use crate::platform::macos::{
    activate_cursor_workspace_window, find_terminal_app_for_pid, get_tty_for_pid,
};

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
pub(crate) struct CursorWindowBinding {
    pub(crate) port: u16,
    pub(crate) workspace_root: String,
    pub(crate) workspace_name: String,
    pub(crate) native_handle: Option<String>,
}
/// Compute the JSONL session file path (matching notchi's ConversationParser.sessionFilePath)
pub(crate) fn claude_session_file_path(session_id: &str, cwd: &str) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_default();
    // On Windows, Claude Code replaces all of / \ : . with "-" when computing
    // the project directory name (e.g. G:\Desktop\code → G--Desktop-code).
    // The colon after the drive letter (G:) must also be replaced.
    #[cfg(windows)]
    let project_dir = cwd
        .replace('/', "-")
        .replace('\\', "-")
        .replace(':', "-")
        .replace('.', "-");
    #[cfg(not(windows))]
    let project_dir = cwd.replace('/', "-").replace('.', "-");
    home.join(".claude")
        .join("projects")
        .join(project_dir)
        .join(format!("{}.jsonl", session_id))
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
    let mut stream =
        TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(120)).ok()?;
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
    let status = headers
        .lines()
        .next()?
        .split_whitespace()
        .nth(1)?
        .parse::<u16>()
        .ok()?;

    let is_chunked = headers
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked");
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
    let request =
        format!("GET /window-meta HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
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

    log::info!(
        "[cursor_bind_resolve] cwd={} existing_port={:?} existing_handle={:?}",
        cwd,
        existing_port,
        existing_native_handle
    );

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

    log::info!(
        "[cursor_bind_resolve] {} candidates: {:?}",
        candidates.len(),
        candidates
            .iter()
            .map(|c| format!(
                "port={} score={} focused={} handle_match={} keep_existing={} handle={:?}",
                c.port, c.score, c.focused, c.handle_match, c.keep_existing, c.native_handle
            ))
            .collect::<Vec<_>>()
    );

    // If we have a native handle match, that wins unconditionally.
    if let Some(idx) = candidates.iter().position(|c| c.handle_match) {
        let c = &candidates[idx];
        log::info!(
            "[cursor_bind_resolve] → native handle match: port={}",
            c.port
        );
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
        b.score
            .cmp(&a.score)
            .then_with(|| b.focused.cmp(&a.focused))
            .then_with(|| a.port.cmp(&b.port))
    });

    let best = candidates.first()?;
    log::info!(
        "[cursor_bind_resolve] → best candidate: port={} score={} focused={}",
        best.port,
        best.score,
        best.focused
    );

    Some(CursorWindowBinding {
        port: best.port,
        workspace_root: best.workspace_root.clone(),
        workspace_name: best.workspace_name.clone(),
        native_handle: best.native_handle.clone(),
    })
}
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
pub async fn focus_cursor_terminal(
    session_id: String,
    state: tauri::State<'_, ClaudeState>,
) -> Result<String, String> {
    log::info!(
        "[focus_cursor] called for session={}",
        &session_id[..session_id.len().min(8)]
    );

    let ax_ok = check_accessibility_permission();
    log::info!("[focus_cursor] accessibility_permission={}", ax_ok);

    let (
        cwd,
        existing_port,
        existing_workspace_root,
        existing_workspace_name,
        existing_native_handle,
    ) = {
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
    let workspace_name = resolved_binding
        .as_ref()
        .map(|b| b.workspace_name.clone())
        .or(existing_workspace_name)
        .or_else(|| {
            existing_workspace_root
                .as_deref()
                .map(cursor_workspace_name_from_path)
        })
        .or_else(|| (!cwd.is_empty()).then(|| cursor_workspace_name_from_path(&cwd)))
        .unwrap_or_default();

    log::info!(
        "[focus_cursor] session={} cwd={} port={:?} workspace_name={}",
        &session_id[..session_id.len().min(8)],
        cwd,
        port,
        workspace_name
    );

    #[cfg(target_os = "macos")]
    activate_cursor_workspace_window(&workspace_name);

    if let Some(port) = port {
        let focused = post_cursor_window_action(port, "/focus-window", "{}");
        log::info!(
            "[focus_cursor] POST /focus-window to port {} → {}",
            port,
            focused
        );
        if focused {
            return Ok(format!("Focused Cursor window on port {}", port));
        }
        return Ok(format!(
            "Activated Cursor window but /focus-window failed on port {}",
            port
        ));
    }

    #[cfg(target_os = "macos")]
    activate_cursor_workspace_window(&workspace_name);

    Ok("Activated Cursor without a bound window".to_string())
}
/// Jump to the terminal running a Claude Code session.
/// Walks the parent process chain from the given PID to identify the terminal app,
/// then uses AppleScript (macOS) to activate and focus the matching window.
#[tauri::command]
pub async fn jump_to_claude_terminal(
    session_id: String,
    state: tauri::State<'_, ClaudeState>,
) -> Result<String, String> {
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
            let script = format!(
                r#"tell application "{}" to activate"#,
                app_name.replace('"', "\\\"")
            );
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
                if let Ok(out) = std::process::Command::new("osascript")
                    .args(["-e", &script])
                    .output()
                {
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
                    if let Ok(out) = std::process::Command::new("osascript")
                        .args(["-e", &fallback_script])
                        .output()
                    {
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
        let escaped_tty = tty
            .as_deref()
            .unwrap_or("")
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
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
                let escaped_tid = terminal_id
                    .as_deref()
                    .unwrap_or("")
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"");
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
