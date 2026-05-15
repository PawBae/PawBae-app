//! Tauri command modules.
//!
//! Phase 3 of the lib.rs modular refactor. Commands are grouped by domain;
//! each submodule's items are re-exported via glob so the existing
//! `generate_handler!` macro in lib.rs continues to reference them by
//! unqualified name.

pub mod agent;
pub mod codex_pet;
pub mod media;
pub mod misc;
pub mod ssh;
pub mod update;

#[allow(unused_imports)]
pub use agent::*;
#[allow(unused_imports)]
pub use codex_pet::*;
#[allow(unused_imports)]
pub use media::*;
#[allow(unused_imports)]
pub use misc::*;
#[allow(unused_imports)]
pub use ssh::*;
#[allow(unused_imports)]
pub use update::*;
