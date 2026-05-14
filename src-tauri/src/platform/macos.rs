//! macOS-specific helpers extracted from `lib.rs` during the Phase 2 modular refactor.
//!
//! The whole file is gated to `target_os = "macos"` via an inner attribute so
//! everything below is only compiled on macOS. Items called from the rest of
//! the crate are marked `pub(crate)`.
#![cfg(target_os = "macos")]
