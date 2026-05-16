//! Pet interaction: passthrough polling and alpha-restore scheduling.

use std::sync::atomic::Ordering;
use tauri::Manager;

use crate::mascot::large_collapsed_mascot_window_size;
use crate::state::*;

use super::drag::macos_cursor_position;

/// Schedule restoring the NSWindow alpha to 1.0 after the webview has had
/// time to composite at the new frame size.  Uses GCD `dispatch_after_f` on
/// the main queue so the restore runs at a precise time without thread-spawn
/// overhead.  A generation counter (`PET_ALPHA_GEN`) prevents stale callbacks
/// from restoring alpha during a subsequent resize (fast double-clicks).
pub(crate) fn pet_context_schedule_restore_alpha(ns_win_ptr: *mut std::ffi::c_void) {
    extern "C" {
        // dispatch_get_main_queue() is a C macro; the real symbol is a global.
        #[link_name = "_dispatch_main_q"]
        static DISPATCH_MAIN_Q: std::ffi::c_void;
        fn dispatch_after_f(
            when: u64,
            queue: *const std::ffi::c_void,
            context: *mut std::ffi::c_void,
            work: extern "C" fn(*mut std::ffi::c_void),
        );
        fn dispatch_time(when: u64, delta: i64) -> u64;
    }

    /// Packed context passed through GCD void* pointer.
    struct RestoreCtx {
        ns_win: *mut std::ffi::c_void,
        gen: u64,
    }

    extern "C" fn restore_alpha(ctx_raw: *mut std::ffi::c_void) {
        let ctx = unsafe { Box::from_raw(ctx_raw as *mut RestoreCtx) };
        // Only restore if no newer resize has happened since we were scheduled.
        if PET_ALPHA_GEN.load(Ordering::SeqCst) != ctx.gen {
            return;
        }
        use objc2::msg_send;
        let obj = unsafe { &*(ctx.ns_win as *const objc2::runtime::AnyObject) };
        unsafe {
            let _: () = msg_send![obj, setAlphaValue: 1.0f64];
        }
    }

    let gen = PET_ALPHA_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let ctx = Box::new(RestoreCtx {
        ns_win: ns_win_ptr,
        gen,
    });
    unsafe {
        // 34ms ≈ 2 frames at 60Hz — minimal delay for the webview to
        // finish compositing at the new window size.
        let when = dispatch_time(0, 34_000_000); // nanoseconds
        dispatch_after_f(
            when,
            &DISPATCH_MAIN_Q as *const std::ffi::c_void,
            Box::into_raw(ctx) as *mut std::ffi::c_void,
            restore_alpha,
        );
    }
}

/// Polling loop for pet-mode click pass-through. Checks cursor position every
/// 20ms. When the cursor is over the mascot (bottom-right of the expanded
/// window) or the context menu is open, `setIgnoresMouseEvents: false` so the
/// webview receives events. Otherwise `setIgnoresMouseEvents: true` so clicks
/// pass through to whatever is behind.
pub(crate) fn pet_passthrough_poll(
    app: tauri::AppHandle,
    mascot_scale: f64,
    large_mascot_scale: f64,
) {
    use std::time::Duration;
    PET_PASSTHROUGH_THREAD_ALIVE.store(true, Ordering::SeqCst);
    let mut was_interactive = false;
    let (mascot_w, mascot_h) = large_collapsed_mascot_window_size(mascot_scale, large_mascot_scale);
    // Keep these ratios aligned with frontend `Mini.tsx` so hover cursor and
    // native pass-through behavior remain consistent around mascot edges.
    let hit_w = mascot_w * (2.4 / 3.0);
    let hit_h = mascot_h * (2.8 / 3.0);
    let inset_x = (mascot_w - hit_w) / 2.0;
    let inset_y = (mascot_h - hit_h) / 2.0;
    let edge_threshold = 30.0;
    // Get screen bounds once at startup so we can detect edge proximity.
    let screen_bounds: Option<(f64, f64, f64, f64)> = {
        let (tx, rx) = std::sync::mpsc::channel();
        let app_c = app.clone();
        let _ = app_c.run_on_main_thread(move || {
            use objc2::msg_send;
            use objc2::runtime::{AnyClass, AnyObject};
            use objc2_foundation::NSRect;
            let result: Option<(f64, f64, f64, f64)> = unsafe {
                AnyClass::get(c"NSScreen").and_then(|cls| {
                    let ms: *mut AnyObject = msg_send![cls, mainScreen];
                    if ms.is_null() {
                        None
                    } else {
                        let sf: NSRect = msg_send![&*ms, frame];
                        Some((sf.origin.x, sf.origin.y, sf.size.width, sf.size.height))
                    }
                })
            };
            let _ = tx.send(result);
        });
        rx.recv().ok().flatten()
    };

    while PET_PASSTHROUGH_ACTIVE.load(Ordering::SeqCst) {
        let menu_open = PET_CONTEXT_MENU_OPEN.load(Ordering::SeqCst);
        let pomodoro_active = PET_POMODORO_ACTIVE.load(Ordering::SeqCst);
        let frame = MINI_WINDOW_FRAME.lock().ok().and_then(|g| *g);

        let should_be_interactive = if menu_open || pomodoro_active {
            true
        } else if let Some((fx, fy, fw, _fh)) = frame {
            let cursor = macos_cursor_position();
            let mascot_left = fx + fw - mascot_w;
            let mascot_right = mascot_left + mascot_w;
            let mascot_bottom = fy;
            // Drop hitbox insets when the mascot extends near/past a screen
            // edge so the visible portion stays fully clickable.
            let near_edge = if let Some((sx, _sy, sw, _sh)) = screen_bounds {
                mascot_left < sx + edge_threshold || mascot_right > sx + sw - edge_threshold
            } else {
                mascot_left < edge_threshold
            };
            // Near screen edge, keep hitbox reasonably generous but never full-rect.
            // Full-rect near-edge hitboxes make peek feel "too clickable" and steal
            // hover/clicks away from nearby desktop content.
            let ix = if near_edge { inset_x * 0.5 } else { inset_x };
            let iy = inset_y;
            let hit_left = mascot_left + ix;
            let hit_right = mascot_right - ix;
            let hit_bottom = mascot_bottom + iy;
            let hit_top = mascot_bottom + mascot_h - iy;
            cursor.0 >= hit_left
                && cursor.0 <= hit_right
                && cursor.1 >= hit_bottom
                && cursor.1 <= hit_top
        } else {
            false
        };

        if should_be_interactive != was_interactive {
            let app1 = app.clone();
            let app2 = app.clone();
            let val = should_be_interactive;
            let _ = app1.run_on_main_thread(move || {
                if let Some(win) = app2.get_webview_window("main") {
                    if let Ok(ns_win) = win.ns_window() {
                        use objc2::msg_send;
                        let obj = unsafe { &*(ns_win as *mut objc2::runtime::AnyObject) };
                        unsafe {
                            let _: () = msg_send![obj, setIgnoresMouseEvents: !val];
                        }
                    }
                }
            });
            was_interactive = should_be_interactive;
        }

        std::thread::sleep(Duration::from_millis(20));
    }

    // Ensure events are re-enabled when the thread exits.
    let app_exit = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(win) = app_exit.get_webview_window("main") {
            if let Ok(ns_win) = win.ns_window() {
                use objc2::msg_send;
                let obj = unsafe { &*(ns_win as *mut objc2::runtime::AnyObject) };
                unsafe {
                    let _: () = msg_send![obj, setIgnoresMouseEvents: false];
                }
            }
        }
    });
    PET_PASSTHROUGH_THREAD_ALIVE.store(false, Ordering::SeqCst);
}
