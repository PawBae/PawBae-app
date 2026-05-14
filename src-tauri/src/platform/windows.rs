//! Windows-specific helpers extracted from `lib.rs` during the Phase 2 modular refactor.
//!
//! The whole file is gated to `target_os = "windows"` via an inner attribute.
#![cfg(target_os = "windows")]
