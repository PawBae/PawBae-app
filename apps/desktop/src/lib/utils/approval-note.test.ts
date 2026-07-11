import { describe, expect, it } from 'vitest';
import {
  AFFECTION_APPROVAL,
  APPROVAL_DAILY_LIMIT,
  APPROVAL_FAST_RESPONSE_MS,
  approvalAwardFor,
  initialApprovalState,
  oldestPending,
  stepApprovalNotes,
} from './approval-note';

const T0 = 1_700_000_000_000;

describe('stepApprovalNotes', () => {
  it('tracks a rising edge and keeps the first-seen timestamp across polls', () => {
    const s = initialApprovalState();
    expect(stepApprovalNotes(s, ['a'], T0).responses).toEqual([]);
    // Same session still waiting two polls later — timestamp must not move.
    stepApprovalNotes(s, ['a'], T0 + 2_000);
    const { responses } = stepApprovalNotes(s, [], T0 + 5_000);
    expect(responses).toEqual([{ sessionId: 'a', waitedMs: 5_000 }]);
  });

  it('reports each cleared session once and forgets it', () => {
    const s = initialApprovalState();
    stepApprovalNotes(s, ['a'], T0);
    expect(stepApprovalNotes(s, [], T0 + 1_000).responses).toHaveLength(1);
    expect(stepApprovalNotes(s, [], T0 + 2_000).responses).toEqual([]);
  });

  it('handles several sessions independently', () => {
    const s = initialApprovalState();
    stepApprovalNotes(s, ['a'], T0);
    stepApprovalNotes(s, ['a', 'b'], T0 + 10_000);
    const { responses } = stepApprovalNotes(s, ['b'], T0 + 30_000);
    expect(responses).toEqual([{ sessionId: 'a', waitedMs: 30_000 }]);
    expect(oldestPending(s)).toBe('b');
  });

  it('a re-wait after clearing starts a fresh timestamp', () => {
    const s = initialApprovalState();
    stepApprovalNotes(s, ['a'], T0);
    stepApprovalNotes(s, [], T0 + 1_000);
    stepApprovalNotes(s, ['a'], T0 + 60_000);
    const { responses } = stepApprovalNotes(s, [], T0 + 61_000);
    expect(responses).toEqual([{ sessionId: 'a', waitedMs: 1_000 }]);
  });

  it('clamps a clock that ran backwards to zero waited time', () => {
    const s = initialApprovalState();
    stepApprovalNotes(s, ['a'], T0);
    const { responses } = stepApprovalNotes(s, [], T0 - 5_000);
    expect(responses).toEqual([{ sessionId: 'a', waitedMs: 0 }]);
  });
});

describe('oldestPending', () => {
  it('returns the longest-waiting session, null when none', () => {
    const s = initialApprovalState();
    expect(oldestPending(s)).toBeNull();
    stepApprovalNotes(s, ['late', 'early'], T0); // same poll: array order breaks the tie
    expect(oldestPending(s)).toBe('late');
  });
});

describe('approvalAwardFor', () => {
  it('awards within the fast window, nothing after it', () => {
    expect(approvalAwardFor(APPROVAL_FAST_RESPONSE_MS, 0)).toBe(AFFECTION_APPROVAL);
    expect(approvalAwardFor(APPROVAL_FAST_RESPONSE_MS + 1, 0)).toBe(0);
  });

  it('stops at the daily cap', () => {
    expect(approvalAwardFor(1_000, APPROVAL_DAILY_LIMIT - 1)).toBe(AFFECTION_APPROVAL);
    expect(approvalAwardFor(1_000, APPROVAL_DAILY_LIMIT)).toBe(0);
  });

  it('rejects corrupt input', () => {
    expect(approvalAwardFor(Number.NaN, 0)).toBe(0);
    expect(approvalAwardFor(-1, 0)).toBe(0);
  });
});
