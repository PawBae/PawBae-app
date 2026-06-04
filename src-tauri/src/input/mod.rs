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

use crate::state::InputState;

/// Flush cadence: batch input bursts into one event per kind per tick. 80 ms
/// sits inside the 50–100 ms window agreed with the frontend, so high-frequency
/// typing can never flood Svelte state or the sprite animation loop.
const FLUSH_INTERVAL_MS: u64 = 80;

/// Which input kinds are actually being captured right now, plus an optional
/// human-readable reason when a kind is degraded/off. Returned to the frontend
/// so settings/logs can surface the degraded state (e.g. keyboard off when
/// macOS Input Monitoring is denied).
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
    fn start(&self, app: &tauri::AppHandle, state: &Arc<InputState>) -> ListenerStatus;
    /// Remove OS hooks.
    fn stop(&self, app: &tauri::AppHandle, state: &Arc<InputState>);
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Background thread: drain the aggregator on a fixed cadence and emit one
/// `user-input` event per non-empty kind. Mirrors
/// `pet_core::efficiency_hover_poll`'s active/alive flag pattern so a toggle
/// never spawns duplicate threads.
fn input_flush_loop(app: tauri::AppHandle, state: Arc<InputState>) {
    state.thread_alive.store(true, Ordering::SeqCst);
    while state.active.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(FLUSH_INTERVAL_MS));
        let events = match state.aggregator.lock() {
            Ok(mut agg) => {
                if agg.is_empty() {
                    continue;
                }
                agg.drain(now_ms())
            }
            Err(_) => continue,
        };
        for event in events {
            let _ = app.emit("user-input", event);
        }
    }
    state.thread_alive.store(false, Ordering::SeqCst);
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
    let status = make_listener().start(app, &state);
    if let Ok(mut s) = state.status.lock() {
        *s = status.clone();
    }
    if !state.thread_alive.load(Ordering::SeqCst) {
        let app2 = app.clone();
        let state2 = Arc::clone(&state);
        std::thread::spawn(move || input_flush_loop(app2, state2));
    }
    status
}

/// Stop capturing input and tear down OS hooks.
pub(crate) fn stop_tracking(app: &tauri::AppHandle) -> ListenerStatus {
    let st = app.state::<Arc<InputState>>();
    let state = Arc::clone(&*st);
    state.active.store(false, Ordering::SeqCst);
    make_listener().stop(app, &state);
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
    fn start(&self, _app: &tauri::AppHandle, _state: &Arc<InputState>) -> ListenerStatus {
        // No global-input backend on this platform yet (Phase 2).
        ListenerStatus::disabled("platform-unsupported")
    }
    fn stop(&self, _app: &tauri::AppHandle, _state: &Arc<InputState>) {}
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
    use tauri::Manager;

    use super::{InputListener, ListenerStatus};
    use crate::input::aggregator::InputKind;
    use crate::state::InputState;

    // IOKit HID access controls "Input Monitoring". Global *keyboard* monitoring
    // requires this grant; *mouse* monitoring does not.
    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOHIDCheckAccess(request: u32) -> u32;
        fn IOHIDRequestAccess(request: u32) -> u8;
    }
    const KIOHID_REQUEST_TYPE_LISTEN_EVENT: u32 = 1;
    const KIOHID_ACCESS_TYPE_GRANTED: u32 = 0;

    // NSEventMask = 1 << NSEventType. We monitor only key-down and mouse-down,
    // never key-up / mouse-moved (avoids floods and minimizes what we observe).
    const NSEVENT_MASK_KEY_DOWN: u64 = 1 << 10;
    const NSEVENT_MASK_LEFT_MOUSE_DOWN: u64 = 1 << 1;
    const NSEVENT_MASK_RIGHT_MOUSE_DOWN: u64 = 1 << 3;
    const NSEVENT_MASK_OTHER_MOUSE_DOWN: u64 = 1 << 25;

    fn keyboard_permission_granted() -> bool {
        unsafe { IOHIDCheckAccess(KIOHID_REQUEST_TYPE_LISTEN_EVENT) == KIOHID_ACCESS_TYPE_GRANTED }
    }

    /// Install one NSEvent global monitor for `mask`, whose handler bumps the
    /// aggregator counter for `kind`. The retained monitor handle is stored (as
    /// a raw pointer) so `stop` can remove it.
    fn install_monitor(
        app: &tauri::AppHandle,
        state: &Arc<InputState>,
        mask: u64,
        kind: InputKind,
    ) {
        let state = Arc::clone(state);
        let _ = app.run_on_main_thread(move || unsafe {
            let Some(cls) = AnyClass::get(c"NSEvent") else {
                return;
            };
            let agg = Arc::clone(&state.aggregator);
            // void(^)(NSEvent*) — we take the event as an opaque pointer and
            // never read it. Only the *kind* (known from `mask`) is recorded.
            let handler = RcBlock::new(move |_event: *mut AnyObject| {
                if let Ok(mut g) = agg.lock() {
                    g.record(kind);
                }
            });
            let monitor: *mut AnyObject = msg_send![
                cls,
                addGlobalMonitorForEventsMatchingMask: mask,
                handler: &*handler
            ];
            // AppKit copies the block; forgetting our RcBlock simply guarantees
            // liveness (matches the speech.rs handler pattern).
            std::mem::forget(handler);
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
        fn start(&self, app: &tauri::AppHandle, state: &Arc<InputState>) -> ListenerStatus {
            let keyboard = keyboard_permission_granted();
            if !keyboard {
                // Prompt for Input Monitoring once; non-fatal if the user denies.
                unsafe {
                    let _ = IOHIDRequestAccess(KIOHID_REQUEST_TYPE_LISTEN_EVENT);
                }
            }
            // Mouse-down needs no permission — always capture it.
            install_monitor(
                app,
                state,
                NSEVENT_MASK_LEFT_MOUSE_DOWN
                    | NSEVENT_MASK_RIGHT_MOUSE_DOWN
                    | NSEVENT_MASK_OTHER_MOUSE_DOWN,
                InputKind::Mouse,
            );
            if keyboard {
                install_monitor(app, state, NSEVENT_MASK_KEY_DOWN, InputKind::Keyboard);
            }
            ListenerStatus {
                keyboard,
                mouse: true,
                reason: if keyboard {
                    None
                } else {
                    Some("input-monitoring-denied".to_string())
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
