//! Tauri window-management commands: mini window open/close, position, sizing, IME, UI scale, focus.

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::mascot::{
    collapsed_mascot_window_size, large_collapsed_mascot_window_size, sanitized_mascot_scale,
    COLLAPSED_MASCOT_BASE_H, COLLAPSED_MASCOT_BASE_W, LARGE_MASCOT_SIZE_MULTIPLIER,
    MASCOT_TOP_INSET,
};
use crate::pet_core::reassert_mini_floating;
use crate::state::MINI_WINDOW_FRAME;

#[cfg(target_os = "macos")]
use crate::mascot::{collapsed_x, current_sprite_pad};
#[cfg(target_os = "macos")]
use crate::platform::macos::{get_notch_offset, pet_context_schedule_restore_alpha};
#[cfg(target_os = "macos")]
use crate::state::{EFFICIENCY_EXPANDED, NOTCH_SCREEN_INFO, PET_MENU_RESTORE_FRAME};
#[cfg(target_os = "macos")]
use std::sync::atomic::Ordering;

#[cfg(target_os = "windows")]
use crate::platform::windows::win_ui_scale;
#[cfg(target_os = "windows")]
use crate::state::FULLSCREEN_HIDING;

#[tauri::command]
pub async fn open_mini(app: tauri::AppHandle) -> Result<(), String> {
    log::info!("[mini-pos] open_mini called");
    if let Some(win) = app.get_webview_window("main") {
        // Reposition to collapsed position before showing
        #[cfg(target_os = "macos")]
        {
            let win_clone = win.clone();
            let _ = app.run_on_main_thread(move || {
                use objc2::runtime::{AnyClass, AnyObject};
                use objc2::msg_send;
                use objc2_foundation::{NSRect, NSPoint, NSSize};

                if let Ok(ns_win) = win_clone.ns_window() {
                    let obj = unsafe { &*(ns_win as *mut AnyObject) };
                    unsafe {
                        let _: () = msg_send![obj, setLevel: 27isize];
                        let behavior: usize = (1 << 0) | (1 << 4) | (1 << 8) | (1 << 6);
                        let _: () = msg_send![obj, setCollectionBehavior: behavior];
                    }
                    let screen_info: Option<(f64, f64, f64, f64, f64)> = unsafe {
                        let cls = match AnyClass::get(c"NSScreen") {
                            Some(c) => c,
                            None => return,
                        };
                        let screens: *mut AnyObject = msg_send![cls, screens];
                        if screens.is_null() { return; }
                        let count: usize = msg_send![&*screens, count];
                        if count == 0 { return; }
                        let screen: *mut AnyObject = msg_send![&*screens, objectAtIndex: 0usize];
                        if screen.is_null() { return; }
                        let frame: NSRect = msg_send![&*screen, frame];
                        let notch_off = get_notch_offset(screen);
                        Some((frame.origin.x, frame.origin.y, frame.size.width, frame.size.height, notch_off))
                    };
                    if let Some((sx, sy, sw, sh, notch_off)) = screen_info {
                        let (win_w, win_h) = MINI_WINDOW_FRAME
                            .lock()
                            .ok()
                            .and_then(|g| *g)
                            .map(|(_, _, w, h)| (w, h))
                            .unwrap_or_else(|| collapsed_mascot_window_size(1.0));
                        let x = sx + sw / 2.0 + notch_off;
                        // Pull the window down by MASCOT_TOP_INSET so it
                        // does not sit under the menu bar / notch on launch.
                        let y = sy + sh - win_h - MASCOT_TOP_INSET;
                        log::info!(
                            "[mini-pos] open_mini(existing,mac) target frame x={:.1} y={:.1} w={:.1} h={:.1} inset={:.1} screen=({:.1},{:.1},{:.1},{:.1}) notch_off={:.1}",
                            x, y, win_w, win_h, MASCOT_TOP_INSET, sx, sy, sw, sh, notch_off
                        );
                        let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(win_w, win_h));
                        unsafe {
                            let _: () = msg_send![obj, setFrame: frame, display: true];
                            let _: () = msg_send![obj, orderFrontRegardless];
                        }
                    }
                }
            });
        }
        #[cfg(target_os = "windows")]
        {
            // Reposition to top-center (simulating macOS notch position), DPI-aware
            if let Ok(Some(monitor)) = win.primary_monitor() {
                let scale = monitor.scale_factor();
                let sw = monitor.size().width as f64 / scale;
                let ui = win_ui_scale(&monitor);
                let (base_w, base_h) = MINI_WINDOW_FRAME
                    .lock()
                    .ok()
                    .and_then(|g| *g)
                    .map(|(_, _, w, h)| (w, h))
                    .unwrap_or_else(|| collapsed_mascot_window_size(1.0));
                let win_w = (base_w * ui).round();
                let win_h = (base_h * ui).round();
                let notch_off = (80.0 * ui).round();
                let x = sw / 2.0 + notch_off;
                let _ = win.set_size(tauri::LogicalSize::new(win_w, win_h));
                log::info!(
                    "[mini-pos] open_mini(existing,win) target pos x={:.1} y={:.1} w={:.1} h={:.1} ui={:.2} notch_off={:.1}",
                    x, 0.0, win_w, win_h, ui, notch_off
                );
                let _ = win.set_position(tauri::LogicalPosition::new(x, 0.0));
            }
            if !FULLSCREEN_HIDING.load(std::sync::atomic::Ordering::SeqCst) {
                win.show().map_err(|e| e.to_string())?;
                win.set_focus().map_err(|e| e.to_string())?;
            }
        }
        return Ok(());
    }

    let builder = WebviewWindowBuilder::new(&app, "main", WebviewUrl::App("index.html".into()))
        .title("PawBae")
        .inner_size(COLLAPSED_MASCOT_BASE_W, COLLAPSED_MASCOT_BASE_H)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .accept_first_mouse(true); // single click from any app

    let win = builder.build().map_err(|e| e.to_string())?;

    // Use macOS native API to position at menu bar level (like notchi)
    // Must run on main thread for AppKit calls
    #[cfg(target_os = "macos")]
    {
        let win_clone = win.clone();
        let _ = app.run_on_main_thread(move || {
            use objc2::runtime::{AnyClass, AnyObject};
            use objc2::msg_send;
            use objc2_foundation::{NSRect, NSPoint, NSSize};

            if let Ok(ns_win) = win_clone.ns_window() {
                let obj = unsafe { &*(ns_win as *mut AnyObject) };

                unsafe {
                    let _: () = msg_send![obj, setLevel: 27isize];
                    let behavior: usize = (1 << 0) | (1 << 4) | (1 << 8) | (1 << 6);
                    let _: () = msg_send![obj, setCollectionBehavior: behavior];
                }

                let screen_info: Option<(f64, f64, f64, f64, f64)> = unsafe {
                    let cls = match AnyClass::get(c"NSScreen") {
                        Some(c) => c,
                        None => return,
                    };
                    let screens: *mut AnyObject = msg_send![cls, screens];
                    if screens.is_null() { return; }
                    let count: usize = msg_send![&*screens, count];
                    if count == 0 { return; }
                    let screen: *mut AnyObject = msg_send![&*screens, objectAtIndex: 0usize];
                    if screen.is_null() { return; }
                    let frame: NSRect = msg_send![&*screen, frame];
                    let notch_off = get_notch_offset(screen);
                    Some((frame.origin.x, frame.origin.y, frame.size.width, frame.size.height, notch_off))
                };

                if let Some((sx, sy, sw, sh, notch_off)) = screen_info {
                    let (win_w, win_h) = collapsed_mascot_window_size(1.0);
                    let x = sx + sw / 2.0 + notch_off;
                    // Pull the window down by MASCOT_TOP_INSET so the sprite
                    // is fully visible below the menu bar / notch on launch.
                    let y = sy + sh - win_h - MASCOT_TOP_INSET;
                    log::info!(
                        "[mini-pos] open_mini(new,mac) target frame x={:.1} y={:.1} w={:.1} h={:.1} inset={:.1} screen=({:.1},{:.1},{:.1},{:.1}) notch_off={:.1}",
                        x, y, win_w, win_h, MASCOT_TOP_INSET, sx, sy, sw, sh, notch_off
                    );
                    let frame = NSRect::new(
                        NSPoint::new(x, y),
                        NSSize::new(win_w, win_h),
                    );
                    unsafe {
                        let _: () = msg_send![obj, setFrame: frame, display: true];
                    }
                }

                unsafe {
                    let _: () = msg_send![obj, orderFrontRegardless];
                }
            }
        });
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows: position at top-center (simulating macOS notch position), DPI-aware
        if let Ok(Some(monitor)) = win.primary_monitor() {
            let scale = monitor.scale_factor();
            let sw = monitor.size().width as f64 / scale;
            let ui = win_ui_scale(&monitor);
            let (base_w, base_h) = collapsed_mascot_window_size(1.0);
            let win_w = (base_w * ui).round();
            let win_h = (base_h * ui).round();
            let notch_off = (80.0 * ui).round();
            let x = sw / 2.0 + notch_off;
            let _ = win.set_size(tauri::LogicalSize::new(win_w, win_h));
            log::info!(
                "[mini-pos] open_mini(new,win) target pos x={:.1} y={:.1} w={:.1} h={:.1} ui={:.2} notch_off={:.1}",
                x, 0.0, win_w, win_h, ui, notch_off
            );
            let _ = win.set_position(tauri::LogicalPosition::new(x, 0.0));
        }
        if !FULLSCREEN_HIDING.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = win.show();
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn close_mini(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        win.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
pub async fn get_ui_scale(app: tauri::AppHandle) -> Result<f64, String> {
    #[cfg(target_os = "windows")]
    {
        let win = app.get_webview_window("main").ok_or("mini not found")?;
        if let Ok(Some(m)) = win.current_monitor() {
            return Ok(win_ui_scale(&m));
        }
    }
    Ok(1.0)
}

#[tauri::command]
pub async fn move_mini_by(app: tauri::AppHandle, dx: f64, dy: f64) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or("mini window not found")?;
    #[cfg(target_os = "macos")]
    {
        let win_clone = win.clone();
        app.run_on_main_thread(move || {
            use objc2::msg_send;
            use objc2::runtime::{AnyClass, AnyObject};
            use objc2_foundation::{NSPoint, NSRect, NSSize};
            if let Ok(ns_win) = win_clone.ns_window() {
                let obj = unsafe { &*(ns_win as *mut AnyObject) };
                let frame: NSRect = unsafe { msg_send![obj, frame] };

                // Fetch both visibleFrame and full frame from the same
                // screen the window lives on (with mainScreen fallback).
                // visibleFrame is the on-Dock floor; frame.origin.y is
                // the off-Dock floor (actual screen bottom).
                let (visible_frame, screen_frame): (Option<NSRect>, Option<NSRect>) = unsafe {
                    let screen: *mut AnyObject = msg_send![obj, screen];
                    if screen.is_null() {
                        match AnyClass::get(c"NSScreen") {
                            Some(cls) => {
                                let main_screen: *mut AnyObject = msg_send![cls, mainScreen];
                                if main_screen.is_null() {
                                    (None, None)
                                } else {
                                    (
                                        Some(msg_send![&*main_screen, visibleFrame]),
                                        Some(msg_send![&*main_screen, frame]),
                                    )
                                }
                            }
                            None => (None, None),
                        }
                    } else {
                        (
                            Some(msg_send![&*screen, visibleFrame]),
                            Some(msg_send![&*screen, frame]),
                        )
                    }
                };
                // Intentionally NOT calling CGWindowList here: that's the
                // surface that risks a Screen Recording permission prompt
                // on recent macOS. We treat the whole visibleFrame width
                // as the platform — `over_dock` becomes unconditionally
                // true below.
                let dock_rect: Option<(f64, f64, f64, f64)> = None;

                // Bottom-up Cocoa coords: dy>0 means visually-down, which
                // is a *decrease* in origin.y.
                let target_x = frame.origin.x + dx;
                let target_y = frame.origin.y - dy;
                let (clamped_x, clamped_y) =
                    if let (Some(vf), Some(sf)) = (visible_frame, screen_frame) {
                        // Sprite-content padding. Each edge prefers the
                        // absolute CSS-pixel override pushed by the frontend
                        // after alpha-scanning the pet's atlas and
                        // DOM-measuring the rendered sprite layout; falls
                        // back to (fraction × window dimension) when the
                        // frontend hasn't pushed a value for that edge
                        // (e.g. a floor-only pet has no climb-ceiling row,
                        // so top_px stays None and the top fraction wins).
                        // The fraction-of-window-size formula is incorrect
                        // when the sprite div doesn't fill the window
                        // (centered, with empty pixels around) — only an
                        // absolute pixel value captures that offset.
                        let pad = current_sprite_pad();
                        let pad_bottom = pad.bottom_px.unwrap_or(frame.size.height * pad.bottom);
                        let pad_top = pad.top_px.unwrap_or(frame.size.height * pad.top);
                        let pad_left = pad.left_px.unwrap_or(frame.size.width * pad.left);
                        let pad_right = pad.right_px.unwrap_or(frame.size.width * pad.right);

                        // X: bounded by visibleFrame (so side Docks act as
                        // walls too). Walls and ceiling use visibleFrame; the
                        // *floor* is what becomes piecewise.
                        let min_x = vf.origin.x - pad_left;
                        let max_x = vf.origin.x + vf.size.width - frame.size.width + pad_right;
                        let cx = if max_x < min_x {
                            target_x
                        } else {
                            target_x.clamp(min_x, max_x)
                        };

                        // Piecewise floor: when the window's center-x is
                        // inside the Dock's horizontal extent, the floor is
                        // the top of the Dock (visibleFrame.y). When the
                        // pet walks off the side of the Dock the floor
                        // drops to the actual screen bottom (frame.y).
                        // Safety fallback: if Dock detection fails (returns
                        // None), treat the entire visibleFrame width as a
                        // platform so the pet still sits on the Dock area
                        // instead of plummeting past it.
                        let center_x = cx + frame.size.width / 2.0;
                        let over_dock = match dock_rect {
                            Some((dx0, _, dw, _)) => center_x >= dx0 && center_x <= dx0 + dw,
                            None => true,
                        };
                        let floor_y = if over_dock { vf.origin.y } else { sf.origin.y };
                        let min_y = floor_y - pad_bottom;
                        let max_y = vf.origin.y + vf.size.height - frame.size.height + pad_top;
                        let cy = if max_y < min_y {
                            target_y
                        } else {
                            target_y.clamp(min_y, max_y)
                        };
                        (cx, cy)
                    } else {
                        (target_x, target_y)
                    };

                let new_frame = NSRect::new(
                    NSPoint::new(clamped_x, clamped_y),
                    NSSize::new(frame.size.width, frame.size.height),
                );
                unsafe {
                    let _: () = msg_send![obj, setFrame: new_frame, display: true, animate: false];
                }
                if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                    *f = Some((
                        new_frame.origin.x,
                        new_frame.origin.y,
                        new_frame.size.width,
                        new_frame.size.height,
                    ));
                }
                // Keep the pet-context restore frame in sync when dragging
                // while the context menu is open, so closing restores to the
                // new position instead of the stale pre-drag position.
                // Use the *clamped* delta so a wall-hit doesn't desync the
                // restore frame from the real window position.
                let actual_dx = clamped_x - frame.origin.x;
                let actual_dy_top_down = frame.origin.y - clamped_y;
                if let Ok(mut saved) = PET_MENU_RESTORE_FRAME.lock() {
                    if let Some(ref mut s) = *saved {
                        s.0 += actual_dx;
                        s.1 -= actual_dy_top_down;
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        // outer_position() returns PhysicalPosition; dx/dy are in logical (CSS) pixels.
        // Convert physical → logical before adding the delta.
        if let Ok(pos) = win.outer_position() {
            let scale = win.scale_factor().unwrap_or(1.0);
            let logical_x = pos.x as f64 / scale;
            let logical_y = pos.y as f64 / scale;
            let _ = win.set_position(tauri::LogicalPosition::new(logical_x + dx, logical_y + dy));
        }
    }
    Ok(())
}

/// Get the mini window's origin in logical coordinates.
/// macOS: bottom-left origin (NSWindow frame).
/// Windows: top-left origin (screen coordinates).
#[tauri::command]
pub async fn get_mini_origin(app: tauri::AppHandle) -> Result<(f64, f64), String> {
    let win = app.get_webview_window("main").ok_or("mini not found")?;
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let win_clone = win.clone();
        app.run_on_main_thread(move || {
            use objc2::msg_send;
            use objc2::runtime::AnyObject;
            use objc2_foundation::NSRect;
            if let Ok(ns_win) = win_clone.ns_window() {
                let obj = unsafe { &*(ns_win as *mut AnyObject) };
                let frame: NSRect = unsafe { msg_send![obj, frame] };
                let _ = tx.send((frame.origin.x, frame.origin.y));
            }
        })
        .map_err(|e| e.to_string())?;
        if let Ok(pos) = rx.recv_timeout(std::time::Duration::from_secs(1)) {
            return Ok(pos);
        }
    }
    #[cfg(target_os = "windows")]
    {
        // outer_position() returns PhysicalPosition; convert to logical for consistency.
        if let Ok(pos) = win.outer_position() {
            let scale = win.scale_factor().unwrap_or(1.0);
            return Ok((pos.x as f64 / scale, pos.y as f64 / scale));
        }
    }
    Err("failed to get origin".into())
}

/// Return the monitor rect (x, y, w, h) in logical pixels for the monitor
/// the mini window currently lives on. Used by the front-end pet physics
/// and walk/peek/menu logic to detect screen edges on multi-monitor setups.
///
/// On macOS this returns `NSScreen.visibleFrame` — the rect excluding the
/// menu bar, the notch's reserved strip, and the Dock (regardless of
/// Dock position or auto-hide). That way the pet's floor sits on top of
/// the Dock, its ceiling hugs the menu bar, and side Docks act as walls.
/// On Windows the full monitor rect is still returned (taskbar handling
/// is a planned follow-up).
#[tauri::command]
pub async fn get_mini_monitor_rect(app: tauri::AppHandle) -> Result<(f64, f64, f64, f64), String> {
    let win = app.get_webview_window("main").ok_or("mini not found")?;
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
                // visibleFrame excludes menu bar + Dock so physics edge
                // detection treats the top of the Dock as the floor and
                // the bottom of the menu bar as the ceiling automatically.
                let screen_frame: NSRect = unsafe {
                    let screen: *mut AnyObject = msg_send![obj, screen];
                    if screen.is_null() {
                        let cls = match AnyClass::get(c"NSScreen") {
                            Some(c) => c,
                            None => return,
                        };
                        let main_screen: *mut AnyObject = msg_send![cls, mainScreen];
                        if main_screen.is_null() {
                            return;
                        }
                        msg_send![&*main_screen, visibleFrame]
                    } else {
                        msg_send![&*screen, visibleFrame]
                    }
                };
                let _ = tx.send((
                    screen_frame.origin.x,
                    screen_frame.origin.y,
                    screen_frame.size.width,
                    screen_frame.size.height,
                ));
            }
        })
        .map_err(|e| e.to_string())?;
        if let Ok(rect) = rx.recv_timeout(std::time::Duration::from_secs(1)) {
            return Ok(rect);
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(Some(monitor)) = win.current_monitor() {
            let pos = monitor.position();
            let size = monitor.size();
            let scale = win.scale_factor().unwrap_or(1.0);
            return Ok((
                pos.x as f64 / scale,
                pos.y as f64 / scale,
                size.width as f64 / scale,
                size.height as f64 / scale,
            ));
        }
    }
    Err("failed to get monitor rect".into())
}

/// Set the mini window's origin in logical coordinates.
/// macOS: bottom-left origin. Windows: top-left origin.
///
/// `confine` (default true) clamps the target into the current monitor's
/// rect. Set `Some(false)` for live drag flows so the user can pull the
/// mascot across to a neighbouring monitor — the per-monitor clamp here
/// is what previously made cross-monitor drag impossible.
#[tauri::command]
pub async fn set_mini_origin(
    app: tauri::AppHandle,
    x: f64,
    y: f64,
    confine: Option<bool>,
) -> Result<(), String> {
    let confine = confine.unwrap_or(true);
    log::info!(
        "[mini-pos] set_mini_origin request x={:.1} y={:.1} confine={}",
        x,
        y,
        confine
    );
    let win = app.get_webview_window("main").ok_or("mini not found")?;
    #[cfg(target_os = "macos")]
    {
        let win_clone = win.clone();
        app.run_on_main_thread(move || {
            use objc2::runtime::{AnyClass, AnyObject};
            use objc2::msg_send;
            use objc2_foundation::{NSRect, NSPoint, NSSize};
            if let Ok(ns_win) = win_clone.ns_window() {
                let obj = unsafe { &*(ns_win as *mut AnyObject) };
                let frame: NSRect = unsafe { msg_send![obj, frame] };
                let (clamped_x, clamped_y) = if confine {
                    let screen_frame: NSRect = unsafe {
                        let screen: *mut AnyObject = msg_send![obj, screen];
                        if screen.is_null() {
                            let cls = match AnyClass::get(c"NSScreen") {
                                Some(c) => c,
                                None => return,
                            };
                            let main_screen: *mut AnyObject = msg_send![cls, mainScreen];
                            if main_screen.is_null() {
                                return;
                            }
                            msg_send![&*main_screen, frame]
                        } else {
                            msg_send![&*screen, frame]
                        }
                    };
                    let min_x = screen_frame.origin.x;
                    let max_x = (screen_frame.origin.x + screen_frame.size.width - frame.size.width).max(min_x);
                    let min_y = screen_frame.origin.y;
                    // Keep collapsed mascot windows below top chrome. This also
                    // prevents stale persisted positions from parking the window
                    // under the notch/menu bar after startup.
                    let max_y = (screen_frame.origin.y + screen_frame.size.height - frame.size.height - MASCOT_TOP_INSET).max(min_y);
                    let cx = x.max(min_x).min(max_x);
                    let cy = y.max(min_y).min(max_y);
                    log::info!(
                        "[mini-pos] set_mini_origin(mac) clamped x={:.1}->{:.1} y={:.1}->{:.1} bounds x[{:.1},{:.1}] y[{:.1},{:.1}]",
                        x, cx, y, cy, min_x, max_x, min_y, max_y
                    );
                    (cx, cy)
                } else {
                    log::info!(
                        "[mini-pos] set_mini_origin(mac) unconfined x={:.1} y={:.1}",
                        x, y
                    );
                    (x, y)
                };
                let new_frame = NSRect::new(
                    NSPoint::new(clamped_x, clamped_y),
                    NSSize::new(frame.size.width, frame.size.height),
                );
                unsafe {
                    let _: () = msg_send![obj, setFrame: new_frame, display: true, animate: false];
                }
                if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                    *f = Some((new_frame.origin.x, new_frame.origin.y, new_frame.size.width, new_frame.size.height));
                }
            }
        }).map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        if !confine {
            log::info!(
                "[mini-pos] set_mini_origin(win) unconfined x={:.1} y={:.1}",
                x,
                y
            );
            let _ = win.set_position(tauri::LogicalPosition::new(x, y));
            return Ok(());
        }
        if let Ok(Some(monitor)) = win.current_monitor() {
            let scale = monitor.scale_factor();
            let mp = monitor.position();
            let mx = mp.x as f64 / scale;
            let my = mp.y as f64 / scale;
            let sw = monitor.size().width as f64 / scale;
            let sh = monitor.size().height as f64 / scale;
            let ui = win_ui_scale(&monitor);
            let (ww, wh) = win
                .outer_size()
                .map(|s| (s.width as f64 / scale, s.height as f64 / scale))
                .unwrap_or((0.0, 0.0));
            let min_x = mx;
            let max_x = (mx + sw - ww).max(min_x);
            let min_y = my + (MASCOT_TOP_INSET * ui).round();
            let max_y = (my + sh - wh).max(min_y);
            let clamped_x = x.max(min_x).min(max_x);
            let clamped_y = y.max(min_y).min(max_y);
            log::info!(
                "[mini-pos] set_mini_origin(win) clamped x={:.1}->{:.1} y={:.1}->{:.1} bounds x[{:.1},{:.1}] y[{:.1},{:.1}]",
                x, clamped_x, y, clamped_y, min_x, max_x, min_y, max_y
            );
            let _ = win.set_position(tauri::LogicalPosition::new(clamped_x, clamped_y));
        } else {
            log::info!(
                "[mini-pos] set_mini_origin(win,fallback) apply x={:.1} y={:.1} (with inset)",
                x,
                y + MASCOT_TOP_INSET
            );
            let _ = win.set_position(tauri::LogicalPosition::new(x, y + MASCOT_TOP_INSET));
        }
    }
    Ok(())
}

/// Kept as a compatibility no-op while macOS IME handling is fixed directly on
/// the underlying Wry webview class.
#[tauri::command]
pub async fn set_ime_mode(_app: tauri::AppHandle, _active: bool) -> Result<(), String> {
    Ok(())
}

/// Resize/reposition the mini window between collapsed (small, right of notch)
/// and expanded (larger, centered on notch) states.
#[tauri::command]
pub async fn set_mini_expanded(
    app: tauri::AppHandle,
    expanded: bool,
    position: Option<String>,
    efficiency: Option<bool>,
    max_height: Option<f64>,
    mascot_scale: Option<f64>,
    large_mascot: Option<bool>,
    keep_position: Option<bool>,
    large_mascot_scale: Option<f64>,
) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or("mini window not found")?;
    let pos = position.unwrap_or_else(|| "right".to_string());
    let mascot_scale = sanitized_mascot_scale(mascot_scale);
    let large_mascot_scale = large_mascot_scale.unwrap_or(LARGE_MASCOT_SIZE_MULTIPLIER);
    log::info!(
        "[mini-pos] set_mini_expanded request expanded={} pos={} efficiency={:?} keep_position={:?} large_mascot={:?} mascot_scale={:.2} large_scale={:.2}",
        expanded, pos, efficiency, keep_position, large_mascot, mascot_scale, large_mascot_scale
    );

    #[cfg(target_os = "macos")]
    {
        let win_clone = win.clone();
        app.run_on_main_thread(move || {
            use objc2::runtime::{AnyClass, AnyObject};
            use objc2::msg_send;
            use objc2_foundation::{NSRect, NSPoint, NSSize};

            if let Ok(ns_win) = win_clone.ns_window() {
                let obj = unsafe { &*(ns_win as *mut AnyObject) };

                let screen_info: Option<(f64, f64, f64, f64, f64)> = unsafe {
                    let screen: *mut AnyObject = msg_send![obj, screen];
                    if screen.is_null() {
                        let cls = match AnyClass::get(c"NSScreen") {
                            Some(c) => c,
                            None => return,
                        };
                        let main_screen: *mut AnyObject = msg_send![cls, mainScreen];
                        if main_screen.is_null() { return; }
                        let sf: NSRect = msg_send![&*main_screen, frame];
                        let notch_off = get_notch_offset(main_screen);
                        Some((sf.origin.x, sf.origin.y, sf.size.width, sf.size.height, notch_off))
                    } else {
                        let sf: NSRect = msg_send![&*screen, frame];
                        let notch_off = get_notch_offset(screen);
                        Some((sf.origin.x, sf.origin.y, sf.size.width, sf.size.height, notch_off))
                    }
                };

                if let Some((sx, sy, sw, sh, notch_off)) = screen_info {
                    // Cache screen geometry for the efficiency hover poll thread.
                    if let Ok(mut info) = NOTCH_SCREEN_INFO.lock() {
                        *info = Some((sx, sy, sw, sh, notch_off));
                    }
                    EFFICIENCY_EXPANDED.store(expanded, Ordering::SeqCst);

                    unsafe {
                        let _: () = msg_send![obj, setLevel: 27isize];
                    }
                    let (final_x, final_y, final_w, final_h) = if expanded {
                        let win_w = if efficiency.unwrap_or(false) { 600.0 } else { 500.0 };
                        let win_h = max_height.unwrap_or(350.0).max(200.0).min(500.0);
                        let x = sx + (sw - win_w) / 2.0;
                        // Expanded panel hugs the top of the screen (its window
                        // level is high enough to draw over the menu bar). The
                        // MASCOT_TOP_INSET only applies to the collapsed mascot
                        // so it stays clear of the notch.
                        let y = sy + sh - win_h;
                        log::info!(
                            "[mini-pos] set_mini_expanded(mac,expanded) frame x={:.1} y={:.1} w={:.1} h={:.1}",
                            x, y, win_w, win_h
                        );
                        let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(win_w, win_h));
                        unsafe {
                            let _: () = msg_send![obj, setFrame: frame, display: true, animate: false];
                            let ns_app_cls = AnyClass::get(c"NSApplication").unwrap();
                            let ns_app: *mut AnyObject = msg_send![ns_app_cls, sharedApplication];
                            let _: () = msg_send![&*ns_app, activateIgnoringOtherApps: true];
                            let null: *mut AnyObject = std::ptr::null_mut();
                            let _: () = msg_send![obj, makeKeyAndOrderFront: null];
                        }
                        (x, y, win_w, win_h)
                    } else {
                        let (win_w, win_h) = if large_mascot.unwrap_or(false) {
                            large_collapsed_mascot_window_size(mascot_scale, large_mascot_scale)
                        } else {
                            collapsed_mascot_window_size(mascot_scale)
                        };
                        let (mut x, mut y) = if keep_position.unwrap_or(false) {
                            let cur: NSRect = unsafe { msg_send![obj, frame] };
                            (cur.origin.x, cur.origin.y + cur.size.height - win_h)
                        } else if large_mascot.unwrap_or(false) {
                            let margin_x = 10.0;
                            let margin_y = 300.0;
                            (sx + sw - win_w - margin_x, sy + margin_y)
                        } else {
                            (
                                collapsed_x(sx, sw, win_w, &pos, notch_off),
                                sy + sh - win_h - MASCOT_TOP_INSET,
                            )
                        };
                        if !large_mascot.unwrap_or(false) {
                            let max_y = sy + sh - win_h - MASCOT_TOP_INSET;
                            if y > max_y { y = max_y; }
                        }
                        log::info!(
                            "[mini-pos] set_mini_expanded(mac,collapsed) frame x={:.1} y={:.1} w={:.1} h={:.1} keep_position={}",
                            x, y, win_w, win_h, keep_position.unwrap_or(false)
                        );
                        let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(win_w, win_h));
                        unsafe {
                            let _: () = msg_send![obj, setFrame: frame, display: true, animate: false];
                        }
                        (x, y, win_w, win_h)
                    };
                    // Cache the real window frame for the hover poll thread.
                    if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                        *f = Some((final_x, final_y, final_w, final_h));
                    }
                }
            }
        }).map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows: DPI-aware positioning and sizing.
        // Use monitor.position() to offset into the correct monitor in the virtual desktop.
        if let Ok(Some(monitor)) = win.current_monitor() {
            let scale = monitor.scale_factor();
            let mp = monitor.position();
            let mx = mp.x as f64 / scale;
            let my = mp.y as f64 / scale;
            let sw = monitor.size().width as f64 / scale;
            let ui = win_ui_scale(&monitor);
            if expanded {
                let base_w = if efficiency.unwrap_or(false) {
                    600.0
                } else {
                    500.0
                };
                let win_w = (base_w * ui).round();
                let win_h = (400.0 * ui).round();
                let x = mx + (sw - win_w) / 2.0;
                // Expanded panel hugs the top of the monitor (no inset) so it
                // does not get pushed below the IDE chrome.
                let y = my;
                log::info!(
                    "[mini-pos] set_mini_expanded(win,expanded) frame x={:.1} y={:.1} w={:.1} h={:.1} ui={:.2}",
                    x, y, win_w, win_h, ui
                );
                let _ = win.set_size(tauri::LogicalSize::new(win_w, win_h));
                let _ = win.set_position(tauri::LogicalPosition::new(x, y));
            } else {
                let (base_w, base_h) = if large_mascot.unwrap_or(false) {
                    large_collapsed_mascot_window_size(mascot_scale, large_mascot_scale)
                } else {
                    collapsed_mascot_window_size(mascot_scale)
                };
                let win_w = (base_w * ui).round();
                let win_h = (base_h * ui).round();
                let _ = win.set_size(tauri::LogicalSize::new(win_w, win_h));
                if !keep_position.unwrap_or(false) {
                    if large_mascot.unwrap_or(false) {
                        // Large mascot defaults to bottom-right corner.
                        let sh = monitor.size().height as f64 / scale;
                        let margin = (10.0 * ui).round();
                        let x = mx + sw - win_w - margin;
                        let y = my + sh - win_h - margin;
                        let _ = win.set_position(tauri::LogicalPosition::new(x, y));
                    } else {
                        let notch_off = (80.0 * ui).round();
                        let x = mx
                            + if pos == "left" {
                                sw / 2.0 - notch_off - win_w
                            } else {
                                sw / 2.0 + notch_off
                            };
                        let y = my + (MASCOT_TOP_INSET * ui).round();
                        log::info!(
                            "[mini-pos] set_mini_expanded(win,collapsed) frame x={:.1} y={:.1} w={:.1} h={:.1} keep_position={}",
                            x, y, win_w, win_h, keep_position.unwrap_or(false)
                        );
                        let _ = win.set_position(tauri::LogicalPosition::new(x, y));
                    }
                }
            }
        }
        if !FULLSCREEN_HIDING.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = win.set_always_on_top(true);
        }
    }

    Ok(())
}
#[tauri::command]
pub async fn resize_mini_height(
    app: tauri::AppHandle,
    height: f64,
    max_height: Option<f64>,
    animate: Option<bool>,
) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or("mini window not found")?;
    let limit = max_height.unwrap_or(350.0).max(200.0).min(2000.0);
    // Scale height limits on Windows to match DPI-aware window sizes
    #[cfg(target_os = "windows")]
    let h = {
        let ui = if let Ok(Some(m)) = win.current_monitor() {
            win_ui_scale(&m)
        } else {
            1.0
        };
        (height * ui).round().max(45.0 * ui).min(limit * ui)
    };
    #[cfg(not(target_os = "windows"))]
    let h = height.max(45.0).min(limit);

    #[cfg(target_os = "macos")]
    {
        let win_clone = win.clone();
        app.run_on_main_thread(move || {
            use objc2::msg_send;
            use objc2::runtime::{AnyClass, AnyObject};
            use objc2_foundation::{NSPoint, NSRect, NSSize};

            if let Ok(ns_win) = win_clone.ns_window() {
                let obj = unsafe { &*(ns_win as *mut AnyObject) };
                let screen: *mut AnyObject = unsafe { msg_send![obj, screen] };
                let screen_ptr = if screen.is_null() {
                    let cls = match AnyClass::get(c"NSScreen") {
                        Some(c) => c,
                        None => return,
                    };
                    let ms: *mut AnyObject = unsafe { msg_send![cls, mainScreen] };
                    if ms.is_null() {
                        return;
                    }
                    ms
                } else {
                    screen
                };
                let sf: NSRect = unsafe { msg_send![&*screen_ptr, frame] };
                let cur: NSRect = unsafe { msg_send![obj, frame] };
                let capped_h = h.min((sf.size.height * 0.75).max(200.0));
                // Top-aligned to the screen, matching the expanded panel's
                // initial placement in `set_mini_expanded`. No MASCOT_TOP_INSET
                // here — that inset only applies to the collapsed mascot.
                let new_y = sf.origin.y + sf.size.height - capped_h;
                let new_frame = NSRect::new(
                    NSPoint::new(cur.origin.x, new_y),
                    NSSize::new(cur.size.width, capped_h),
                );
                log::info!(
                    "[mini-pos] resize_mini_height(mac) frame x={:.1} y={:.1} w={:.1} h={:.1}",
                    cur.origin.x,
                    new_y,
                    cur.size.width,
                    capped_h
                );
                unsafe {
                    let do_animate: bool = animate.unwrap_or(false);
                    let _: () =
                        msg_send![obj, setFrame: new_frame, display: true, animate: do_animate];
                }
                if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                    *f = Some((cur.origin.x, new_y, cur.size.width, capped_h));
                }
            }
        })
        .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows: keep top-left position, just change height
        if let Ok(size) = win.outer_size() {
            let scale = win.scale_factor().unwrap_or(1.0);
            let _ = win.set_size(tauri::LogicalSize::new(size.width as f64 / scale, h));
        }
    }

    Ok(())
}

/// Expand the mini window to pet-context size and start a cursor-position poll
/// that toggles `setIgnoresMouseEvents:` — the transparent area around the
/// mascot passes clicks through to the desktop. When the context menu is open
/// (`PET_CONTEXT_MENU_OPEN`), the entire window accepts clicks.
///
/// Pass `active: false` to stop the poll and shrink back to collapsed size.
#[tauri::command]
pub async fn set_mini_size(
    app: tauri::AppHandle,
    restore: bool,
    position: Option<String>,
    keep_on_top: Option<bool>,
    pet_context: Option<bool>,
    mascot_scale: Option<f64>,
    large_mascot: Option<bool>,
    large_mascot_scale: Option<f64>,
) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or("mini window not found")?;
    let pos = position.unwrap_or_else(|| "right".to_string());
    let want_top = keep_on_top.unwrap_or(restore);
    let is_pet_context = pet_context.unwrap_or(false);
    let mascot_scale = sanitized_mascot_scale(mascot_scale);
    let large_mascot_scale = large_mascot_scale.unwrap_or(LARGE_MASCOT_SIZE_MULTIPLIER);

    #[cfg(target_os = "macos")]
    {
        let win_clone = win.clone();
        app.run_on_main_thread(move || {
            use objc2::runtime::{AnyClass, AnyObject};
            use objc2::msg_send;
            use objc2_foundation::{NSRect, NSPoint, NSSize};

            if let Ok(ns_win) = win_clone.ns_window() {
                let obj = unsafe { &*(ns_win as *mut AnyObject) };

                let screen_info: Option<(f64, f64, f64, f64, f64)> = unsafe {
                    let screen: *mut AnyObject = msg_send![obj, screen];
                    if screen.is_null() {
                        let cls = match AnyClass::get(c"NSScreen") {
                            Some(c) => c,
                            None => return,
                        };
                        let main_screen: *mut AnyObject = msg_send![cls, mainScreen];
                        if main_screen.is_null() { return; }
                        let sf: NSRect = msg_send![&*main_screen, frame];
                        let notch_off = get_notch_offset(main_screen);
                        Some((sf.origin.x, sf.origin.y, sf.size.width, sf.size.height, notch_off))
                    } else {
                        let sf: NSRect = msg_send![&*screen, frame];
                        let notch_off = get_notch_offset(screen);
                        Some((sf.origin.x, sf.origin.y, sf.size.width, sf.size.height, notch_off))
                    }
                };

                if let Some((sx, sy, sw, sh, notch_off)) = screen_info {
                    // Keep the hover poll's screen geometry cache fresh even when
                    // the mini window is temporarily resized into settings/update mode.
                    if let Ok(mut info) = NOTCH_SCREEN_INFO.lock() {
                        *info = Some((sx, sy, sw, sh, notch_off));
                    }
                    if is_pet_context {
                        if restore {
                            let current: NSRect = unsafe { msg_send![obj, frame] };
                            if let Ok(mut saved) = PET_MENU_RESTORE_FRAME.lock() {
                                if let Some((x, y, win_w, win_h)) = *saved {
                                    let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(win_w, win_h));
                                    // Hide window before shrink to avoid compositor
                                    // flashing the old large-frame content at the
                                    // wrong position inside the smaller window.
                                    unsafe {
                                        let _: () = msg_send![obj, setAlphaValue: 0.0f64];
                                        let _: () = msg_send![obj, setLevel: if want_top { 27isize } else { 0isize }];
                                        let _: () = msg_send![obj, setFrame: frame, display: true, animate: false];
                                        if want_top {
                                            let _: () = msg_send![obj, orderFrontRegardless];
                                        }
                                    }
                                    *saved = None;
                                    if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                                        *f = Some((x, y, win_w, win_h));
                                    }
                                    // Restore alpha after the webview repaints at
                                    // the new size.  dispatch_after on the main
                                    // queue with a
                                    // short delay lets the compositor
                                    // finish compositing the new frame.
                                    pet_context_schedule_restore_alpha(ns_win as *mut std::ffi::c_void);
                                    return;
                                }
                            }
                            // Fallback: if save frame is missing (e.g. race/double close),
                            // still collapse around current center instead of jumping to
                            // default corner placement.
                            let (target_w, target_h) = if large_mascot.unwrap_or(false) {
                                large_collapsed_mascot_window_size(mascot_scale, large_mascot_scale)
                            } else {
                                collapsed_mascot_window_size(mascot_scale)
                            };
                            let mut x = current.origin.x + (current.size.width - target_w) / 2.0;
                            let mut y = current.origin.y + (current.size.height - target_h) / 2.0;
                            x = x.max(sx).min(sx + sw - target_w);
                            y = y.max(sy).min(sy + sh - target_h);
                            let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(target_w, target_h));
                            unsafe {
                                let _: () = msg_send![obj, setAlphaValue: 0.0f64];
                                let _: () = msg_send![obj, setLevel: if want_top { 27isize } else { 0isize }];
                                let _: () = msg_send![obj, setFrame: frame, display: true, animate: false];
                                if want_top {
                                    let _: () = msg_send![obj, orderFrontRegardless];
                                }
                            }
                            if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                                *f = Some((x, y, target_w, target_h));
                            }
                            pet_context_schedule_restore_alpha(ns_win as *mut std::ffi::c_void);
                            return;
                        } else {
                            let current: NSRect = unsafe { msg_send![obj, frame] };
                            if let Ok(mut saved) = PET_MENU_RESTORE_FRAME.lock() {
                                *saved = Some((current.origin.x, current.origin.y, current.size.width, current.size.height));
                            }
                            // Expand LEFT and UP, keeping the bottom-right corner
                            // of the window fixed (macOS: origin.x+width, origin.y).
                            // The mascot stays at bottom-right via CSS absolute pos.
                            let left_pad = 180.0;
                            let top_pad = 100.0;
                            let win_w = (current.size.width + left_pad).min(sw);
                            let win_h = (current.size.height + top_pad).min(sh);
                            let mut x = current.origin.x + current.size.width - win_w;
                            let mut y = current.origin.y;
                            x = x.max(sx).min(sx + sw - win_w);
                            y = y.max(sy).min(sy + sh - win_h);
                            let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(win_w, win_h));
                            // Hide → resize → delayed restore, same as the
                            // shrink path, to prevent the old small-window
                            // content from flashing at the top-left of the
                            // newly expanded frame.
                            unsafe {
                                let _: () = msg_send![obj, setAlphaValue: 0.0f64];
                                let _: () = msg_send![obj, setLevel: if want_top { 27isize } else { 0isize }];
                                let _: () = msg_send![obj, setFrame: frame, display: true, animate: false];
                                if want_top {
                                    let _: () = msg_send![obj, orderFrontRegardless];
                                }
                            }
                            if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                                *f = Some((x, y, win_w, win_h));
                            }
                            pet_context_schedule_restore_alpha(ns_win as *mut std::ffi::c_void);
                            return;
                        }
                    }
                    if restore {
                        let (win_w, win_h) = if large_mascot.unwrap_or(false) {
                            large_collapsed_mascot_window_size(mascot_scale, large_mascot_scale)
                        } else {
                            collapsed_mascot_window_size(mascot_scale)
                        };
                        let (x, y) = if large_mascot.unwrap_or(false) {
                            let margin = 10.0;
                            (sx + sw - win_w - margin, sy + margin)
                        } else {
                            (
                                collapsed_x(sx, sw, win_w, &pos, notch_off),
                                sy + sh - win_h - MASCOT_TOP_INSET,
                            )
                        };
                        let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(win_w, win_h));
                        unsafe {
                            let _: () = msg_send![obj, setLevel: if want_top { 27isize } else { 0isize }];
                            let _: () = msg_send![obj, setFrame: frame, display: true, animate: false];
                            if want_top {
                                let _: () = msg_send![obj, orderFrontRegardless];
                            }
                        }
                        // Restoring from settings/update mode returns the widget to
                        // the collapsed notch state, so the hover poll must switch
                        // back to collapsed-region detection immediately.
                        EFFICIENCY_EXPANDED.store(false, Ordering::SeqCst);
                        if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                            *f = Some((x, y, win_w, win_h));
                        }
                    } else {
                        let win_w = (sw * 0.85).round();
                        let win_h = (sh * 0.85).round();
                        let x = sx + (sw - win_w) / 2.0;
                        let y = sy + sh - win_h;
                        let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(win_w, win_h));
                        unsafe {
                            let _: () = msg_send![obj, setLevel: if want_top { 27isize } else { 0isize }];
                            let _: () = msg_send![obj, setFrame: frame, display: true, animate: false];
                            if want_top {
                                let _: () = msg_send![obj, orderFrontRegardless];
                            }
                        }
                        // Settings/update mode is not the normal expanded panel.
                        // Clear the expanded hover state so a stale panel frame does
                        // not survive after the window is later restored.
                        EFFICIENCY_EXPANDED.store(false, Ordering::SeqCst);
                        if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                            *f = Some((x, y, win_w, win_h));
                        }
                    }
                }
            }
        }).map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(Some(monitor)) = win.current_monitor() {
            let scale = monitor.scale_factor();
            let mp = monitor.position();
            let mx = mp.x as f64 / scale;
            let my = mp.y as f64 / scale;
            let sw = monitor.size().width as f64 / scale;
            let sh = monitor.size().height as f64 / scale;
            let ui = win_ui_scale(&monitor);
            if restore {
                let (base_w, base_h) = if large_mascot.unwrap_or(false) {
                    large_collapsed_mascot_window_size(mascot_scale, large_mascot_scale)
                } else {
                    collapsed_mascot_window_size(mascot_scale)
                };
                let win_w = (base_w * ui).round();
                let win_h = (base_h * ui).round();
                let _ = win.set_size(tauri::LogicalSize::new(win_w, win_h));
                let _ = win.set_always_on_top(
                    want_top && !FULLSCREEN_HIDING.load(std::sync::atomic::Ordering::SeqCst),
                );
                if large_mascot.unwrap_or(false) {
                    let margin = (10.0 * ui).round();
                    let x = mx + sw - win_w - margin;
                    let y = my + sh - win_h - margin;
                    let _ = win.set_position(tauri::LogicalPosition::new(x, y));
                } else {
                    let notch_off = (80.0 * ui).round();
                    let x = mx
                        + if pos == "left" {
                            sw / 2.0 - notch_off - win_w
                        } else {
                            sw / 2.0 + notch_off
                        };
                    let _ = win.set_position(tauri::LogicalPosition::new(
                        x,
                        my + (MASCOT_TOP_INSET * ui).round(),
                    ));
                }
            } else {
                let win_w = (sw * 0.85).round();
                let win_h = (sh * 0.85).round();
                let x = mx + (sw - win_w) / 2.0;
                let _ = win.set_always_on_top(
                    want_top && !FULLSCREEN_HIDING.load(std::sync::atomic::Ordering::SeqCst),
                );
                let _ = win.set_size(tauri::LogicalSize::new(win_w, win_h));
                let _ = win.set_position(tauri::LogicalPosition::new(x, my));
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn open_detail_panel(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("detail") {
        win.show().map_err(|e| e.to_string())?;
        win.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let win =
        WebviewWindowBuilder::new(&app, "detail", WebviewUrl::App("index.html#/detail".into()))
            .title("PawBae - Detail")
            .inner_size(480.0, 600.0)
            .decorations(true)
            .resizable(true)
            .center()
            .build()
            .map_err(|e| e.to_string())?;
    let _ = win.maximize();

    Ok(())
}

#[tauri::command]
pub async fn reassert_floating(app: tauri::AppHandle) -> Result<(), String> {
    reassert_mini_floating(&app);
    Ok(())
}
