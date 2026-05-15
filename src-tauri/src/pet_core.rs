//! Pet-mode helpers: efficiency_hover_poll, mini-window floating reassert, codex utility-session detection.

use std::sync::atomic::Ordering;

use tauri::Emitter;

use crate::state::{
    drag_anchor, ClaudeSession, EFFICIENCY_EXPANDED, EFFICIENCY_HOVER_ACTIVE,
    EFFICIENCY_HOVER_THREAD_ALIVE, MINI_WINDOW_FRAME, NOTCH_SCREEN_INFO, THROW_TRACKING_ENABLED,
};

#[cfg(target_os = "macos")]
use crate::platform::macos::{
    macos_cursor_position, macos_pressed_mouse_buttons, request_drag_apply,
};

/// Background polling loop for efficiency-mode hover.
/// Checks the cursor position against two regions:
///  - **Collapsed**: a wide strip around the notch (notch_off*2 + 200 px,
///    50 px tall at the top of the screen) — much wider than the actual
///    window so the user can approach from either side.
///  - **Expanded**: the panel area (500 × 400 px, top-center).
pub(crate) fn efficiency_hover_poll(app: tauri::AppHandle) {
    use std::time::{Duration, Instant};
    EFFICIENCY_HOVER_THREAD_ALIVE.store(true, Ordering::SeqCst);
    let mut was_inside = false;
    let mut was_over_mascot = false;
    let mut last_enter_emit = Instant::now();
    // Drag state machine, driven entirely by NSEvent.pressedMouseButtons +
    // NSEvent.mouseLocation. The webview cannot observe mouseDown on a
    // non-key floating window, so the JS-side drag would otherwise need a
    // priming click. We mirror codex's approach: poll cursor + button,
    // translate the mini NSWindow ourselves, and emit walk-dir events to
    // the frontend so the codex sprite shows run-left/run-right.
    let mut drag_active = false;
    let mut last_cursor: (f64, f64) = (0.0, 0.0);
    let mut last_walk_dir: i32 = 0;
    let mut was_pressed = false;
    // Used only for run-left/right detection — measured between successive
    // poll iterations. Window translation itself is anchor-based and lives
    // in request_drag_apply (which reads the live cursor on main thread).

    // Drag-throw velocity sampling buffer (Phase 2 pet physics).
    // Holds a sliding window of (timestamp, dx, dy_topdown) entries while
    // the user drags the mascot. On release we average the most recent
    // ~80 ms of samples to derive an initial velocity for the falling
    // animation. Disabled by default; enabled by the frontend through
    // `set_throw_tracking` once the user picks a physics-capable pet
    // and stroll-mode is on.
    // `Instant` is already in scope from the function-top
    // `use std::time::{Duration, Instant};`.
    use std::collections::VecDeque;
    let mut throw_samples: VecDeque<(Instant, f64, f64)> = VecDeque::with_capacity(32);
    // 250ms is a wider window than the typical 80ms peak-velocity grab.
    // Users instinctively settle the cursor for a beat before releasing,
    // so a tighter window often averages mostly-zero samples.
    const THROW_SAMPLE_CAP: usize = 24;
    const THROW_AVG_WINDOW_MS: u128 = 250;
    const MAX_THROW_SPEED: f64 = 30.0;

    while EFFICIENCY_HOVER_ACTIVE.load(Ordering::SeqCst) {
        let info = NOTCH_SCREEN_INFO.lock().ok().and_then(|g| *g);
        let sleep_ms = if let Some((sx, sy, sw, sh, notch_off)) = info {
            let cursor = macos_cursor_position();
            let buttons = macos_pressed_mouse_buttons();
            let left_pressed = (buttons & 1) != 0;
            let is_expanded = EFFICIENCY_EXPANDED.load(Ordering::SeqCst);
            let frame = MINI_WINDOW_FRAME.lock().ok().and_then(|g| *g);

            let inside = if is_expanded {
                if let Some((fx, fy, fw, fh)) = frame {
                    cursor.0 >= fx && cursor.0 <= fx + fw && cursor.1 >= fy && cursor.1 <= fy + fh
                } else {
                    false
                }
            } else {
                let rw = (notch_off * 2.0 + 10.0).max(80.0);
                let rh = frame
                    .map(|(_, _, _, fh)| fh.clamp(20.0, 28.0))
                    .unwrap_or(35.0);
                let rx = sx + (sw - rw) / 2.0;
                let ry = sy + sh - rh;
                cursor.0 >= rx && cursor.0 <= rx + rw && cursor.1 >= ry && cursor.1 <= ry + rh
            };

            if inside && !was_inside {
                let _ = app.emit("efficiency-hover", true);
                last_enter_emit = Instant::now();
            } else if inside && was_inside && last_enter_emit.elapsed() > Duration::from_millis(300)
            {
                let _ = app.emit("efficiency-hover", true);
                last_enter_emit = Instant::now();
            } else if !inside && was_inside {
                let _ = app.emit("efficiency-hover", false);
            }
            was_inside = inside;

            // ── Mascot body hit-test ──
            // Use a tighter rect than the full 96x96 window: the codex
            // 192x208 cell paints the character roughly in its centre with
            // transparent margins (and the status badge lives in the
            // bottom-right corner). Hover/drag should only fire on the
            // visible body, so we inset to ~35% wide x 65% tall around the
            // upper-centre where the head/torso sit.
            let over_mascot = if is_expanded {
                false
            } else if let Some((fx, fy, fw, fh)) = frame {
                let l = fx + fw * 0.32;
                let r = fx + fw * 0.68;
                let b = fy + fh * 0.25; // NSEvent y axis grows upward
                let t = fy + fh * 0.90;
                cursor.0 >= l && cursor.0 <= r && cursor.1 >= b && cursor.1 <= t
            } else {
                false
            };

            // ── Drag state machine ──
            // Only engage in collapsed (mascot) state, never in expanded
            // panel mode (clicks inside the panel must keep their normal
            // webview behavior).
            if !is_expanded {
                if drag_active {
                    if left_pressed {
                        // Always request a fresh window-snap; the main-thread
                        // task reads cursor position itself, so even if many
                        // requests collapse into one, the window still ends
                        // up under the live cursor.
                        request_drag_apply(&app);
                        let dx = cursor.0 - last_cursor.0;
                        // macOS NSEvent y axis is bottom-up; flip to
                        // top-down so the throw velocity matches the
                        // frontend physics convention.
                        let dy_topdown = -(cursor.1 - last_cursor.1);
                        last_cursor = cursor;
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
                        if THROW_TRACKING_ENABLED.load(Ordering::SeqCst) {
                            let now = Instant::now();
                            throw_samples.push_back((now, dx, dy_topdown));
                            while throw_samples.len() > THROW_SAMPLE_CAP {
                                throw_samples.pop_front();
                            }
                        }
                    } else {
                        // Drag finished. Clear anchor + walk dir and notify
                        // the frontend so it can persist the new origin.
                        drag_active = false;
                        if let Ok(mut a) = drag_anchor().lock() {
                            *a = None;
                        }
                        if last_walk_dir != 0 {
                            let _ = app.emit("mini-mascot-walk", 0i32);
                            last_walk_dir = 0;
                        }
                        // Compute drag-release velocity from the most
                        // recent ~80 ms of samples. Older samples are
                        // dropped so a long pause before release doesn't
                        // dilute the final velocity.
                        if THROW_TRACKING_ENABLED.load(Ordering::SeqCst)
                            && !throw_samples.is_empty()
                        {
                            let cutoff = Instant::now();
                            // Average only over samples where the cursor
                            // actually moved. Users typically pause for
                            // ~50–150ms before releasing, so naively
                            // averaging the last fixed window picks up
                            // mostly zero samples and the throw lands
                            // at zero velocity.
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
                                let avg_dx = sum_dx / count as f64;
                                let avg_dy = sum_dy / count as f64;
                                let vx = avg_dx.clamp(-MAX_THROW_SPEED, MAX_THROW_SPEED);
                                let vy = avg_dy.clamp(-MAX_THROW_SPEED, MAX_THROW_SPEED);
                                log::info!(
                                    "[drag-throw] samples={}/{} avg_dx={:.2} avg_dy={:.2} → vx={:.2} vy={:.2}",
                                    count, total_seen, avg_dx, avg_dy, vx, vy,
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
                } else if over_mascot && left_pressed && !was_pressed {
                    drag_active = true;
                    last_cursor = cursor;
                    // Reset the velocity sampling buffer; previous
                    // samples (from the last drag) must not bleed into
                    // the new throw.
                    throw_samples.clear();
                    // Capture the cursor-to-origin offset at drag start so
                    // the main-thread task can place the window absolutely
                    // each frame instead of summing deltas.
                    if let Some((fx, fy, _, _)) = frame {
                        if let Ok(mut a) = drag_anchor().lock() {
                            *a = Some((cursor.0 - fx, cursor.1 - fy));
                        }
                    }
                    // Cancel any active hover so the sprite immediately
                    // switches from `jumping` to its base/run state when
                    // the drag begins.
                    if was_over_mascot {
                        let _ = app.emit("mini-mascot-hover", false);
                        was_over_mascot = false;
                    }
                    // Stroll-mode physics needs an explicit drag-start
                    // signal so it can suspend the gravity tick while
                    // the user holds the mascot. The existing
                    // mini-mascot-walk event only fires on horizontal
                    // motion, so a click-and-hold without lateral drag
                    // would otherwise leave physics running underneath.
                    log::info!(
                        "[drag-start] cursor=({:.1},{:.1}) tracking={}",
                        cursor.0,
                        cursor.1,
                        THROW_TRACKING_ENABLED.load(Ordering::SeqCst),
                    );
                    let _ = app.emit("mini-mascot-drag-start", ());
                }
            } else if drag_active {
                drag_active = false;
                throw_samples.clear();
                if let Ok(mut a) = drag_anchor().lock() {
                    *a = None;
                }
                if last_walk_dir != 0 {
                    let _ = app.emit("mini-mascot-walk", 0i32);
                    last_walk_dir = 0;
                }
            }
            was_pressed = left_pressed;

            // Hover signal is suppressed while dragging so the sprite
            // shows run-left/run-right instead of jumping.
            let hover_signal = over_mascot && !drag_active;
            if hover_signal != was_over_mascot {
                let _ = app.emit("mini-mascot-hover", hover_signal);
                was_over_mascot = hover_signal;
            }

            // Adaptive polling: fastest while dragging (60fps) so the
            // window keeps up with the cursor; slower when just hovering;
            // very slow when far from the mascot to save battery.
            if drag_active {
                16
            } else if is_expanded || inside || over_mascot {
                30
            } else {
                let screen_top = sy + sh;
                let dist_from_top = screen_top - cursor.1;
                let near_mascot = frame
                    .map(|(fx, fy, fw, fh)| {
                        cursor.0 >= fx - 80.0
                            && cursor.0 <= fx + fw + 80.0
                            && cursor.1 >= fy - 80.0
                            && cursor.1 <= fy + fh + 80.0
                    })
                    .unwrap_or(false);
                if near_mascot || dist_from_top < 200.0 {
                    50
                } else {
                    500
                }
            }
        } else {
            500
        };
        std::thread::sleep(Duration::from_millis(sleep_ms));
    }
    EFFICIENCY_HOVER_THREAD_ALIVE.store(false, Ordering::SeqCst);
}
#[cfg(not(target_os = "macos"))]
fn macos_cursor_position() -> (f64, f64) {
    (0.0, 0.0)
}
// Non-macOS stubs: the efficiency hover / notch drag tracker is a macOS-only
// feature (driven by NSEvent), but the polling loop itself is not gated, so
// we provide no-op implementations on other platforms to keep the build
// happy. The poll loop never engages drag here because `NOTCH_SCREEN_INFO`
// stays unset on Windows/Linux.
#[cfg(not(target_os = "macos"))]
fn macos_pressed_mouse_buttons() -> usize {
    0
}
#[cfg(not(target_os = "macos"))]
fn request_drag_apply(_app: &tauri::AppHandle) {}
pub(crate) fn reassert_mini_floating(app: &tauri::AppHandle) {
    use tauri::Manager;
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let win_clone = win.clone();
    let _ = app.run_on_main_thread(move || {
        #[cfg(target_os = "macos")]
        {
            use objc2::msg_send;
            use objc2::runtime::AnyObject;
            if let Ok(ns_win) = win_clone.ns_window() {
                let obj = unsafe { &*(ns_win as *mut AnyObject) };
                unsafe {
                    let _: () = msg_send![obj, setLevel: 27isize];
                    let behavior: usize = (1 << 0) | (1 << 4) | (1 << 8) | (1 << 6);
                    let _: () = msg_send![obj, setCollectionBehavior: behavior];
                }
            }
        }
        let _ = win_clone.set_always_on_top(true);
    });
}
pub(crate) fn is_codex_internal_utility_session(session: &ClaudeSession) -> bool {
    if session.source != "codex" {
        return false;
    }

    let prompt = session.user_prompt.as_deref().unwrap_or("");
    if prompt.starts_with("You are a helpful assistant. You will be presented with a user prompt") {
        return true;
    }

    let last = session.last_response.as_deref().unwrap_or("").trim_start();
    last.starts_with("{\"title\":")
}
