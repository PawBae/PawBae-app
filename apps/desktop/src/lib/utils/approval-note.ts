// Approval note machine (Phase 1 叼来审批单). Pure logic, zero Svelte/Tauri imports —
// mirrors the rewards.ts precedent. Tracks which sessions are blocked on the user and
// how long each has waited; a wait answered inside the fast window earns affection.
//
// Red lines (approved strategy): never punish — a slow or absent response does
// nothing; reward the outcome — clearing the wait counts however the user did it.
// See docs/superpowers/specs/2026-07-08-approval-note-design.md.

/** A wait answered within this window counts as a fast response. */
export const APPROVAL_FAST_RESPONSE_MS = 120_000;
/** Affection per fast response — headpat scale. */
export const AFFECTION_APPROVAL = 2;
/** Fast responses that can earn affection per day (ephemeral counter, like headpat's). */
export const APPROVAL_DAILY_LIMIT = 10;

export interface ApprovalNoteState {
  /** sessionId → epoch ms first seen waiting. Map insertion order = age order. */
  pending: Map<string, number>;
}

export interface ApprovalResponse {
  sessionId: string;
  waitedMs: number;
}

export function initialApprovalState(): ApprovalNoteState {
  return { pending: new Map() };
}

/**
 * Reconcile the pending set against the currently-waiting session ids. New waits are
 * timestamped with `now`; sessions no longer waiting are returned once with how long
 * they waited (clock regressions clamp to 0 — never a negative wait). The caller owns
 * the clock so the machine stays deterministic under test.
 */
export function stepApprovalNotes(
  s: ApprovalNoteState,
  waitingIds: readonly string[],
  now: number,
): { responses: ApprovalResponse[] } {
  if (!Number.isFinite(now)) return { responses: [] };
  const waiting = new Set(waitingIds);
  const responses: ApprovalResponse[] = [];
  for (const [sessionId, since] of s.pending) {
    if (!waiting.has(sessionId)) {
      responses.push({ sessionId, waitedMs: Math.max(0, now - since) });
      s.pending.delete(sessionId);
    }
  }
  for (const id of waitingIds) {
    if (!s.pending.has(id)) s.pending.set(id, now);
  }
  return { responses };
}

/** The longest-waiting session id (deterministic click target), or null. */
export function oldestPending(s: ApprovalNoteState): string | null {
  for (const sessionId of s.pending.keys()) return sessionId;
  return null;
}

/**
 * Affection a cleared wait earns: the fast window and the daily cap are the only
 * gates. The daily counter lives with the caller (petData, ephemeral like headpat's)
 * — this is just the pure decision.
 */
export function approvalAwardFor(waitedMs: number, todayCount: number): number {
  if (!Number.isFinite(waitedMs) || waitedMs < 0) return 0;
  if (waitedMs > APPROVAL_FAST_RESPONSE_MS) return 0;
  if (todayCount >= APPROVAL_DAILY_LIMIT) return 0;
  return AFFECTION_APPROVAL;
}
