//! Windows-specific helpers. Gated by the outer `#[cfg(target_os = "windows")]` in `platform/mod.rs`.

use std::sync::atomic::Ordering;
use tauri::Manager;

use crate::mascot::large_collapsed_mascot_window_size;
#[allow(unused_imports)]
use crate::platform::common::*;
#[allow(unused_imports)]
use crate::state::*;

/// Apply CREATE_NO_WINDOW on Windows to prevent console popups from child processes.
pub(crate) fn hide_window_cmd(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}
/// Apply CREATE_NO_WINDOW on Windows to prevent console popups (tokio version).
pub(crate) fn hide_window_tokio_cmd(cmd: &mut tokio::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}
// ---------------------------------------------------------------------------
// Windows SSH multiplexer — a persistent `ssh -T` subprocess per host
// that serialises commands over stdin/stdout, avoiding the per-exec overhead
// of a full TCP+SSH handshake (Windows lacks ControlMaster).
// ---------------------------------------------------------------------------
pub(crate) mod win_ssh_mux {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::process::{Child, Command};
    use tokio::sync::Mutex as TokioMutex;

    struct MuxChild {
        stdin: tokio::process::ChildStdin,
        stdout: BufReader<tokio::process::ChildStdout>,
        child: Child,
    }

    // One multiplexed SSH session per user@host.
    // The TokioMutex serialises commands so marker boundaries never interleave.
    static MUX_SESSIONS: OnceLock<Mutex<HashMap<String, Arc<TokioMutex<Option<MuxChild>>>>>> =
        OnceLock::new();

    fn mux_map() -> &'static Mutex<HashMap<String, Arc<TokioMutex<Option<MuxChild>>>>> {
        MUX_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn session_lock(host_key: &str) -> Arc<TokioMutex<Option<MuxChild>>> {
        let mut map = mux_map().lock().unwrap();
        map.entry(host_key.to_string())
            .or_insert_with(|| Arc::new(TokioMutex::new(None)))
            .clone()
    }

    /// Spawn the persistent SSH process if it isn't already running.
    pub async fn ensure(ssh_user: &str, ssh_host: &str) -> Result<(), String> {
        let host_key = format!("{}@{}", ssh_user, ssh_host);
        let lock = session_lock(&host_key);
        let mut guard = lock.lock().await;

        // Already running and alive?
        if let Some(ref mut m) = *guard {
            if m.child.try_wait().ok().flatten().is_none() {
                return Ok(());
            }
            // Process exited — fall through and respawn.
        }

        let mut cmd = Command::new("ssh");
        cmd.args([
            "-T",
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ServerAliveInterval=15",
            "-o",
            "ServerAliveCountMax=3",
            &host_key,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
        super::hide_window_tokio_cmd(&mut cmd);
        let mut child = cmd.spawn().map_err(|e| format!("ssh mux spawn: {}", e))?;

        let stdin = child.stdin.take().ok_or("ssh mux: no stdin")?;
        let stdout = child.stdout.take().ok_or("ssh mux: no stdout")?;
        let reader = BufReader::new(stdout);

        *guard = Some(MuxChild {
            stdin,
            stdout: reader,
            child,
        });

        // Validate the connection with a quick echo test.
        drop(guard);
        match exec_inner(ssh_user, ssh_host, "echo __oc_mux_ready__").await {
            Ok(out) if out.contains("__oc_mux_ready__") => Ok(()),
            Ok(out) => {
                kill(ssh_user, ssh_host).await;
                Err(format!("ssh mux validation unexpected output: {}", out))
            }
            Err(e) => {
                kill(ssh_user, ssh_host).await;
                Err(format!("ssh mux validation failed: {}", e))
            }
        }
    }

    /// Send `cmd` through the persistent session and collect its stdout + exit code.
    pub async fn exec(ssh_user: &str, ssh_host: &str, cmd: &str) -> Result<String, String> {
        exec_inner(ssh_user, ssh_host, cmd).await
    }

    async fn exec_inner(ssh_user: &str, ssh_host: &str, cmd: &str) -> Result<String, String> {
        let host_key = format!("{}@{}", ssh_user, ssh_host);
        let lock = session_lock(&host_key);
        let mut guard = lock.lock().await;
        let mux = guard
            .as_mut()
            .ok_or_else(|| "ssh mux: not connected".to_string())?;

        // Check the process is still alive.
        if mux.child.try_wait().ok().flatten().is_some() {
            *guard = None;
            return Err("ssh mux: process exited".to_string());
        }

        // Unique marker that cannot appear in normal command output.
        let marker = format!(
            "__OCCLAW_{}__",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );

        // Wrap the command so we can capture its exit code after a unique delimiter.
        // The shell on the remote side will:
        //   1. Run cmd, capturing exit code in __ec
        //   2. Print a blank line + the marker + exit_code on one line
        let wrapped = format!(
            "{cmd}\n__ec=$?\necho \"\"\necho \"{marker} $__ec\"\n",
            cmd = cmd,
            marker = marker,
        );

        mux.stdin
            .write_all(wrapped.as_bytes())
            .await
            .map_err(|e| format!("ssh mux write: {}", e))?;
        mux.stdin
            .flush()
            .await
            .map_err(|e| format!("ssh mux flush: {}", e))?;

        // Read lines until we see the marker.
        let mut output = String::new();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
        let exit_code: i32;

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                // Subprocess may be hung — kill and return error.
                *guard = None;
                return Err("ssh mux: command timed out after 30s".to_string());
            }

            let mut line = String::new();
            let read_result =
                tokio::time::timeout(remaining, mux.stdout.read_line(&mut line)).await;

            match read_result {
                Ok(Ok(0)) => {
                    // EOF — the SSH process died.
                    *guard = None;
                    return Err("ssh mux: connection lost (EOF)".to_string());
                }
                Ok(Ok(_)) => {
                    if let Some(rest) = line.trim().strip_prefix(&marker) {
                        exit_code = rest.trim().parse().unwrap_or(-1);
                        break;
                    }
                    output.push_str(&line);
                }
                Ok(Err(e)) => {
                    *guard = None;
                    return Err(format!("ssh mux read: {}", e));
                }
                Err(_) => {
                    *guard = None;
                    return Err("ssh mux: command timed out after 30s".to_string());
                }
            }
        }

        if exit_code == 0 {
            Ok(output)
        } else if exit_code == 255 {
            // Transport-level failure — mark session dead so it respawns.
            *guard = None;
            Err(format!("ssh mux transport error (exit 255)"))
        } else {
            Err(format!(
                "ssh cmd failed [exit {}]\nstdout: {}",
                exit_code,
                output.trim()
            ))
        }
    }

    /// Kill the persistent subprocess for a given host.
    pub async fn kill(ssh_user: &str, ssh_host: &str) {
        let host_key = format!("{}@{}", ssh_user, ssh_host);
        let lock = session_lock(&host_key);
        let mut guard = lock.lock().await;
        if let Some(ref mut m) = *guard {
            let _ = m.child.kill().await;
        }
        *guard = None;
    }

    /// Kill all persistent subprocesses.
    pub async fn kill_all() {
        let keys: Vec<String> = { mux_map().lock().unwrap().keys().cloned().collect() };
        for key in keys {
            let lock = session_lock(&key);
            let mut guard = lock.lock().await;
            if let Some(ref mut m) = *guard {
                let _ = m.child.kill().await;
            }
            *guard = None;
        }
    }
}
/// Read the last `n` lines of a file using pure Rust (Windows replacement for `tail -n`
/// which is not available on Windows).
pub(crate) fn tail_lines_from_file(path: &std::path::Path, n: usize) -> Vec<String> {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else {
        return vec![];
    };
    let Ok(meta) = file.metadata() else {
        return vec![];
    };
    let len = meta.len();
    // Read up to 8KB from the end — more than enough for a handful of JSONL lines
    let read_size = std::cmp::min(len, 8192) as usize;
    let _ = file.seek(SeekFrom::End(-(read_size as i64)));
    let mut buf = vec![0u8; read_size];
    let Ok(bytes_read) = file.read(&mut buf) else {
        return vec![];
    };
    let text = String::from_utf8_lossy(&buf[..bytes_read]);
    let all_lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    if all_lines.len() <= n {
        all_lines
    } else {
        all_lines[all_lines.len() - n..].to_vec()
    }
}
/// Compute a UI-scale multiplier for Windows based on the monitor's logical
/// resolution. Baseline is 1080 logical height (a typical 1080p or 4K@200%
/// display). On a 4K@150% display the logical height is 1440, giving
/// multiplier ≈ 1.33 so all window dimensions grow proportionally.
/// On macOS this is not needed — the system handles points uniformly.
pub(crate) fn win_ui_scale(monitor: &tauri::Monitor) -> f64 {
    let scale = monitor.scale_factor();
    let logical_h = monitor.size().height as f64 / scale;
    (logical_h / 1080.0).max(1.0)
}
/// Returns the HMONITOR of the fullscreen foreground window, or None if the
/// foreground window is not fullscreen.  Excludes desktop shell windows
/// (Progman, WorkerW, Shell_TrayWnd) which cover the full screen but are
/// not real fullscreen apps.
pub(crate) fn fullscreen_foreground_monitor() -> Option<windows::Win32::Graphics::Gdi::HMONITOR> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowRect,
    };
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0 == std::ptr::null_mut() {
            return None;
        }

        let mut class_buf = [0u16; 64];
        let len = GetClassNameW(fg, &mut class_buf) as usize;
        if len > 0 {
            let class_name = String::from_utf16_lossy(&class_buf[..len]);
            if class_name == "Progman" || class_name == "WorkerW" || class_name == "Shell_TrayWnd" {
                return None;
            }
        }

        let mut fg_rect = RECT::default();
        if GetWindowRect(fg, &mut fg_rect).is_err() {
            return None;
        }
        let monitor = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut mi).as_bool() {
            return None;
        }
        let mr = mi.rcMonitor;
        if fg_rect.left <= mr.left
            && fg_rect.top <= mr.top
            && fg_rect.right >= mr.right
            && fg_rect.bottom >= mr.bottom
        {
            Some(monitor)
        } else {
            None
        }
    }
}
pub(crate) fn is_music_app_win(id: &str) -> bool {
    const MUSIC_APPS: &[&str] = &[
        "spotify",
        "zune",
        "zunemusic",
        "cloudmusic",
        "163music",
        "netease",
        "\u{7f51}\u{6613}\u{4e91}",
        "qqmusic",
        "qq\u{97f3}\u{4e50}",
        "kugou",
        "\u{9177}\u{72d7}",
        "kuwo",
        "\u{9177}\u{6211}",
        "foobar2000",
        "aimp",
        "musicbee",
        "itunes",
        "applemusic",
        "cider",
        "\u{6c7d}\u{6c34}\u{97f3}\u{4e50}",
        "soda",
    ];
    MUSIC_APPS.iter().any(|m| id.contains(m))
}
pub(crate) fn is_video_app_win(id: &str) -> bool {
    const VIDEO_APPS: &[&str] = &[
        "potplayer",
        "vlc",
        "mpv",
        "plex",
        "mpc-hc",
        "mpc-be",
        "kmplayer",
        "iina",
        "films",
        "bilibili",
        "\u{54d4}\u{54e9}\u{54d4}\u{54e9}",
        "disney",
        "netflix",
        "hbo",
        "douyin",
        "\u{6296}\u{97f3}",
        "tiktok",
        "iqiyi",
        "\u{7231}\u{5947}\u{827a}",
        "youku",
        "\u{4f18}\u{9177}",
        "mgtv",
        "\u{8292}\u{679c}",
        "dandanplay",
    ];
    VIDEO_APPS.iter().any(|v| id.contains(v))
}
pub(crate) fn is_browser_win(id: &str) -> bool {
    const BROWSERS: &[&str] = &[
        "chrome", "firefox", "msedge", "brave", "vivaldi", "opera", "arc",
    ];
    BROWSERS.iter().any(|b| id.contains(b))
}
/// Windows equivalent of `pet_passthrough_poll`. Polls the global cursor
/// position (via Win32 `GetCursorPos`) every 20 ms and toggles the mini
/// webview's `set_ignore_cursor_events` so clicks outside the mascot
/// hit-box pass through to whatever is behind, while clicks on the mascot
/// itself reach the webview. When the pet context menu is open the entire
/// window is interactive so menu buttons receive clicks.
pub(crate) fn pet_passthrough_poll_windows(
    app: tauri::AppHandle,
    mascot_scale: f64,
    large_mascot_scale: f64,
) {
    use std::time::Duration;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    PET_PASSTHROUGH_THREAD_ALIVE.store(true, Ordering::SeqCst);
    // mascot dimensions in logical pixels (matches CSS px on Windows WebView2).
    let (mascot_w_logical, mascot_h_logical) =
        large_collapsed_mascot_window_size(mascot_scale, large_mascot_scale);
    let hit_w = mascot_w_logical * (1.8 / 3.0);
    let hit_h = mascot_h_logical * (2.5 / 3.0);
    let inset_x_logical = (mascot_w_logical - hit_w) / 2.0;
    let inset_y_logical = (mascot_h_logical - hit_h) / 2.0;
    let edge_threshold_logical = 30.0_f64;

    let mut last_state: Option<bool> = None;

    while PET_PASSTHROUGH_ACTIVE.load(Ordering::SeqCst) {
        let menu_open = PET_CONTEXT_MENU_OPEN.load(Ordering::SeqCst);
        let pomodoro_active = PET_POMODORO_ACTIVE.load(Ordering::SeqCst);

        let should_be_interactive = if menu_open || pomodoro_active {
            true
        } else {
            // Read cursor position and window geometry in physical pixels.
            let cursor = unsafe {
                let mut pt = POINT::default();
                if GetCursorPos(&mut pt).is_ok() {
                    Some((pt.x as f64, pt.y as f64))
                } else {
                    None
                }
            };
            let win = app.get_webview_window("main");
            match (win, cursor) {
                (Some(win), Some((cx, cy))) => {
                    let pos = win.outer_position().ok();
                    let size = win.outer_size().ok();
                    let scale = win.scale_factor().unwrap_or(1.0);
                    let monitor = win.current_monitor().ok().flatten();
                    if let (Some(pos), Some(size)) = (pos, size) {
                        let fx = pos.x as f64;
                        let fy = pos.y as f64;
                        let fw = size.width as f64;
                        let fh = size.height as f64;

                        // Mascot is anchored at `left: petBaseWinW - mascotW` and `bottom: 0`,
                        // i.e. the right-bottom corner of the no-menu window. When the menu
                        // is closed, fw == petBaseWinW so the mascot's right edge in screen
                        // physical px is fx + fw and its bottom is fy + fh.
                        let mascot_w = mascot_w_logical * scale;
                        let mascot_h = mascot_h_logical * scale;
                        let inset_x = inset_x_logical * scale;
                        let inset_y = inset_y_logical * scale;
                        let edge_threshold = edge_threshold_logical * scale;

                        let mascot_right = fx + fw;
                        let mascot_left = mascot_right - mascot_w;
                        let mascot_bottom = fy + fh;
                        let mascot_top = mascot_bottom - mascot_h;

                        let near_edge = if let Some(monitor) = monitor {
                            let mp = monitor.position();
                            let ms = monitor.size();
                            let monitor_left = mp.x as f64;
                            let monitor_right = monitor_left + ms.width as f64;
                            mascot_left < monitor_left + edge_threshold
                                || mascot_right > monitor_right - edge_threshold
                        } else {
                            false
                        };

                        // Keep edge hitbox slightly relaxed on X only; do not use
                        // full-rect hitboxes, which feel too large during peek.
                        let ix = if near_edge { inset_x * 0.5 } else { inset_x };
                        let iy = inset_y;
                        let hit_left = mascot_left + ix;
                        let hit_right = mascot_right - ix;
                        let hit_top = mascot_top + iy;
                        let hit_bottom = mascot_bottom - iy;

                        cx >= hit_left && cx <= hit_right && cy >= hit_top && cy <= hit_bottom
                    } else {
                        false
                    }
                }
                _ => false,
            }
        };

        if last_state != Some(should_be_interactive) {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_ignore_cursor_events(!should_be_interactive);
            }
            last_state = Some(should_be_interactive);
        }

        std::thread::sleep(Duration::from_millis(20));
    }

    // Re-enable click events on exit so the window stays usable when leaving pet mode.
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_ignore_cursor_events(false);
    }
    PET_PASSTHROUGH_THREAD_ALIVE.store(false, Ordering::SeqCst);
}
