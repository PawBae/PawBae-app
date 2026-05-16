//! Drag handling: cursor position, mouse buttons, drag-apply scheduling.

use std::sync::atomic::Ordering;
use tauri::Manager;

use crate::state::*;

/// Schedule a main-thread task that snaps the mini window origin to
/// `(cursor_now - DRAG_ANCHOR)` — i.e. wherever the cursor currently is,
/// minus the offset captured at drag-start. Calls coalesce: while a task
/// is in flight, repeated invocations are no-ops; the running task always
/// reads the freshest cursor position. This keeps drag tracking tight
/// even when the poll thread runs much faster than the main thread can
/// repaint, and avoids the cumulative lag of relative-delta translation.
pub(crate) fn request_drag_apply(app: &tauri::AppHandle) {
    if DRAG_TASK_PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app_clone = app.clone();
    let _ = app.run_on_main_thread(move || {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;
        use objc2_foundation::NSPoint;

        DRAG_TASK_PENDING.store(false, Ordering::SeqCst);
        let anchor = drag_anchor().lock().ok().and_then(|g| *g);
        let Some((ax, ay)) = anchor else { return };

        let cursor = macos_cursor_position();
        let new_origin = NSPoint::new(cursor.0 - ax, cursor.1 - ay);

        if let Some(win) = app_clone.get_webview_window("main") {
            if let Ok(ns_win) = win.ns_window() {
                let obj = unsafe { &*(ns_win as *mut AnyObject) };
                // setFrameOrigin: only moves the window — it does not
                // redraw the contents — so it is far cheaper than
                // setFrame:display:animate:NO and keeps up with fast
                // cursor motion.
                unsafe {
                    let _: () = msg_send![obj, setFrameOrigin: new_origin];
                }
                if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                    if let Some((_, _, w, h)) = *f {
                        *f = Some((new_origin.x, new_origin.y, w, h));
                    }
                }
            }
        }
    });
}
/// Read the current mouse cursor position via `[NSEvent mouseLocation]`.
/// Returns (x, y) in macOS screen coordinates (bottom-left origin).
pub(crate) fn macos_cursor_position() -> (f64, f64) {
    unsafe {
        use objc2::msg_send;
        use objc2_foundation::NSPoint;
        if let Some(cls) = objc2::runtime::AnyClass::get(c"NSEvent") {
            let loc: NSPoint = msg_send![cls, mouseLocation];
            (loc.x, loc.y)
        } else {
            (0.0, 0.0)
        }
    }
}
/// Returns the bitmask of currently pressed mouse buttons via
/// `[NSEvent pressedMouseButtons]`. Bit 0 = left button. This works
/// regardless of whether the receiving window is the key window, which is
/// what we need to detect drags on the floating mini mascot.
pub(crate) fn macos_pressed_mouse_buttons() -> usize {
    unsafe {
        use objc2::msg_send;
        if let Some(cls) = objc2::runtime::AnyClass::get(c"NSEvent") {
            let mask: usize = msg_send![cls, pressedMouseButtons];
            mask
        } else {
            0
        }
    }
}
