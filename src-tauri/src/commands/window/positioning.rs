//! Window positioning commands: move_mini_by, get_mini_origin, get_mini_monitor_rect, set_mini_origin, set_ime_mode.

use tauri::Manager;

use crate::mascot::MASCOT_TOP_INSET;
#[cfg(target_os = "macos")]
use crate::state::MINI_WINDOW_FRAME;
#[cfg(target_os = "macos")]
use crate::mascot::current_sprite_pad;
#[cfg(target_os = "macos")]
use crate::state::PET_MENU_RESTORE_FRAME;

#[cfg(target_os = "windows")]
use crate::platform::windows::win_ui_scale;

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
