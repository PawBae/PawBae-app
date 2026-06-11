//! Module-scope statics, managed-state types, and shared global helpers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};

use crate::input::aggregator::InputAggregator;
use crate::input::ListenerStatus;

/// Lock a `Mutex`, recovering from poisoning instead of panicking.
///
/// Poisoning only records that some thread panicked while holding the guard —
/// the data itself is still there. Every mutex in this app protects plain
/// state (session maps, counters, caches) that a panicked writer leaves
/// structurally valid, so inheriting the value beats propagating the panic
/// into every hook event and watcher thread that follows: one panic must not
/// leave the pet permanently stuck in a stale state.
pub(crate) fn lock_or_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| {
        log::warn!("recovering a poisoned mutex (a thread panicked while holding it)");
        poisoned.into_inner()
    })
}

pub(crate) struct SshBackoffState {
    pub(crate) fail_count: u32,
    pub(crate) fail_epoch: u64,
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
    #[serde(
        rename = "permissionSuggestions",
        skip_serializing_if = "Option::is_none"
    )]
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

// ── Managed state structs ─────────────────────────────────────────────
// Registered via `.manage(Arc::new(…))` in lib.rs. Threads clone the Arc;
// commands extract via `app.state::<Arc<T>>()`.

pub(crate) struct WindowState {
    pub(crate) hover_active: AtomicBool,
    pub(crate) hover_thread_alive: AtomicBool,
    pub(crate) expanded: AtomicBool,
    #[allow(clippy::type_complexity)]
    pub(crate) notch_screen_info: Mutex<Option<(f64, f64, f64, f64, f64)>>,
    pub(crate) mini_frame: Mutex<Option<(f64, f64, f64, f64)>>,
    #[cfg(target_os = "windows")]
    pub(crate) fullscreen_hiding: AtomicBool,
}

impl WindowState {
    pub(crate) fn new() -> Self {
        Self {
            hover_active: AtomicBool::new(false),
            hover_thread_alive: AtomicBool::new(false),
            expanded: AtomicBool::new(false),
            notch_screen_info: Mutex::new(None),
            mini_frame: Mutex::new(None),
            #[cfg(target_os = "windows")]
            fullscreen_hiding: AtomicBool::new(false),
        }
    }
}

pub(crate) struct PetState {
    pub(crate) passthrough_active: AtomicBool,
    pub(crate) passthrough_thread_alive: AtomicBool,
    pub(crate) context_menu_open: AtomicBool,
    pub(crate) pomodoro_active: AtomicBool,
    pub(crate) menu_restore_frame: Mutex<Option<(f64, f64, f64, f64)>>,
    pub(crate) stroll_enabled: AtomicBool,
    pub(crate) throw_tracking: AtomicBool,
    pub(crate) sprite_pad: Mutex<SpritePadFracs>,
    pub(crate) drag_anchor: Mutex<Option<(f64, f64)>>,
    #[cfg(target_os = "macos")]
    pub(crate) drag_task_pending: AtomicBool,
    #[cfg(target_os = "macos")]
    pub(crate) alpha_gen: AtomicU64,
}

impl PetState {
    pub(crate) fn new() -> Self {
        Self {
            passthrough_active: AtomicBool::new(false),
            passthrough_thread_alive: AtomicBool::new(false),
            context_menu_open: AtomicBool::new(false),
            pomodoro_active: AtomicBool::new(false),
            menu_restore_frame: Mutex::new(None),
            stroll_enabled: AtomicBool::new(true),
            throw_tracking: AtomicBool::new(false),
            sprite_pad: Mutex::new(SpritePadFracs {
                top: 0.40,
                right: 0.45,
                bottom: 0.30,
                left: 0.45,
                top_px: None,
                right_px: None,
                bottom_px: None,
                left_px: None,
            }),
            drag_anchor: Mutex::new(None),
            #[cfg(target_os = "macos")]
            drag_task_pending: AtomicBool::new(false),
            #[cfg(target_os = "macos")]
            alpha_gen: AtomicU64::new(0),
        }
    }
}

pub(crate) struct SshState {
    pub(crate) backoff: Mutex<HashMap<String, SshBackoffState>>,
    pub(crate) key_used: Mutex<HashMap<String, String>>,
}

impl SshState {
    pub(crate) fn new() -> Self {
        Self {
            backoff: Mutex::new(HashMap::new()),
            key_used: Mutex::new(HashMap::new()),
        }
    }
}

/// Managed state for global input-event capture (Phase 1‑A).
///
/// `active` controls the current user-facing toggle. `generation` advances on
/// every start/stop boundary so stale flush threads and delayed platform
/// monitors cannot emit or record into a later tracking session. `aggregator` is
/// shared with the platform capture layer (the macOS NSEvent handler) and
/// drained by the flush thread. `monitors` holds retained NSEvent monitor handles
/// as raw pointers (Send-safe `usize`) so the macOS listener can remove them on
/// stop — macOS-only.
pub(crate) struct InputState {
    pub(crate) active: AtomicBool,
    pub(crate) generation: AtomicU64,
    pub(crate) aggregator: Arc<Mutex<InputAggregator>>,
    pub(crate) status: Mutex<ListenerStatus>,
    /// Serializes the flush thread's emit with `stop_tracking` so that once stop
    /// returns, no further `user-input` event can fire — a hard privacy boundary.
    /// The flush thread drains, then takes this gate and re-checks `active`
    /// before emitting; stop sets `active=false` then takes the same gate (which
    /// waits for any in-flight emit) before clearing pending counts and returning.
    pub(crate) emit_gate: Mutex<()>,
    #[cfg(target_os = "macos")]
    pub(crate) monitors: Mutex<Vec<usize>>,
}

impl InputState {
    pub(crate) fn new() -> Self {
        Self {
            active: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            aggregator: Arc::new(Mutex::new(InputAggregator::new())),
            status: Mutex::new(ListenerStatus::default()),
            emit_gate: Mutex::new(()),
            #[cfg(target_os = "macos")]
            monitors: Mutex::new(Vec::new()),
        }
    }
}

/// Global registry of active file watchers, keyed by session ID
pub(crate) static SESSION_WATCHERS: std::sync::LazyLock<
    Mutex<HashMap<String, notify::RecommendedWatcher>>,
> = std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
