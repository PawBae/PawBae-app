//! Pure, platform-agnostic input-event aggregation.
//!
//! This module holds *no* OS handles and does *no* I/O, so it is fully unit
//! testable on every platform (the tests run in CI on both macOS and Windows).
//! The capture layer feeds it `record(kind)`; the flush thread periodically
//! calls `drain(now)` to turn accumulated counts into batched `user-input`
//! events. Time is injected (`at`) rather than read here, keeping the logic
//! deterministic and free of `SystemTime`.
//!
//! Privacy: only a per-kind *count* is ever stored — never key codes,
//! characters, coordinates, window titles, or app identity.

use serde::Serialize;

/// The kind of input event. Serializes to the lowercase string the frontend
/// `user-input` contract expects (`"keyboard"` / `"mouse"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InputKind {
    Keyboard,
    Mouse,
}

/// One batched input fact emitted to the frontend.
///
/// Matches the agreed contract exactly: `{ kind, count, at }`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UserInputEvent {
    pub kind: InputKind,
    pub count: u32,
    pub at: u64,
}

/// Accumulates input counts per kind between flushes.
#[derive(Debug, Default)]
pub struct InputAggregator {
    keyboard: u32,
    mouse: u32,
}

impl InputAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a single input event of `kind`. Saturating so a pathological
    /// burst can never overflow the counter (it is reset every flush anyway).
    ///
    /// Only the macOS capture backend calls this today; the non-macOS lib build
    /// has no caller (a real Windows backend is Phase 2), but the unit tests
    /// exercise it on every platform.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn record(&mut self, kind: InputKind) {
        match kind {
            InputKind::Keyboard => self.keyboard = self.keyboard.saturating_add(1),
            InputKind::Mouse => self.mouse = self.mouse.saturating_add(1),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.keyboard == 0 && self.mouse == 0
    }

    /// Emit one event per non-empty kind, stamped with `at`, and reset the
    /// counters. A burst of N same-kind events collapses to a single event with
    /// `count = N`, so high-frequency input never floods the frontend.
    pub fn drain(&mut self, at: u64) -> Vec<UserInputEvent> {
        let mut out = Vec::new();
        if self.keyboard > 0 {
            out.push(UserInputEvent {
                kind: InputKind::Keyboard,
                count: self.keyboard,
                at,
            });
            self.keyboard = 0;
        }
        if self.mouse > 0 {
            out.push(UserInputEvent {
                kind: InputKind::Mouse,
                count: self.mouse,
                at,
            });
            self.mouse = 0;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_burst_collapses_to_one_event() {
        let mut agg = InputAggregator::new();
        for _ in 0..5 {
            agg.record(InputKind::Keyboard);
        }
        let events = agg.drain(1000);
        assert_eq!(
            events,
            vec![UserInputEvent {
                kind: InputKind::Keyboard,
                count: 5,
                at: 1000
            }]
        );
    }

    #[test]
    fn keyboard_and_mouse_aggregate_separately() {
        let mut agg = InputAggregator::new();
        agg.record(InputKind::Keyboard);
        agg.record(InputKind::Mouse);
        agg.record(InputKind::Mouse);
        let events = agg.drain(7);
        assert_eq!(events.len(), 2);
        assert!(events.contains(&UserInputEvent {
            kind: InputKind::Keyboard,
            count: 1,
            at: 7
        }));
        assert!(events.contains(&UserInputEvent {
            kind: InputKind::Mouse,
            count: 2,
            at: 7
        }));
    }

    #[test]
    fn drain_resets_counters() {
        let mut agg = InputAggregator::new();
        agg.record(InputKind::Keyboard);
        let _ = agg.drain(1);
        assert!(agg.is_empty());
        assert!(agg.drain(2).is_empty());
    }

    #[test]
    fn empty_aggregator_drains_nothing() {
        let mut agg = InputAggregator::new();
        assert!(agg.is_empty());
        assert!(agg.drain(0).is_empty());
    }

    #[test]
    fn is_empty_reflects_state() {
        let mut agg = InputAggregator::new();
        assert!(agg.is_empty());
        agg.record(InputKind::Mouse);
        assert!(!agg.is_empty());
    }

    #[test]
    fn high_frequency_input_is_bounded() {
        let mut agg = InputAggregator::new();
        for _ in 0..10_000 {
            agg.record(InputKind::Keyboard);
        }
        let events = agg.drain(42);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].count, 10_000);
        // Counters reset — no unbounded queue growth.
        assert!(agg.drain(43).is_empty());
    }

    #[test]
    fn at_is_propagated_to_every_event() {
        let mut agg = InputAggregator::new();
        agg.record(InputKind::Keyboard);
        agg.record(InputKind::Mouse);
        let events = agg.drain(999);
        assert!(events.iter().all(|e| e.at == 999));
    }

    #[test]
    fn input_kind_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&InputKind::Keyboard).unwrap(),
            "\"keyboard\""
        );
        assert_eq!(
            serde_json::to_string(&InputKind::Mouse).unwrap(),
            "\"mouse\""
        );
    }

    #[test]
    fn user_input_event_matches_contract() {
        let event = UserInputEvent {
            kind: InputKind::Mouse,
            count: 3,
            at: 12345,
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            serde_json::json!({ "kind": "mouse", "count": 3, "at": 12345 })
        );
    }
}
