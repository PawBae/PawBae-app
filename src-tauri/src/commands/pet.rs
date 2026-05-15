//! Tauri pet-physics, efficiency-hover, stroll-mode, demo-mascot, and context-menu commands.

use std::sync::atomic::Ordering;

use tauri::Manager;

use crate::platform::common::AppWindowInfo;
use crate::state::{
    EFFICIENCY_HOVER_ACTIVE, EFFICIENCY_HOVER_THREAD_ALIVE, MINI_WINDOW_FRAME,
    PET_CONTEXT_MENU_OPEN, PET_MENU_RESTORE_FRAME, PET_PASSTHROUGH_ACTIVE,
    PET_PASSTHROUGH_THREAD_ALIVE, PET_POMODORO_ACTIVE, SPRITE_PAD, STROLL_MODE_ENABLED,
    THROW_TRACKING_ENABLED,
};
use crate::{
    efficiency_hover_poll, large_collapsed_mascot_window_size, sanitized_mascot_scale,
    LARGE_MASCOT_SIZE_MULTIPLIER, MASCOT_TOP_INSET,
};

#[cfg(target_os = "macos")]
use crate::{
    compute_frontmost_app_window_macos, frontmost_app_window_cache,
    pet_context_schedule_restore_alpha, pet_passthrough_poll,
};

#[cfg(target_os = "windows")]
use crate::platform::windows::pet_passthrough_poll_windows;
#[cfg(target_os = "windows")]
use crate::state::FULLSCREEN_HIDING;

#[tauri::command]
pub async fn get_frontmost_app_window(
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
        })
        .map_err(|e| e.to_string())?;
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
pub async fn set_sprite_pad_fractions(
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
    if let Some(v) = top {
        if v.is_finite() && v >= 0.0 && v <= 0.95 {
            g.top = v;
        }
    }
    if let Some(v) = right {
        if v.is_finite() && v >= 0.0 && v <= 0.95 {
            g.right = v;
        }
    }
    if let Some(v) = bottom {
        if v.is_finite() && v >= 0.0 && v <= 0.95 {
            g.bottom = v;
        }
    }
    if let Some(v) = left {
        if v.is_finite() && v >= 0.0 && v <= 0.95 {
            g.left = v;
        }
    }
    // Absolute CSS pixels. Reject NaN / negative / insanely large
    // values so a buggy frontend can't push the cat off-screen.
    let validate_px = |v: f64| -> Option<f64> {
        if v.is_finite() && v >= 0.0 && v <= 1000.0 {
            Some(v)
        } else {
            None
        }
    };
    if let Some(v) = top_px {
        if let Some(px) = validate_px(v) {
            g.top_px = Some(px);
        }
    }
    if let Some(v) = right_px {
        if let Some(px) = validate_px(v) {
            g.right_px = Some(px);
        }
    }
    if let Some(v) = bottom_px {
        if let Some(px) = validate_px(v) {
            g.bottom_px = Some(px);
        }
    }
    if let Some(v) = left_px {
        if let Some(px) = validate_px(v) {
            g.left_px = Some(px);
        }
    }
    Ok(())
}
/// Pet-physics floor info, packed for one IPC roundtrip per cache TTL.
/// Y values are in macOS bottom-up logical pixels.
#[derive(serde::Serialize)]
pub(crate) struct PetFloorInfo {
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
pub async fn get_pet_floor_info(app: tauri::AppHandle) -> Result<PetFloorInfo, String> {
    let win = app
        .get_webview_window("main")
        .ok_or("mini window not found")?;
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let win_clone = win.clone();
        app.run_on_main_thread(move || {
            use objc2::msg_send;
            use objc2::runtime::{AnyClass, AnyObject};
            use objc2_foundation::NSRect;
            if let Ok(ns_win) = win_clone.ns_window() {
                let obj = unsafe { &*(ns_win as *mut AnyObject) };
                let screen: *mut AnyObject = unsafe { msg_send![obj, screen] };
                let (frame, visible): (Option<NSRect>, Option<NSRect>) = unsafe {
                    if screen.is_null() {
                        match AnyClass::get(c"NSScreen") {
                            Some(cls) => {
                                let main: *mut AnyObject = msg_send![cls, mainScreen];
                                if main.is_null() {
                                    (None, None)
                                } else {
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
        })
        .map_err(|e| e.to_string())?;
        if let Ok((frame, visible, dock)) = rx.recv_timeout(std::time::Duration::from_secs(1)) {
            let off_dock_y = frame.map(|f| f.origin.y).unwrap_or(0.0);
            let on_dock_y = visible.map(|v| v.origin.y).unwrap_or(off_dock_y);
            let dock_x_range = dock.map(|(x, _, w, _)| (x, x + w));
            return Ok(PetFloorInfo {
                on_dock_y,
                off_dock_y,
                dock_x_range,
            });
        }
    }
    #[allow(unreachable_code)]
    Ok(PetFloorInfo {
        on_dock_y: 0.0,
        off_dock_y: 0.0,
        dock_x_range: None,
    })
}
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
pub async fn set_efficiency_hover_tracking(
    app: tauri::AppHandle,
    active: bool,
) -> Result<(), String> {
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
pub fn set_stroll_mode(_app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
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
pub fn set_throw_tracking(enabled: bool) -> Result<(), String> {
    log::info!("[stroll] set_throw_tracking({})", enabled);
    THROW_TRACKING_ENABLED.store(enabled, Ordering::SeqCst);
    Ok(())
}
#[tauri::command]
pub async fn set_pet_mode_window(
    app: tauri::AppHandle,
    active: bool,
    mascot_scale: Option<f64>,
    large_mascot_scale: Option<f64>,
) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or("mini window not found")?;
    let mascot_scale = sanitized_mascot_scale(mascot_scale);
    let large_mascot_scale = large_mascot_scale.unwrap_or(LARGE_MASCOT_SIZE_MULTIPLIER);

    if active {
        // Expand window to menu-ready size (mascot area + padding for buttons).
        #[cfg(target_os = "macos")]
        {
            let win_clone = win.clone();
            app.run_on_main_thread(move || {
                use objc2::msg_send;
                use objc2::runtime::{AnyClass, AnyObject};
                use objc2_foundation::{NSPoint, NSRect, NSSize};
                if let Ok(ns_win) = win_clone.ns_window() {
                    let obj = unsafe { &*(ns_win as *mut AnyObject) };
                    let current: NSRect = unsafe { msg_send![obj, frame] };
                    let screen_info: Option<(f64, f64, f64, f64)> = unsafe {
                        let screen: *mut AnyObject = msg_send![obj, screen];
                        if screen.is_null() {
                            let cls = AnyClass::get(c"NSScreen");
                            cls.and_then(|c| {
                                let ms: *mut AnyObject = msg_send![c, mainScreen];
                                if ms.is_null() {
                                    None
                                } else {
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
                            let _: () =
                                msg_send![obj, setFrame: frame, display: true, animate: false];
                            let _: () = msg_send![obj, setLevel: 27isize];
                            let _: () = msg_send![obj, orderFrontRegardless];
                        }
                        if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                            *f = Some((x, y, win_w, win_h));
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
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
            std::thread::spawn(move || {
                pet_passthrough_poll(app2, mascot_scale, large_mascot_scale)
            });
        }
        #[cfg(target_os = "windows")]
        if !PET_PASSTHROUGH_THREAD_ALIVE.load(Ordering::SeqCst) {
            let app2 = app.clone();
            std::thread::spawn(move || {
                pet_passthrough_poll_windows(app2, mascot_scale, large_mascot_scale)
            });
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
                use objc2::msg_send;
                use objc2::runtime::AnyObject;
                use objc2_foundation::{NSPoint, NSRect, NSSize};
                if let Ok(ns_win) = win_clone.ns_window() {
                    let obj = unsafe { &*(ns_win as *mut AnyObject) };
                    let current: NSRect = unsafe { msg_send![obj, frame] };
                    let (win_w, win_h) =
                        large_collapsed_mascot_window_size(mascot_scale, large_mascot_scale);
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
            })
            .map_err(|e| e.to_string())?;
        }
        #[cfg(target_os = "windows")]
        {
            if let Ok(Some(monitor)) = win.current_monitor() {
                let scale = monitor.scale_factor();
                if let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) {
                    let current_x = pos.x as f64 / scale;
                    let current_y = pos.y as f64 / scale;
                    let current_w = size.width as f64 / scale;
                    let (win_w, win_h) =
                        large_collapsed_mascot_window_size(mascot_scale, large_mascot_scale);
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
pub async fn set_pet_pomodoro_active(active: bool) -> Result<(), String> {
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
pub async fn set_pet_context_menu(
    app: tauri::AppHandle,
    open: bool,
    side: Option<String>,
) -> Result<(), String> {
    PET_CONTEXT_MENU_OPEN.store(open, Ordering::SeqCst);

    #[cfg(target_os = "macos")]
    {
        let right_pad = 180.0_f64;
        if open && side.as_deref() == Some("right") {
            if let Some(win) = app.get_webview_window("main") {
                let win_clone = win.clone();
                let (tx, rx) = std::sync::mpsc::channel::<()>();
                let _ = app.run_on_main_thread(move || {
                    use objc2::msg_send;
                    use objc2::runtime::AnyObject;
                    use objc2_foundation::{NSPoint, NSRect, NSSize};
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
                            let _: () =
                                msg_send![obj, setFrame: frame, display: true, animate: false];
                        }
                        if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                            *f = Some((
                                current.origin.x,
                                current.origin.y,
                                new_w,
                                current.size.height,
                            ));
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
                        let (current_x, current_y) =
                            match (win.outer_position(), win.current_monitor()) {
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
/// Spawn a demo-mode mini mascot window. Each window runs the bundled
/// frontend with `?demo=1&pet=<id>` query params, which routes to a
/// minimal mascot-only React tree. Used by the dev-mode "演示模式" toggle
/// to drop multiple animated mascots on screen for demo recordings.
#[tauri::command]
pub async fn spawn_demo_mascot(app: tauri::AppHandle, pet_id: String) -> Result<String, String> {
    use std::sync::atomic::AtomicU64;
    static DEMO_COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = DEMO_COUNTER.fetch_add(1, Ordering::SeqCst);
    let label = format!("demo-mascot-{}", n);

    let url = format!("index.html#/mini?demo=1&pet={}", pet_id);
    let win =
        tauri::WebviewWindowBuilder::new(&app, label.clone(), tauri::WebviewUrl::App(url.into()))
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
                    let _: () =
                        msg_send![demo_obj, setFrame: new_frame, display: true, animate: false];
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
pub async fn close_demo_mascot(app: tauri::AppHandle, label: String) -> Result<bool, String> {
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
pub async fn close_demo_mascots(app: tauri::AppHandle) -> Result<u32, String> {
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
