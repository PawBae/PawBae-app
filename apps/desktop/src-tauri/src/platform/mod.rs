//! Platform-specific helpers (macOS / Windows / common).

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

pub mod common;
