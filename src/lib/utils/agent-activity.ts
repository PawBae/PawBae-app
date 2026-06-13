// Session-status aggregation (Phase 2 agent emotionalization). Pure logic over the
// 2s-polled session list: collapses per-session hook statuses into the one signal the
// mascot, the activity bubble and the overload aura all share.
//
// Wire statuses come from src-tauri hook event processing: processing | tool_running |
// compacting | waiting | stopped | idle (Cursor/Codex map into the same set). Unknown
// strings count as inactive rather than guessing.

export interface SessionStatusLike {
  status?: string;
}

export interface AgentActivity {
  /** Sessions blocked on the user (permission / confirmation). */
  waiting: number;
  /** Sessions compacting context. */
  compacting: number;
  /** Sessions actively processing or running tools. */
  working: number;
}

/** Parallel busy sessions (any non-idle kind) at or above this count = overload. */
export const OVERLOAD_SESSIONS = 3;

export function aggregateSessions(sessions: readonly SessionStatusLike[]): AgentActivity {
  const out: AgentActivity = { waiting: 0, compacting: 0, working: 0 };
  for (const s of sessions) {
    switch (s.status) {
      case 'waiting':
        out.waiting += 1;
        break;
      case 'compacting':
        out.compacting += 1;
        break;
      case 'processing':
      case 'tool_running':
        out.working += 1;
        break;
      default:
        break; // idle / stopped / unknown — not busy
    }
  }
  return out;
}

export function busyCount(a: AgentActivity): number {
  return a.waiting + a.compacting + a.working;
}

export function isOverloaded(a: AgentActivity): boolean {
  return busyCount(a) >= OVERLOAD_SESSIONS;
}

export type BubbleKind = 'waiting' | 'compacting' | 'working' | null;

/**
 * What the activity bubble should say. Waiting outranks everything (it's the one state
 * that needs the user); compacting outranks working (it explains an unresponsive agent).
 * The component decides persistence: waiting/compacting stay up, working is transient.
 */
export function bubbleKindFor(a: AgentActivity): BubbleKind {
  if (a.waiting > 0) return 'waiting';
  if (a.compacting > 0) return 'compacting';
  if (a.working > 0) return 'working';
  return null;
}

export type MascotSourceState = 'idle' | 'working' | 'compacting' | 'waiting';

/**
 * The mascot's base state from live activity. Same precedence as the bubble — a pet
 * that visibly asks for attention on `waiting` is the whole point (pets map it via
 * stateMap, e.g. yoonie hides, default pets play the waiting row).
 */
export function mascotStateFor(a: AgentActivity, anyHealthActive: boolean): MascotSourceState {
  if (a.waiting > 0) return 'waiting';
  if (a.compacting > 0) return 'compacting';
  // Health-only signal (OpenClaw agents without hook statuses) still reads as working.
  if (a.working > 0 || anyHealthActive) return 'working';
  return 'idle';
}
