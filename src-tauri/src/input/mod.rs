//! Global input-event collection: privacy-safe keyboard/mouse activity sensing.
//!
//! Three layers:
//! 1. **capture**   — a platform `InputListener` records only the *kind* of each
//!    event (never key codes, characters, coordinates, window titles, or app
//!    identity), bumping a counter in the shared [`aggregator::InputAggregator`].
//! 2. **aggregate** — the pure, testable aggregator batches bursts.
//! 3. **emit**      — a background flush thread drains the aggregator every
//!    [`FLUSH_INTERVAL_MS`] and emits one `user-input` event per non-empty kind.
//!
//! Phase 1 ships a macOS capture backend (NSEvent global monitors). Other
//! platforms get a no-op listener — a real Windows backend is deferred to
//! Phase 2 — but the whole module still compiles cleanly everywhere.

pub mod aggregator;

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::state::{lock_or_recover, InputState};

/// Flush cadence: batch input bursts into one event per kind per tick. 80 ms
/// sits inside the 50–100 ms window agreed with the frontend, so high-frequency
/// typing can never flood Svelte state or the sprite animation loop.
const FLUSH_INTERVAL_MS: u64 = 80;

/// Which input kinds are actually being captured right now, plus an optional
/// human-readable reason when a kind is degraded/off. Returned to the frontend
/// so settings/logs can surface the degraded state (e.g. keyboard off when
/// macOS Accessibility access is denied).
#[derive(Clone, Debug, Default, Serialize)]
pub struct ListenerStatus {
    pub keyboard: bool,
    pub mouse: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl ListenerStatus {
    fn disabled(reason: &str) -> Self {
        Self {
            keyboard: false,
            mouse: false,
            reason: Some(reason.to_string()),
        }
    }
}

/// Platform seam. macOS implements real capture; other platforms no-op.
/// A real Windows implementation is deferred to Phase 2.
trait InputListener: Send + Sync {
    /// Install OS hooks. Returns which kinds are actually active.
    fn start(
        &self,
        app: &tauri::AppHandle,
        state: &Arc<InputState>,
        generation: u64,
    ) -> ListenerStatus;
    /// Remove OS hooks.
    fn stop(&self, app: &tauri::AppHandle, state: &Arc<InputState>);
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn next_generation(state: &InputState) -> u64 {
    state.generation.fetch_add(1, Ordering::SeqCst) + 1
}

fn invalidate_generation(state: &InputState) {
    state.generation.fetch_add(1, Ordering::SeqCst);
}

fn active_for_generation(state: &InputState, generation: u64) -> bool {
    state.active.load(Ordering::SeqCst) && state.generation.load(Ordering::SeqCst) == generation
}

/// Background thread: drain the aggregator on a fixed cadence and emit one
/// `user-input` event per non-empty kind. Each thread is tied to one generation,
/// so quick stop/start cycles cannot let an old thread emit into a new session.
fn input_flush_loop(app: tauri::AppHandle, state: Arc<InputState>, generation: u64) {
    while active_for_generation(&state, generation) {
        std::thread::sleep(Duration::from_millis(FLUSH_INTERVAL_MS));
        // Cheap early-out: skip draining if tracking stopped during the sleep.
        // (The hard off boundary is enforced under `emit_gate` below.)
        if !active_for_generation(&state, generation) {
            break;
        }
        let events = match state.aggregator.lock() {
            Ok(mut agg) => {
                if agg.is_empty() {
                    continue;
                }
                agg.drain(now_ms())
            }
            Err(_) => continue,
        };
        // Serialize emission with stop_tracking: hold the emit gate and re-check
        // `active` under it. stop sets active=false then takes this same gate
        // before returning, so once stop has returned no `user-input` can fire —
        // a drained-but-unsent batch is dropped here instead.
        let _gate = lock_or_recover(&state.emit_gate);
        if !active_for_generation(&state, generation) {
            break;
        }
        for event in events {
            let _ = app.emit("user-input", event);
        }
    }
}

/// Begin capturing input. Idempotent: a second call while already active just
/// returns the current status. OFF by default — the frontend opts in via the
/// `set_input_tracking` command.
pub(crate) fn start_tracking(app: &tauri::AppHandle) -> ListenerStatus {
    let st = app.state::<Arc<InputState>>();
    let state = Arc::clone(&*st);
    if state.active.swap(true, Ordering::SeqCst) {
        return state.status.lock().map(|s| s.clone()).unwrap_or_default();
    }
    let generation = next_generation(&state);
    let status = make_listener().start(app, &state, generation);
    if let Ok(mut s) = state.status.lock() {
        *s = status.clone();
    }
    // Nothing is actually being captured (unsupported platform, or macOS with
    // no usable backend): don't leave `active` set or spawn a flush thread that
    // would wake every 80 ms forever doing nothing. Roll the activation back.
    if !status.keyboard && !status.mouse {
        state.active.store(false, Ordering::SeqCst);
        invalidate_generation(&state);
        return status;
    }
    let app2 = app.clone();
    let state2 = Arc::clone(&state);
    std::thread::spawn(move || input_flush_loop(app2, state2, generation));
    status
}

/// Stop capturing input and tear down OS hooks.
pub(crate) fn stop_tracking(app: &tauri::AppHandle) -> ListenerStatus {
    let st = app.state::<Arc<InputState>>();
    let state = Arc::clone(&*st);
    state.active.store(false, Ordering::SeqCst);
    invalidate_generation(&state);
    make_listener().stop(app, &state);
    // Take the emit gate (waits for any in-flight flush emit to finish), then
    // clear pending counts. Combined with the flush thread re-checking `active`
    // under the same gate, this makes "off" a hard boundary: once we return, no
    // further `user-input` can fire and no stale pre-stop counts survive a restart.
    {
        let _gate = lock_or_recover(&state.emit_gate);
        if let Ok(mut agg) = state.aggregator.lock() {
            agg.clear();
        }
    }
    let status = ListenerStatus::disabled("stopped");
    if let Ok(mut s) = state.status.lock() {
        *s = status.clone();
    }
    status
}

/// Current capture status (for the frontend to render settings/logs).
pub(crate) fn status(app: &tauri::AppHandle) -> ListenerStatus {
    let st = app.state::<Arc<InputState>>();
    st.status.lock().map(|s| s.clone()).unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn make_listener() -> Box<dyn InputListener> {
    Box::new(macos_listener::MacInputListener)
}

#[cfg(not(target_os = "macos"))]
fn make_listener() -> Box<dyn InputListener> {
    Box::new(NoopInputListener)
}

#[cfg(not(target_os = "macos"))]
struct NoopInputListener;

#[cfg(not(target_os = "macos"))]
impl InputListener for NoopInputListener {
    fn start(
        &self,
        _app: &tauri::AppHandle,
        _state: &Arc<InputState>,
        _generation: u64,
    ) -> ListenerStatus {
        // No global-input backend on this platform yet (Phase 2).
        ListenerStatus::disabled("platform-unsupported")
    }
    fn stop(&self, _app: &tauri::AppHandle, _state: &Arc<InputState>) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_invalidates_stale_tracking_sessions_after_restart() {
        let state = InputState::new();

        let first = next_generation(&state);
        state.active.store(true, Ordering::SeqCst);
        assert!(active_for_generation(&state, first));

        state.active.store(false, Ordering::SeqCst);
        invalidate_generation(&state);
        assert!(!active_for_generation(&state, first));

        let second = next_generation(&state);
        state.active.store(true, Ordering::SeqCst);
        assert!(!active_for_generation(&state, first));
        assert!(active_for_generation(&state, second));
    }
}

#[cfg(target_os = "macos")]
mod macos_listener {
    //! macOS capture backend using NSEvent **global** monitors.
    //!
    //! Global monitors observe events delivered to *other* applications — i.e.
    //! exactly the "developer is typing in their editor/terminal" case. The
    //! handler block reads only that an event of a given kind fired; it never
    //! inspects the `NSEvent` contents. Monitors must be installed/removed on
    //! the main thread (AppKit requirement), so all objc work hops there via
    //! `run_on_main_thread`, matching the codebase pattern in `setup.rs`.

    use std::sync::Arc;

    use block2::RcBlock;
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};

    use super::{InputListener, ListenerStatus};
    use crate::input::aggregator::InputKind;
    use crate::state::InputState;

    // NSEventMask = 1 << NSEventType. We monitor only key-down and mouse-down,
    // never key-up / mouse-moved (avoids floods and minimizes what we observe).
    const NSEVENT_MASK_KEY_DOWN: u64 = 1 << 10;
    const NSEVENT_MASK_LEFT_MOUSE_DOWN: u64 = 1 << 1;
    const NSEVENT_MASK_RIGHT_MOUSE_DOWN: u64 = 1 << 3;
    const NSEVENT_MASK_OTHER_MOUSE_DOWN: u64 = 1 << 25;

    /// Global NSEvent **key** monitors require the app to be trusted for
    /// **Accessibility** (per Apple's `addGlobalMonitorForEvents(matching:handler:)`
    /// docs) — NOT IOKit "Input Monitoring" (that gates `CGEventTap`, a different
    /// backend). Mouse-down monitors need no permission. Reuse the repo's
    /// existing `AXIsProcessTrusted` check so behavior matches the rest of the app.
    fn keyboard_permission_granted() -> bool {
        crate::platform::macos::check_accessibility_permission()
    }

    /// Install one NSEvent global monitor for `mask`, whose handler bumps the
    /// aggregator counter for `kind`. The retained monitor handle is stored (as
    /// a raw pointer) so `stop` can remove it.
    fn install_monitor(
        app: &tauri::AppHandle,
        state: &Arc<InputState>,
        generation: u64,
        mask: u64,
        kind: InputKind,
    ) {
        let state = Arc::clone(state);
        let _ = app.run_on_main_thread(move || unsafe {
            let Some(cls) = AnyClass::get(c"NSEvent") else {
                return;
            };
            let handler_state = Arc::clone(&state);
            // void(^)(NSEvent*) — we take the event as an opaque pointer and
            // never read it. Only the *kind* (known from `mask`) is recorded, and
            // only while this exact tracking generation is active: this honors the
            // off boundary during the gap between `stop` and the async main-thread
            // `removeMonitor`, and prevents delayed old monitors from recording
            // into a later restart.
            let handler = RcBlock::new(move |_event: *mut AnyObject| {
                if !super::active_for_generation(&handler_state, generation) {
                    return;
                }
                if let Ok(mut g) = handler_state.aggregator.lock() {
                    g.record(kind);
                }
            });
            let monitor: *mut AnyObject = msg_send![
                cls,
                addGlobalMonitorForEventsMatchingMask: mask,
                handler: &*handler
            ];
            // `addGlobalMonitorForEventsMatchingMask:handler:` *copies* the block
            // (it is a stored handler invoked until `removeMonitor`), so AppKit
            // owns its own retained copy. Drop our RcBlock — do NOT forget it —
            // so the block and its captured Arc are freed when the monitor is
            // removed; otherwise every start/stop toggle would leak one block.
            drop(handler);
            if !monitor.is_null() {
                // The returned monitor is autoreleased — retain so it survives
                // past this run-loop turn until `removeMonitor` in `stop`.
                let _: () = msg_send![&*monitor, retain];
                if let Ok(mut mons) = state.monitors.lock() {
                    mons.push(monitor as usize);
                }
            }
        });
    }

    pub(super) struct MacInputListener;

    impl InputListener for MacInputListener {
        fn start(
            &self,
            app: &tauri::AppHandle,
            state: &Arc<InputState>,
            generation: u64,
        ) -> ListenerStatus {
            let keyboard = keyboard_permission_granted();
            if !keyboard {
                // Prompt for Accessibility once; non-fatal if the user declines.
                crate::platform::macos::request_accessibility_permission();
            }
            // Mouse-down needs no permission — always capture it.
            install_monitor(
                app,
                state,
                generation,
                NSEVENT_MASK_LEFT_MOUSE_DOWN
                    | NSEVENT_MASK_RIGHT_MOUSE_DOWN
                    | NSEVENT_MASK_OTHER_MOUSE_DOWN,
                InputKind::Mouse,
            );
            if keyboard {
                install_monitor(
                    app,
                    state,
                    generation,
                    NSEVENT_MASK_KEY_DOWN,
                    InputKind::Keyboard,
                );
            }
            ListenerStatus {
                keyboard,
                mouse: true,
                reason: if keyboard {
                    None
                } else {
                    Some("accessibility-denied".to_string())
                },
            }
        }

        fn stop(&self, app: &tauri::AppHandle, state: &Arc<InputState>) {
            let state = Arc::clone(state);
            let _ = app.run_on_main_thread(move || unsafe {
                let Some(cls) = AnyClass::get(c"NSEvent") else {
                    return;
                };
                if let Ok(mut mons) = state.monitors.lock() {
                    for ptr in mons.drain(..) {
                        let monitor = ptr as *mut AnyObject;
                        if !monitor.is_null() {
                            let _: () = msg_send![cls, removeMonitor: monitor];
                            // Balance the retain we took in `install_monitor`.
                            let _: () = msg_send![&*monitor, release];
                        }
                    }
                }
            });
        }
    }
}
