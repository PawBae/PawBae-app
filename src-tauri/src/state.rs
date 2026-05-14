//! Module-scope statics, managed-state types, and shared global helpers.
//!
//! Extracted from `lib.rs` during the Phase 1 modular refactor (pure relocation).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[cfg(target_os = "windows")]
pub(crate) static FULLSCREEN_HIDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the efficiency-mode notch hover tracking thread should be running.
pub(crate) static EFFICIENCY_HOVER_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Whether the hover poll thread is actually alive (set true on entry, false on exit).
pub(crate) static EFFICIENCY_HOVER_THREAD_ALIVE: AtomicBool = AtomicBool::new(false);
/// Whether the mini panel is currently expanded (used by the hover poll to
/// decide which detection region to check — collapsed notch area vs expanded
/// panel area).
pub(crate) static EFFICIENCY_EXPANDED: AtomicBool = AtomicBool::new(false);
/// Cached screen geometry for the notch hover poll thread so it doesn't need
/// to access NSWindow from a background thread.
/// (screen_x, screen_y, screen_width, screen_height, notch_offset)
pub(crate) static NOTCH_SCREEN_INFO: Mutex<Option<(f64, f64, f64, f64, f64)>> = Mutex::new(None);
/// Cached mini window frame (x, y, w, h) in macOS screen coordinates
/// (bottom-left origin).  Updated by `set_mini_expanded` and
/// `resize_mini_height` so the hover poll can use the real frame size
/// instead of hard-coded constants.
pub(crate) static MINI_WINDOW_FRAME: Mutex<Option<(f64, f64, f64, f64)>> = Mutex::new(None);
/// Temporary frame snapshot used by pet-context menu expansion. We store the
/// original collapsed frame before expanding, then restore exactly to avoid
/// mascot "teleport" after right-click close.
pub(crate) static PET_MENU_RESTORE_FRAME: Mutex<Option<(f64, f64, f64, f64)>> = Mutex::new(None);
/// Generation counter for pet-context alpha restore (legacy resize path).
#[cfg(target_os = "macos")]
pub(crate) static PET_ALPHA_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Whether the pet-mode click-through poll thread should be running.
pub(crate) static PET_PASSTHROUGH_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Whether the pet-mode click-through poll thread is alive.
pub(crate) static PET_PASSTHROUGH_THREAD_ALIVE: AtomicBool = AtomicBool::new(false);
/// Whether the pet-mode context menu is currently open. When true the poll
/// thread disables ignoresMouseEvents so the entire expanded window accepts
/// clicks (for the menu buttons). When false, only the mascot area accepts
/// clicks and the rest is pass-through.
pub(crate) static PET_CONTEXT_MENU_OPEN: AtomicBool = AtomicBool::new(false);
/// Whether a pomodoro timer is currently active. When true the poll thread
/// keeps the entire window interactive so the bottom-anchored Pomodoro
/// stop button receives clicks (it sits in the centered hitbox's bottom
/// inset region and would otherwise pass through to whatever is behind).
pub(crate) static PET_POMODORO_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Coalesces drag-apply tasks so we never queue more than one
/// setFrameOrigin: call on the main thread at a time. The poll thread
/// records the anchor (cursor-to-origin offset at drag start) once; each
/// scheduled main-thread task simply reads the live cursor position and
/// snaps the window origin to (cursor - anchor). This is the same pattern
/// macOS uses for native window dragging and avoids the lag introduced by
/// accumulating deltas across pre-empted frames.
pub(crate) static DRAG_TASK_PENDING: AtomicBool = AtomicBool::new(false);
pub(crate) static DRAG_ANCHOR: std::sync::OnceLock<Mutex<Option<(f64, f64)>>> = std::sync::OnceLock::new();
pub(crate) fn drag_anchor() -> &'static Mutex<Option<(f64, f64)>> {
    DRAG_ANCHOR.get_or_init(|| Mutex::new(None))
}

// Stroll-mode toggles (Phase 2 pet physics).
//
// `STROLL_MODE_ENABLED` mirrors the user-visible global toggle in the
// system-tray menu. The frontend persists the setting in
// `settings.json::stroll_mode_enabled` and pushes it back to Rust via
// `set_stroll_mode` on startup so the tray check-state stays in sync
// across launches. Default is `true` so users who pick a physics-
// enabled pet (e.g. shimeji-bola) see the effect immediately.
pub(crate) static STROLL_MODE_ENABLED: AtomicBool = AtomicBool::new(true);
// `THROW_TRACKING_ENABLED` gates the per-tick velocity sample collection
// inside the macOS drag loop. The frontend toggles it on only when
// stroll is active AND the selected pet declares physics, so we don't
// spend cycles on the VecDeque push for legacy pets.
pub(crate) static THROW_TRACKING_ENABLED: AtomicBool = AtomicBool::new(false);

/// Per-host SSH backoff state.
pub(crate) struct SshBackoffState {
    pub(crate) fail_count: u32,
    pub(crate) fail_epoch: u64,
}

pub(crate) static SSH_BACKOFF: std::sync::OnceLock<Mutex<HashMap<String, SshBackoffState>>> = std::sync::OnceLock::new();

pub(crate) fn ssh_backoff_map() -> &'static Mutex<HashMap<String, SshBackoffState>> {
    SSH_BACKOFF.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Stores which SSH key was accepted for each host (user@host → key path).
/// Populated by ensure_ssh_master via `ssh -v` output parsing.
pub(crate) static SSH_KEY_USED: std::sync::OnceLock<Mutex<HashMap<String, String>>> = std::sync::OnceLock::new();

pub(crate) fn ssh_key_map() -> &'static Mutex<HashMap<String, String>> {
    SSH_KEY_USED.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Managed state: tracks the PID of the currently running `openclaw agent` subprocess.
/// Used by interrupt_agent to SIGINT the active turn.
pub(crate) struct ActiveAgentPid {
    pub(crate) pid: Mutex<Option<u32>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub label: Option<String>,
    pub status: String,
    pub model: Option<String>,
    pub channel: Option<String>,
}

/// Sprite-content padding fractions for the current pet. These let the
/// physics safety-net clamp in `move_mini_by` agree with the frontend
/// edge-detection in `edgeDetect.ts`: both must subtract the same
/// transparent-gap fraction from the visibleFrame so the *visible*
/// character ends up flush with each screen edge (bottom in particular
/// = the Dock top).
///
/// Defaults match the legacy hardcoded constants in `edgeDetect.ts`
/// (top 0.40, sides 0.45, bottom 0.30) so this Mutex is safe to read
/// before the frontend has pushed a measurement.
///
/// The frontend pushes the bottom fraction at physics-enable time, after
/// alpha-scanning the pet's idle frame to find the actual transparent
/// gap below the visible foot. Top / sides remain at the legacy
/// defaults because the climb poses determine those edges, not the
/// idle pose, and we currently only scan idle.
#[derive(Clone, Copy)]
pub(crate) struct SpritePadFracs {
    pub(crate) top: f64,
    pub(crate) right: f64,
    pub(crate) bottom: f64,
    pub(crate) left: f64,
    /// Absolute CSS-pixel overrides per edge. When `Some`,
    /// `move_mini_by` uses these directly instead of multiplying the
    /// fraction by the window dimension. Set by the frontend after
    /// alpha-scanning the relevant animation rows and DOM-measuring
    /// the rendered sprite's distance from each window edge — the
    /// fraction approach is wrong whenever the sprite div is
    /// smaller than the window (centered, with empty pixels around
    /// it), because cell-fraction × window-size doesn't account for
    /// the centering offset.
    pub(crate) top_px: Option<f64>,
    pub(crate) right_px: Option<f64>,
    pub(crate) bottom_px: Option<f64>,
    pub(crate) left_px: Option<f64>,
}

pub(crate) static SPRITE_PAD: std::sync::Mutex<SpritePadFracs> = std::sync::Mutex::new(SpritePadFracs {
    top: 0.40,
    right: 0.45,
    bottom: 0.30,
    left: 0.45,
    top_px: None,
    right_px: None,
    bottom_px: None,
    left_px: None,
});

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClaudeSession {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub cwd: String,
    pub status: String, // processing, waiting, idle, tool_running, compacting, stopped
    pub tool: Option<String>,
    #[serde(rename = "toolInput")]
    pub tool_input: Option<String>,
    #[serde(rename = "userPrompt")]
    pub user_prompt: Option<String>,
    pub interactive: bool,
    #[serde(rename = "updatedAt")]
    pub updated_at: u64,
    /// Derived from Claude's own status field: true when status != "waiting_for_input"
    #[serde(rename = "isProcessing")]
    pub is_processing: bool,
    /// PID of the Claude Code process that owns this session.
    /// Used to detect Ctrl+C exits: if the PID is dead and status is "waiting",
    /// the session is stale and should be cleared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Number of sub-agents (Agent tool) still running in the background.
    /// Incremented on PreToolUse(Agent), decremented on SubagentStop.
    /// Sound only plays on Stop when this reaches 0 (all agents done).
    #[serde(rename = "pendingAgents")]
    pub pending_agents: u32,
    /// Raw permission_suggestions JSON from the PermissionRequest hook event.
    #[serde(rename = "permissionSuggestions", skip_serializing_if = "Option::is_none")]
    pub permission_suggestions: Option<serde_json::Value>,
    /// AI's last response text (truncated), forwarded from the Stop hook event.
    /// Shown in the efficiency-mode completion reminder popup.
    #[serde(rename = "lastResponse", skip_serializing_if = "Option::is_none")]
    pub last_response: Option<String>,
    /// True when the last Stop-like event represented a failed/interrupted run.
    #[serde(rename = "lastFailure")]
    pub last_failure: bool,
    /// Whether this session's terminal tab is currently the active/focused tab.
    /// Set dynamically in `get_claude_sessions` — not persisted.
    #[serde(rename = "isActiveTab")]
    pub is_active_tab: bool,
    /// Source of this session: "cc" (Claude Code), "codex" (Codex), or "cursor" (Cursor IDE).
    pub source: String,
    /// Ghostty terminal `id` captured when the session is first seen.
    /// Used by `jump_to_claude_terminal` to select the exact tab instead
    /// of relying on CWD/title matching which is ambiguous.
    #[serde(skip)]
    pub terminal_id: Option<String>,
    /// Host terminal app name (e.g. "Ghostty", "Cursor", "iTerm2").
    /// Captured once at session creation via process chain walk.
    #[serde(skip)]
    pub host_terminal: Option<String>,
    /// Bound Cursor extension port for this session.
    /// Unlike `pid`, this is stable for the lifetime of a Cursor window.
    /// We resolve it from the session cwd/workspace and reuse it on click.
    #[serde(skip)]
    pub cursor_port: Option<u16>,
    /// Workspace root matched to the bound Cursor window.
    /// Stored so we can revalidate the binding when the session cwd changes.
    #[serde(skip)]
    pub cursor_workspace_root: Option<String>,
    /// Human-readable workspace name reported by the Cursor extension.
    /// Used to raise the correct Cursor window on macOS before focusing content.
    #[serde(skip)]
    pub cursor_workspace_name: Option<String>,
    /// Native window handle (hex string) from the Cursor extension.
    /// Uniquely identifies a Cursor window even when multiple windows
    /// share the same workspace root.
    #[serde(skip)]
    pub cursor_native_handle: Option<String>,
}

pub(crate) type PendingPermissions = Arc<Mutex<HashMap<String, std::sync::mpsc::Sender<String>>>>;

pub(crate) struct ClaudeState {
    pub(crate) sessions: Arc<Mutex<HashMap<String, ClaudeSession>>>,
    pub(crate) pending_permissions: PendingPermissions,
}

/// Global registry of active file watchers, keyed by session ID
pub(crate) static SESSION_WATCHERS: std::sync::LazyLock<Mutex<HashMap<String, notify::RecommendedWatcher>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
