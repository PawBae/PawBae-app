//! Window expansion/sizing commands: set_mini_expanded, resize_mini_height, set_mini_size.

use tauri::Manager;

#[cfg(target_os = "macos")]
use crate::mascot::collapsed_x;
use crate::mascot::{
    collapsed_mascot_window_size, large_collapsed_mascot_window_size, sanitized_mascot_scale,
    LARGE_MASCOT_SIZE_MULTIPLIER, MASCOT_TOP_INSET,
};
#[cfg(target_os = "macos")]
use crate::platform::macos::{get_notch_offset, pet_context_schedule_restore_alpha};
#[cfg(target_os = "macos")]
use crate::state::MINI_WINDOW_FRAME;
#[cfg(target_os = "macos")]
use crate::state::{EFFICIENCY_EXPANDED, NOTCH_SCREEN_INFO, PET_MENU_RESTORE_FRAME};
#[cfg(target_os = "macos")]
use std::sync::atomic::Ordering;

#[cfg(target_os = "windows")]
use crate::platform::windows::win_ui_scale;
#[cfg(target_os = "windows")]
use crate::state::FULLSCREEN_HIDING;
#[cfg(target_os = "windows")]
use crate::state::MINI_WINDOW_FRAME;

/// Resize/reposition the mini window between collapsed (small, right of notch)
/// and expanded (larger, centered on notch) states.
#[tauri::command]
#[allow(unused_variables, clippy::too_many_arguments)]
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
                        let win_h = max_height.unwrap_or(350.0).clamp(200.0, 500.0);
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
                        let (x, mut y) = if keep_position.unwrap_or(false) {
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
        // Cache frame for the Windows poll thread (hover + drag).
        if let Ok(pos) = win.outer_position() {
            if let Ok(size) = win.outer_size() {
                let scale = win.scale_factor().unwrap_or(1.0);
                if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                    *f = Some((
                        pos.x as f64 / scale,
                        pos.y as f64 / scale,
                        size.width as f64 / scale,
                        size.height as f64 / scale,
                    ));
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
#[allow(unused_variables)]
pub async fn resize_mini_height(
    app: tauri::AppHandle,
    height: f64,
    max_height: Option<f64>,
    animate: Option<bool>,
) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or("mini window not found")?;
    let limit = max_height.unwrap_or(350.0).clamp(200.0, 2000.0);
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
#[allow(unused_variables, clippy::too_many_arguments)]
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
                                    pet_context_schedule_restore_alpha(ns_win);
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
                            pet_context_schedule_restore_alpha(ns_win);
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
                            pet_context_schedule_restore_alpha(ns_win);
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
