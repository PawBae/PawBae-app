//! Platform-specific helpers (macOS / Windows / common).
//!
//! Each submodule is gated to its target OS via an inner `#![cfg]` attribute,
//! so the items inside are only compiled on the matching platform.
//!
//! Consumers in the rest of the crate import the contents directly via
//! `use crate::platform::macos::*;` (cfg-gated) etc. — we do NOT re-export
//! `pub use windows::*` here because that would lift the bare `windows`
//! name into `crate::platform::*`, clashing with the external `windows`
//! crate from `lib.rs`.

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

pub mod common;
