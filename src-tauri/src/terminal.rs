//! Terminal/frontmost-app detection helpers.

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
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
            match handle {
                Ok(h) => {
                    let _ = CloseHandle(h);
                    true
                }
                Err(_) => false,
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn get_active_ghostty_terminal_id() -> Option<String> {
    None
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn get_frontmost_app_name() -> String {
    String::new()
}

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
