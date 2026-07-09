// Adventure trip machine (Phase 1 Agent 冒险). Pure logic, zero Svelte/Tauri imports —
// mirrors the approval-note machine. Tracks how long each session has been busy;
// a session that completes after a long-enough run earns a souvenir drop.
//
// Eligibility is deliberately decoupled from the "pet is away" visual: consuming a
// trip on completion works whether or not the pet was on screen at that moment, so a
// visual interruption (approval note, meal beat, hover) can never lose a souvenir.
//
// Red lines: never punish — a killed or ESC'd session simply never fires the
// completion event, so its trip evaporates silently. `waiting` (permission) keeps
// the trip alive: waiting on the user is part of the same task run, and when in
// doubt we err on the generous side.
// See docs/superpowers/specs/2026-07-08-agent-adventure-design.md.

/** A session busy at least this long counts as an adventure-length trip. */
export const ADVENTURE_MIN_MS = 180_000;

export interface AdventureState {
  /** sessionId → epoch ms first seen busy. Cleared on consume or session death. */
  pending: Map<string, number>;
}

export function initialAdventureState(): AdventureState {
  return { pending: new Map() };
}

/**
 * Reconcile the trip timestamps against the current poll. `busyIds` are sessions in
 * an active status (processing/tool_running/compacting); `aliveIds` are sessions in
 * any NON-TERMINAL status (busy or waiting) — not merely present, because the poll
 * keeps killed/ESC'd rows around as stopped/idle, and a stale timestamp surviving
 * there would be inherited by a later run of the same sessionId. New busy sessions
 * are timestamped with `now`; sessions no longer alive are dropped (silently, never
 * a drop); sessions merely waiting on the user keep their timestamp. The caller
 * owns the clock so the machine stays deterministic under test.
 *
 * Returns `away`: whether some CURRENTLY busy session has been on a trip long
 * enough — the display layer's only input from this machine.
 */
export function stepAdventure(
  s: AdventureState,
  busyIds: readonly string[],
  aliveIds: readonly string[],
  now: number,
): { away: boolean } {
  if (!Number.isFinite(now)) return { away: false };
  const alive = new Set(aliveIds);
  for (const sessionId of s.pending.keys()) {
    if (!alive.has(sessionId)) s.pending.delete(sessionId);
  }
  let away = false;
  for (const id of busyIds) {
    const since = s.pending.get(id);
    if (since === undefined) s.pending.set(id, now);
    else if (now - since >= ADVENTURE_MIN_MS) away = true;
  }
  return { away };
}

/**
 * Consume the trip a genuine completion ends: returns how long the session had been
 * out (clock regressions clamp to 0) and forgets it, or null if no trip was ever
 * recorded (short-lived session the poll never caught, or a pre-existing one). The
 * caller compares against ADVENTURE_MIN_MS to decide the drop.
 */
export function consumeTrip(s: AdventureState, sessionId: string, now: number): number | null {
  const since = s.pending.get(sessionId);
  if (since === undefined) return null;
  s.pending.delete(sessionId);
  if (!Number.isFinite(now)) return 0;
  return Math.max(0, now - since);
}
