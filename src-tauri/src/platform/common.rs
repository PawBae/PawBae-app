//! Cross-platform helpers shared between targets.

use serde::Serialize;

/// Rect of the frontmost on-screen app window in Cocoa bottom-left
/// coords (same frame as `get_mini_origin`), used by the pet physics
/// loop to treat that window as a second world (Shimeji-style "active
/// IE"). The pet can climb its sides, sit on its top edge, hang from
/// its bottom edge, and ride along when the user drags it.
#[derive(Serialize, Clone, Debug)]
pub(crate) struct AppWindowInfo {
    /// Stable CGWindowID for the window's lifetime — the physics loop
    /// uses this to detect "the window I was sitting on disappeared".
    pub(crate) window_id: u32,
    /// Process owning the window ("Finder", "Safari", "Cursor", …).
    pub(crate) owner_name: String,
    /// Owning process pid — already filtered against our own pid in
    /// Rust so the frontend can ignore this and just trust the rect.
    pub(crate) owner_pid: i32,
    /// Bottom-left x in main-screen Cocoa coords.
    pub(crate) x: f64,
    /// Bottom-left y in main-screen Cocoa coords.
    pub(crate) y: f64,
    /// Logical points.
    pub(crate) width: f64,
    /// Logical points.
    pub(crate) height: f64,
}
