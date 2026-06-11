//! Windows-specific helpers. Gated by the outer `#[cfg(target_os = "windows")]` in `platform/mod.rs`.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{Emitter, Manager};

use crate::mascot::large_collapsed_mascot_window_size;
#[allow(unused_imports)]
use crate::platform::common::*;
use crate::state::{PetState, WindowState};

/// Apply CREATE_NO_WINDOW on Windows to prevent console popups from child processes.
pub(crate) fn hide_window_cmd(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}
/// Apply CREATE_NO_WINDOW on Windows to prevent console popups (tokio version).
pub(crate) fn hide_window_tokio_cmd(cmd: &mut tokio::process::Command) {
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
    type MuxMap = Mutex<HashMap<String, Arc<TokioMutex<Option<MuxChild>>>>>;
    static MUX_SESSIONS: OnceLock<MuxMap> = OnceLock::new();

    fn mux_map() -> &'static MuxMap {
        MUX_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn session_lock(host_key: &str) -> Arc<TokioMutex<Option<MuxChild>>> {
        let mut map = crate::state::lock_or_recover(mux_map());
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
            Err("ssh mux transport error (exit 255)".to_string())
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
        let keys: Vec<String> = {
            crate::state::lock_or_recover(mux_map())
                .keys()
                .cloned()
                .collect()
        };
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
        if fg.0.is_null() {
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
/// Unified poll thread for both Pet and Coding modes on Windows (the Windows
/// counterpart of macOS's `pet_passthrough_poll` + `efficiency_hover_poll`
/// pair). Polls the global cursor and left-button state every 20 ms.
///
/// **Pet mode** (`ps.passthrough_active`): click-through hit-testing against
/// the mascot rect (bottom-right corner of the window), anchor-based window
/// drag, walk-direction events, and drag-throw velocity for the physics loop.
///
/// **Coding mode** (`ws.hover_active`): hover detection against the mascot
/// body (upper-centre fraction of the collapsed window, like macOS
/// `efficiency_hover_poll`). The window stays fully interactive — it is
/// barely larger than the mascot, so click-through buys nothing there.
///
/// `mini-mascot-hover` / `mini-mascot-drag-*` events match the macOS emitters
/// in `pet_core.rs`; the frontend consumes hover exclusively from these
/// events (`useExternalHover`). Throw velocities are per-tick cursor deltas
/// in logical px (top-down y), the same units the macOS poll feeds
/// `loop.beginThrow`.
///
/// The thread stays alive while EITHER mode flag is set and re-reads the
/// mascot scales from `PetState` every tick, so it survives mode switches
/// without going stale.
pub(crate) fn pet_passthrough_poll_windows(
    app: tauri::AppHandle,
    ws: Arc<WindowState>,
    ps: Arc<PetState>,
) {
    use std::collections::VecDeque;
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let edge_threshold_logical = 30.0_f64;
    let mut last_state: Option<bool> = None;

    // Drag state machine (mirrors macOS pet_core.rs). Anchor and cursor are
    // physical px so cross-monitor DPI changes cannot skew the window drag.
    let mut drag_active = false;
    let mut drag_anchor: Option<(f64, f64)> = None;
    let mut last_cursor: (f64, f64) = (0.0, 0.0);
    let mut last_walk_dir: i32 = 0;
    let mut was_pressed = false;
    let mut was_over_mascot = false;

    // Drag-throw velocity sampling: (timestamp, dx, dy) per-tick deltas in
    // logical px. Same buffer parameters as macOS.
    let mut throw_samples: VecDeque<(Instant, f64, f64)> = VecDeque::with_capacity(32);
    const THROW_SAMPLE_CAP: usize = 24;
    const THROW_AVG_WINDOW_MS: u128 = 250;
    const MAX_THROW_SPEED: f64 = 30.0;

    loop {
        while ps.passthrough_active.load(Ordering::SeqCst) || ws.hover_active.load(Ordering::SeqCst)
        {
            let pet_mode = ps.passthrough_active.load(Ordering::SeqCst);
            let menu_open = ps.context_menu_open.load(Ordering::SeqCst);
            let pomodoro_active = ps.pomodoro_active.load(Ordering::SeqCst);

            let cursor = unsafe {
                let mut pt = POINT::default();
                if GetCursorPos(&mut pt).is_ok() {
                    Some((pt.x as f64, pt.y as f64))
                } else {
                    None
                }
            };
            let left_pressed =
                unsafe { (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0 };

            // Window geometry in physical px.
            let win = app.get_webview_window("main");
            let geom = win.as_ref().and_then(|w| {
                let pos = w.outer_position().ok()?;
                let size = w.outer_size().ok()?;
                let scale = w.scale_factor().unwrap_or(1.0);
                Some((
                    pos.x as f64,
                    pos.y as f64,
                    size.width as f64,
                    size.height as f64,
                    scale,
                ))
            });

            // Scales are re-read every tick: the thread may have been started
            // by coding-mode hover tracking with defaults, then upgraded by
            // set_pet_mode_window.
            let mascot_scale = f64::from_bits(ps.mascot_scale_bits.load(Ordering::SeqCst));
            let large_mascot_scale =
                f64::from_bits(ps.large_mascot_scale_bits.load(Ordering::SeqCst));
            let (mascot_w_logical, mascot_h_logical) =
                large_collapsed_mascot_window_size(mascot_scale, large_mascot_scale);
            let hit_w = mascot_w_logical * (1.8 / 3.0);
            let hit_h = mascot_h_logical * (2.5 / 3.0);
            let inset_x_logical = (mascot_w_logical - hit_w) / 2.0;
            let inset_y_logical = (mascot_h_logical - hit_h) / 2.0;

            // Real mascot hit-test. Never forced by menu/pomodoro state —
            // those only force interactivity below, otherwise a click on a
            // context-menu button would start a window drag.
            let over_mascot = match (geom, cursor) {
                (Some((fx, fy, fw, fh, scale)), Some((cx, cy))) => {
                    if pet_mode {
                        // Mascot is anchored at the bottom-right corner of the
                        // (possibly menu-expanded) window.
                        let mascot_w = mascot_w_logical * scale;
                        let mascot_h = mascot_h_logical * scale;
                        let mascot_right = fx + fw;
                        let mascot_left = mascot_right - mascot_w;
                        let mascot_bottom = fy + fh;
                        let mascot_top = mascot_bottom - mascot_h;

                        // Relax the X insets when the mascot extends near/past
                        // a monitor edge so the visible sliver stays clickable
                        // during peek; never full-rect (feels too grabby).
                        let near_edge = win
                            .as_ref()
                            .and_then(|w| w.current_monitor().ok().flatten())
                            .map(|monitor| {
                                let monitor_left = monitor.position().x as f64;
                                let monitor_right = monitor_left + monitor.size().width as f64;
                                let edge_threshold = edge_threshold_logical * scale;
                                mascot_left < monitor_left + edge_threshold
                                    || mascot_right > monitor_right - edge_threshold
                            })
                            .unwrap_or(false);
                        let ix = inset_x_logical * scale * if near_edge { 0.5 } else { 1.0 };
                        let iy = inset_y_logical * scale;
                        cx >= mascot_left + ix
                            && cx <= mascot_right - ix
                            && cy >= mascot_top + iy
                            && cy <= mascot_bottom - iy
                    } else {
                        // Coding mode: the collapsed window is the mascot's
                        // bounding box; hit only the visible body
                        // (upper-centre fractions, mirroring macOS
                        // `efficiency_hover_poll`), never the expanded panel.
                        !ws.expanded.load(Ordering::SeqCst)
                            && cx >= fx + fw * 0.32
                            && cx <= fx + fw * 0.68
                            && cy >= fy + fh * 0.10
                            && cy <= fy + fh * 0.75
                    }
                }
                _ => false,
            };

            // ── Drag state machine (pet mode only) ──
            if pet_mode {
                if drag_active {
                    if left_pressed {
                        if let (Some((cx, cy)), Some((ax, ay)), Some(w)) =
                            (cursor, drag_anchor, win.as_ref())
                        {
                            let _ = w.set_position(tauri::PhysicalPosition::new(
                                (cx - ax).round() as i32,
                                (cy - ay).round() as i32,
                            ));
                        }
                        if let Some((cx, cy)) = cursor {
                            let scale = geom.map(|g| g.4).unwrap_or(1.0);
                            let dx = (cx - last_cursor.0) / scale;
                            // Windows screen y already grows downward, which
                            // is the frontend physics convention (macOS has
                            // to flip; we don't).
                            let dy = (cy - last_cursor.1) / scale;
                            last_cursor = (cx, cy);
                            let walk_dir = if dx > 0.5 {
                                1
                            } else if dx < -0.5 {
                                -1
                            } else {
                                last_walk_dir
                            };
                            if walk_dir != last_walk_dir {
                                let _ = app.emit("mini-mascot-walk", walk_dir);
                                last_walk_dir = walk_dir;
                            }
                            if ps.throw_tracking.load(Ordering::SeqCst) {
                                throw_samples.push_back((Instant::now(), dx, dy));
                                while throw_samples.len() > THROW_SAMPLE_CAP {
                                    throw_samples.pop_front();
                                }
                            }
                        }
                    } else {
                        // Drag finished.
                        drag_active = false;
                        drag_anchor = None;
                        if last_walk_dir != 0 {
                            let _ = app.emit("mini-mascot-walk", 0i32);
                            last_walk_dir = 0;
                        }
                        // Average the recent non-stationary samples into a
                        // release velocity (same algorithm as pet_core.rs:
                        // users settle the cursor for a beat before letting
                        // go, so zero samples are skipped, not averaged in).
                        if ps.throw_tracking.load(Ordering::SeqCst) && !throw_samples.is_empty() {
                            let cutoff = Instant::now();
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
                                let vx = (sum_dx / count as f64)
                                    .clamp(-MAX_THROW_SPEED, MAX_THROW_SPEED);
                                let vy = (sum_dy / count as f64)
                                    .clamp(-MAX_THROW_SPEED, MAX_THROW_SPEED);
                                log::info!(
                                    "[drag-throw] samples={}/{} → vx={:.2} vy={:.2}",
                                    count,
                                    total_seen,
                                    vx,
                                    vy,
                                );
                                let _ = app.emit(
                                    "mini-mascot-drag-throw",
                                    serde_json::json!({ "vx": vx, "vy": vy }),
                                );
                            } else {
                                log::info!(
                                    "[drag-throw] all {} samples in {}ms window were near-zero",
                                    total_seen,
                                    THROW_AVG_WINDOW_MS,
                                );
                            }
                        }
                        throw_samples.clear();
                        let _ = app.emit("mini-mascot-drag-end", ());
                    }
                } else if over_mascot
                    && left_pressed
                    && !was_pressed
                    && !menu_open
                    && !pomodoro_active
                {
                    // Drag start. Suppressed while the context menu or the
                    // pomodoro UI is up — those clicks must never move the
                    // window.
                    if let (Some((cx, cy)), Some((fx, fy, ..))) = (cursor, geom) {
                        drag_active = true;
                        drag_anchor = Some((cx - fx, cy - fy));
                        last_cursor = (cx, cy);
                        throw_samples.clear();
                        // Cancel hover so the sprite leaves `jumping`
                        // immediately when the drag begins.
                        if was_over_mascot {
                            let _ = app.emit("mini-mascot-hover", false);
                            was_over_mascot = false;
                        }
                        let _ = app.emit("mini-mascot-drag-start", ());
                    }
                }
            } else if drag_active {
                // Mode switched away mid-drag: drop the drag cleanly.
                drag_active = false;
                drag_anchor = None;
                throw_samples.clear();
                if last_walk_dir != 0 {
                    let _ = app.emit("mini-mascot-walk", 0i32);
                    last_walk_dir = 0;
                }
            }
            was_pressed = left_pressed;

            // ── Hover signal (both modes) ──
            // Suppressed while dragging so the sprite shows the pinch/run
            // state instead of jumping. Edge-triggered, like macOS.
            let hover_signal = over_mascot && !drag_active;
            if hover_signal != was_over_mascot {
                let _ = app.emit("mini-mascot-hover", hover_signal);
                was_over_mascot = hover_signal;
            }

            // ── Click-through toggle ──
            // Coding mode keeps the window permanently interactive. Pet mode
            // is interactive over the mascot, during a drag, and whenever the
            // context menu / pomodoro UI needs its buttons clickable.
            let interactive = if pet_mode {
                menu_open || pomodoro_active || over_mascot || drag_active
            } else {
                true
            };
            if last_state != Some(interactive) {
                if let Some(w) = win.as_ref() {
                    let _ = w.set_ignore_cursor_events(!interactive);
                }
                last_state = Some(interactive);
            }

            std::thread::sleep(Duration::from_millis(20));
        }

        ps.passthrough_thread_alive.store(false, Ordering::SeqCst);
        // A mode flag may have flipped back on between the loop condition and
        // the alive=false store above (the spawn sites skip when alive is
        // still true). Re-claim and keep polling instead of leaving the new
        // mode threadless; if a fresh thread already claimed the flag, exit.
        if (ps.passthrough_active.load(Ordering::SeqCst) || ws.hover_active.load(Ordering::SeqCst))
            && ps
                .passthrough_thread_alive
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        {
            continue;
        }
        break;
    }

    // Re-enable click events on exit so the window stays usable when leaving
    // pet/coding mode.
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_ignore_cursor_events(false);
    }
}
