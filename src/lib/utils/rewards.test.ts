import { describe, expect, it } from 'vitest';
import type { CoinAward, CoinSource, CoinSourceTotals, UserInputEvent } from '../types';
import {
  AGENT_STOP_COINS,
  AGENT_STOP_COOLDOWN_MS,
  applyAward,
  applyUserInput,
  awardAgentStop,
  FOCUS_BLOCK_COINS,
  FOCUS_BLOCK_MS,
  FOCUS_GAP_RESET_MS,
  INPUT_MILESTONE_COINS,
  INPUT_MILESTONE_STEP,
  initialRewardState,
  LEDGER_RECENT_CAP,
  type MutableRewardState,
  restoreRewardState,
  snapshotRewardState,
  trackFocusInput,
  trackInputCount,
} from './rewards';

const T0 = 1_000_000;
const MINUTE = 60_000;

function inputEv(
  count: number,
  at: number,
  kind: 'keyboard' | 'mouse' = 'keyboard',
): UserInputEvent {
  return { kind, count, at };
}

function stopEv(sessionId: string, at: number, waiting = false) {
  return { sessionId, waiting, at };
}

/** Feed a series of focus events (one per timestamp), threading the balance through. */
function runFocus(
  s: MutableRewardState,
  coins: number,
  ats: number[],
  pomodoroActive = false,
): { coins: number; awards: CoinAward[] } {
  let c = coins;
  const awards: CoinAward[] = [];
  for (const at of ats) {
    const r = trackFocusInput(s, c, { at }, { pomodoroActive });
    c = r.coinsAfter;
    awards.push(...r.awards);
  }
  return { coins: c, awards };
}

/** Timestamps every minute from `from` to `to` inclusive. */
function everyMinute(from: number, to: number): number[] {
  const ats: number[] = [];
  for (let at = from; at <= to; at += MINUTE) ats.push(at);
  return ats;
}

describe('initialRewardState', () => {
  it('starts with empty ledger, zeroed totals for every source, and no runtime bookkeeping', () => {
    const s = initialRewardState();
    expect(s.recent).toEqual([]);
    expect(s.lifetimeInputCount).toBe(0);
    expect(s.lastAwardedMilestone).toBe(0);
    expect(Object.keys(s.sessionCooldowns)).toHaveLength(0);
    expect(s.focus).toEqual({ streakStartAt: null, lastInputAt: null });
    const sources: CoinSource[] = [
      'agent_stop',
      'focus_minutes',
      'input_milestone',
      'pomodoro',
      'daily_gift',
      'feed',
    ];
    for (const src of sources) {
      expect(s.totals[src]).toEqual({ earned: 0, spent: 0, count: 0 });
    }
  });
});

describe('applyAward', () => {
  it('increases coins and appends a ledger entry carrying source, amount, and at', () => {
    const s = initialRewardState();
    const r = applyAward(s, 0, { source: 'daily_gift', amount: 50, at: T0 });
    expect(r.coinsAfter).toBe(50);
    expect(r.awards).toHaveLength(1);
    expect(s.recent).toHaveLength(1);
    expect(s.recent[0]).toEqual({ source: 'daily_gift', amount: 50, at: T0 });
  });

  it('attaches reason and sessionId to the entry only when provided', () => {
    const s = initialRewardState();
    applyAward(s, 0, { source: 'agent_stop', amount: 20, at: T0, sessionId: 's1' });
    applyAward(s, 20, { source: 'focus_minutes', amount: 5, at: T0 + 1, reason: 'focus 10min' });
    applyAward(s, 25, { source: 'daily_gift', amount: 50, at: T0 + 2 });
    expect(s.recent[0].sessionId).toBe('s1');
    expect(s.recent[0].reason).toBeUndefined();
    expect(s.recent[1].reason).toBe('focus 10min');
    expect(s.recent[1].sessionId).toBeUndefined();
    expect('reason' in s.recent[2]).toBe(false);
    expect('sessionId' in s.recent[2]).toBe(false);
  });

  it('applies a negative spend and ledgers the negative amount', () => {
    const s = initialRewardState();
    const r = applyAward(s, 10, { source: 'feed', amount: -5, at: T0 });
    expect(r.coinsAfter).toBe(5);
    expect(s.recent[0].amount).toBe(-5);
    expect(s.totals.feed).toEqual({ earned: 0, spent: 5, count: 1 });
  });

  it('clamps at zero and records the EFFECTIVE delta (coins 3, feed -5 -> -3)', () => {
    const s = initialRewardState();
    const r = applyAward(s, 3, { source: 'feed', amount: -5, at: T0 });
    expect(r.coinsAfter).toBe(0);
    expect(s.recent[0].amount).toBe(-3);
    expect(s.totals.feed.spent).toBe(3);
  });

  it('ledgers nothing for a fully clamped spend at 0 coins', () => {
    const s = initialRewardState();
    const r = applyAward(s, 0, { source: 'feed', amount: -5, at: T0 });
    expect(r.coinsAfter).toBe(0);
    expect(r.awards).toEqual([]);
    expect(s.recent).toHaveLength(0);
    expect(s.totals.feed).toEqual({ earned: 0, spent: 0, count: 0 });
  });

  it('ledgers nothing for a zero requested amount (pomodoro stop with nothing accrued)', () => {
    const s = initialRewardState();
    const r = applyAward(s, 12, { source: 'pomodoro', amount: 0, at: T0 });
    expect(r.coinsAfter).toBe(12);
    expect(r.awards).toEqual([]);
    expect(s.recent).toHaveLength(0);
  });

  it('never reduces coins on a pomodoro commit (no rollback)', () => {
    const s = initialRewardState();
    const r = applyAward(s, 7, { source: 'pomodoro', amount: 3, at: T0 });
    expect(r.coinsAfter).toBeGreaterThanOrEqual(7);
    expect(r.coinsAfter).toBe(10);
  });

  it('ledgers a daily gift under source daily_gift', () => {
    const s = initialRewardState();
    applyAward(s, 0, { source: 'daily_gift', amount: 50, at: T0 });
    expect(s.recent[0].source).toBe('daily_gift');
    expect(s.totals.daily_gift).toEqual({ earned: 50, spent: 0, count: 1 });
  });

  it('aggregates earned/spent/count per source across a mixed sequence', () => {
    const s = initialRewardState();
    let coins = 0;
    coins = applyAward(s, coins, { source: 'agent_stop', amount: 20, at: T0 }).coinsAfter;
    coins = applyAward(s, coins, { source: 'agent_stop', amount: 20, at: T0 + 1 }).coinsAfter;
    coins = applyAward(s, coins, { source: 'feed', amount: -5, at: T0 + 2 }).coinsAfter;
    coins = applyAward(s, coins, { source: 'daily_gift', amount: 50, at: T0 + 3 }).coinsAfter;
    expect(coins).toBe(85);
    expect(s.totals.agent_stop).toEqual({ earned: 40, spent: 0, count: 2 });
    expect(s.totals.feed).toEqual({ earned: 0, spent: 5, count: 1 });
    expect(s.totals.daily_gift).toEqual({ earned: 50, spent: 0, count: 1 });
    expect(s.totals.pomodoro).toEqual({ earned: 0, spent: 0, count: 0 });
  });

  it('ledgers nothing for a non-finite amount or timestamp', () => {
    const s = initialRewardState();
    expect(applyAward(s, 10, { source: 'feed', amount: Number.NaN, at: T0 }).coinsAfter).toBe(10);
    const inf = Number.POSITIVE_INFINITY;
    expect(applyAward(s, 10, { source: 'daily_gift', amount: 5, at: inf }).coinsAfter).toBe(10);
    expect(s.recent).toHaveLength(0);
  });

  it('caps recent at LEDGER_RECENT_CAP, trimming the oldest, while totals keep everything', () => {
    const s = initialRewardState();
    let coins = 0;
    const n = LEDGER_RECENT_CAP + 5;
    for (let i = 0; i < n; i++) {
      coins = applyAward(s, coins, { source: 'daily_gift', amount: 1, at: T0 + i }).coinsAfter;
    }
    expect(s.recent).toHaveLength(LEDGER_RECENT_CAP);
    expect(s.recent[0].at).toBe(T0 + 5); // oldest five trimmed
    expect(s.recent[s.recent.length - 1].at).toBe(T0 + n - 1); // newest-last preserved
    expect(s.totals.daily_gift.count).toBe(n);
    expect(s.totals.daily_gift.earned).toBe(n);
  });
});

describe('awardAgentStop', () => {
  // Subagent stops, ESC interrupts, and compaction never reach the frontend: Rust emits
  // claude-task-complete only on a genuine main-agent Stop (pending_agents == 0, not
  // interrupted) or a permission-wait. The pure surface for "award nothing" is therefore
  // the waiting flag and the per-session cooldown below.
  it('awards AGENT_STOP_COINS for a genuine stop, with the sessionId on the entry', () => {
    const s = initialRewardState();
    const r = awardAgentStop(s, 0, stopEv('s1', T0));
    expect(r.coinsAfter).toBe(AGENT_STOP_COINS);
    expect(s.recent).toHaveLength(1);
    expect(s.recent[0].source).toBe('agent_stop');
    expect(s.recent[0].sessionId).toBe('s1');
    expect(s.recent[0].at).toBe(T0);
  });

  it('rewards once when the same sessionId completion enters twice within the cooldown', () => {
    const s = initialRewardState();
    const r1 = awardAgentStop(s, 0, stopEv('s1', T0));
    const r2 = awardAgentStop(s, r1.coinsAfter, stopEv('s1', T0 + 30_000));
    expect(r2.coinsAfter).toBe(AGENT_STOP_COINS);
    expect(r2.awards).toEqual([]);
    expect(s.recent).toHaveLength(1);
  });

  it('awards again for the same session at exactly the cooldown boundary (later turn)', () => {
    const s = initialRewardState();
    const r1 = awardAgentStop(s, 0, stopEv('s1', T0));
    const r2 = awardAgentStop(s, r1.coinsAfter, stopEv('s1', T0 + AGENT_STOP_COOLDOWN_MS));
    expect(r2.coinsAfter).toBe(AGENT_STOP_COINS * 2);
    expect(s.recent).toHaveLength(2);
  });

  it('still drops at one millisecond before the cooldown boundary', () => {
    const s = initialRewardState();
    const r1 = awardAgentStop(s, 0, stopEv('s1', T0));
    const r2 = awardAgentStop(s, r1.coinsAfter, stopEv('s1', T0 + AGENT_STOP_COOLDOWN_MS - 1));
    expect(r2.awards).toEqual([]);
    expect(r2.coinsAfter).toBe(AGENT_STOP_COINS);
  });

  it('does not extend the window on dropped duplicates (anchored to the last AWARD)', () => {
    const s = initialRewardState();
    let coins = awardAgentStop(s, 0, stopEv('s1', T0)).coinsAfter;
    coins = awardAgentStop(s, coins, stopEv('s1', T0 + 30_000)).coinsAfter; // dropped
    coins = awardAgentStop(s, coins, stopEv('s1', T0 + 59_000)).coinsAfter; // dropped
    const r = awardAgentStop(s, coins, stopEv('s1', T0 + 61_000)); // >= 60s after the AWARD
    expect(r.awards).toHaveLength(1);
    expect(r.coinsAfter).toBe(AGENT_STOP_COINS * 2);
  });

  it('awards nothing for waiting:true and leaves all state untouched', () => {
    const s = initialRewardState();
    const r = awardAgentStop(s, 0, stopEv('s1', T0, true));
    expect(r.coinsAfter).toBe(0);
    expect(r.awards).toEqual([]);
    expect(s.recent).toHaveLength(0);
    expect(Object.keys(s.sessionCooldowns)).toHaveLength(0);
  });

  it('does not let a waiting event poison the cooldown for a genuine stop right after', () => {
    const s = initialRewardState();
    awardAgentStop(s, 0, stopEv('s1', T0, true));
    const r = awardAgentStop(s, 0, stopEv('s1', T0 + 1_000));
    expect(r.coinsAfter).toBe(AGENT_STOP_COINS);
  });

  it('awards two different sessions back-to-back independently', () => {
    const s = initialRewardState();
    const r1 = awardAgentStop(s, 0, stopEv('s1', T0));
    const r2 = awardAgentStop(s, r1.coinsAfter, stopEv('s2', T0 + 1_000));
    expect(r2.coinsAfter).toBe(AGENT_STOP_COINS * 2);
    expect(s.recent).toHaveLength(2);
  });
});

describe('trackInputCount', () => {
  it('accrues counts below the first milestone without awarding', () => {
    const s = initialRewardState();
    const r = trackInputCount(s, 0, { count: INPUT_MILESTONE_STEP - 1, at: T0 });
    expect(r.awards).toEqual([]);
    expect(r.coinsAfter).toBe(0);
    expect(s.lifetimeInputCount).toBe(INPUT_MILESTONE_STEP - 1);
  });

  it('awards once when crossing 500, with milestone attribution', () => {
    const s = initialRewardState();
    trackInputCount(s, 0, { count: INPUT_MILESTONE_STEP - 1, at: T0 });
    const r = trackInputCount(s, 0, { count: 1, at: T0 + 1 });
    expect(r.coinsAfter).toBe(INPUT_MILESTONE_COINS);
    expect(r.awards).toHaveLength(1);
    expect(s.recent[0].reason).toBe(`milestone ${INPUT_MILESTONE_STEP}`);
    expect(s.lastAwardedMilestone).toBe(INPUT_MILESTONE_STEP);
  });

  it('awards when landing exactly on a milestone boundary', () => {
    const s = initialRewardState();
    const r = trackInputCount(s, 0, { count: INPUT_MILESTONE_STEP, at: T0 });
    expect(r.awards).toHaveLength(1);
    expect(r.coinsAfter).toBe(INPUT_MILESTONE_COINS);
  });

  it('awards each crossed milestone once when one batch jumps 499 -> 1001', () => {
    const s = initialRewardState();
    trackInputCount(s, 0, { count: 499, at: T0 });
    const r = trackInputCount(s, 0, { count: 502, at: T0 + 1 });
    expect(s.lifetimeInputCount).toBe(1001);
    expect(r.awards).toHaveLength(2);
    expect(r.coinsAfter).toBe(INPUT_MILESTONE_COINS * 2);
    expect(r.awards[0].reason).toBe('milestone 500');
    expect(r.awards[1].reason).toBe('milestone 1000');
    expect(s.lastAwardedMilestone).toBe(1000);
  });

  it('never re-awards an already-awarded milestone', () => {
    const s = initialRewardState();
    trackInputCount(s, 0, { count: INPUT_MILESTONE_STEP, at: T0 });
    const r = trackInputCount(s, INPUT_MILESTONE_COINS, { count: 499, at: T0 + 1 });
    expect(r.awards).toEqual([]);
    expect(r.coinsAfter).toBe(INPUT_MILESTONE_COINS);
  });

  it('treats a non-positive count as a no-op', () => {
    const s = initialRewardState();
    const r = trackInputCount(s, 0, { count: 0, at: T0 });
    expect(r.awards).toEqual([]);
    expect(s.lifetimeInputCount).toBe(0);
  });

  it('ignores a corrupt batch (non-finite count or timestamp) and keeps the loop finite', () => {
    const s = initialRewardState();
    trackInputCount(s, 0, { count: Number.POSITIVE_INFINITY, at: T0 });
    trackInputCount(s, 0, { count: 10, at: Number.NaN });
    expect(s.lifetimeInputCount).toBe(0);
    expect(s.recent).toHaveLength(0);
  });

  it('does not re-award after a restore (lastAwardedMilestone persists)', () => {
    const s = initialRewardState();
    trackInputCount(s, 0, { count: INPUT_MILESTONE_STEP, at: T0 });
    const restored = restoreRewardState(snapshotRewardState(s));
    const r = trackInputCount(restored, 0, { count: 1, at: T0 + 1 });
    expect(restored.lifetimeInputCount).toBe(INPUT_MILESTONE_STEP + 1);
    expect(r.awards).toEqual([]);
  });
});

describe('trackFocusInput', () => {
  it('awards FOCUS_BLOCK_COINS exactly once after 10 minutes of steady input', () => {
    const s = initialRewardState();
    const { coins, awards } = runFocus(s, 0, everyMinute(T0, T0 + FOCUS_BLOCK_MS));
    expect(awards).toHaveLength(1);
    expect(coins).toBe(FOCUS_BLOCK_COINS);
    expect(awards[0].source).toBe('focus_minutes');
  });

  it('awards nothing before the block completes (9m59s of activity)', () => {
    const s = initialRewardState();
    const ats = [...everyMinute(T0, T0 + 9 * MINUTE), T0 + FOCUS_BLOCK_MS - 1_000];
    const { awards } = runFocus(s, 0, ats);
    expect(awards).toEqual([]);
  });

  it('advances the baseline by exactly one block so the remainder carries over', () => {
    const s = initialRewardState();
    // Award fires at T0+605s -> baseline becomes T0+600s, so the next block
    // completes at T0+1200s, not T0+1205s.
    const firstLeg = [...everyMinute(T0, T0 + 9 * MINUTE), T0 + FOCUS_BLOCK_MS + 5_000];
    const r1 = runFocus(s, 0, firstLeg);
    expect(r1.awards).toHaveLength(1);
    // Keep the streak alive (gaps <= 90s) through the second block.
    const fill = runFocus(s, r1.coins, everyMinute(T0 + 11 * MINUTE, T0 + 19 * MINUTE));
    expect(fill.awards).toEqual([]);
    const beforeSecond = runFocus(s, fill.coins, [T0 + 2 * FOCUS_BLOCK_MS - 1_000]);
    expect(beforeSecond.awards).toEqual([]);
    const atSecond = runFocus(s, beforeSecond.coins, [T0 + 2 * FOCUS_BLOCK_MS]);
    expect(atSecond.awards).toHaveLength(1);
  });

  it('resets the streak after an input gap longer than FOCUS_GAP_RESET_MS', () => {
    const s = initialRewardState();
    const restart = T0 + 5 * MINUTE + FOCUS_GAP_RESET_MS + 1_000; // 91s+ of silence
    const ats = [
      ...everyMinute(T0, T0 + 5 * MINUTE),
      ...everyMinute(restart, restart + 9 * MINUTE), // only 9 minutes since the restart
    ];
    const { awards } = runFocus(s, 0, ats);
    expect(awards).toEqual([]);
    const paid = runFocus(s, 0, [restart + FOCUS_BLOCK_MS]); // 10 min after the restart
    expect(paid.awards).toHaveLength(1);
  });

  it('continues the streak across a gap of exactly FOCUS_GAP_RESET_MS', () => {
    const s = initialRewardState();
    const ats = [T0, T0 + FOCUS_GAP_RESET_MS, ...everyMinute(T0 + 2 * MINUTE, T0 + FOCUS_BLOCK_MS)];
    const { awards } = runFocus(s, 0, ats);
    expect(awards).toHaveLength(1);
  });

  it('pays consecutive blocks across 20 minutes of continuous activity', () => {
    const s = initialRewardState();
    const { coins, awards } = runFocus(s, 0, everyMinute(T0, T0 + 2 * FOCUS_BLOCK_MS));
    expect(awards).toHaveLength(2);
    expect(coins).toBe(FOCUS_BLOCK_COINS * 2);
  });

  it('accrues nothing and clears the streak while a pomodoro is active', () => {
    const s = initialRewardState();
    runFocus(s, 0, everyMinute(T0, T0 + 9 * MINUTE)); // 9-minute streak built
    const r = trackFocusInput(s, 0, { at: T0 + 9 * MINUTE + 30_000 }, { pomodoroActive: true });
    expect(r.awards).toEqual([]);
    expect(s.focus.streakStartAt).toBeNull();
  });

  it('starts a fresh streak after the pomodoro ends (no instant payout)', () => {
    const s = initialRewardState();
    runFocus(s, 0, everyMinute(T0, T0 + 9 * MINUTE));
    trackFocusInput(s, 0, { at: T0 + 9 * MINUTE + 10_000 }, { pomodoroActive: true });
    const resume = T0 + 10 * MINUTE;
    const tooEarly = runFocus(s, 0, everyMinute(resume, resume + 9 * MINUTE));
    expect(tooEarly.awards).toEqual([]); // old 9 minutes must not count
    const paid = runFocus(s, 0, [resume + FOCUS_BLOCK_MS]);
    expect(paid.awards).toHaveLength(1);
  });

  it('ignores a non-finite timestamp without touching the streak', () => {
    const s = initialRewardState();
    runFocus(s, 0, [T0]);
    const r = trackFocusInput(s, 0, { at: Number.POSITIVE_INFINITY }, { pomodoroActive: false });
    expect(r.awards).toEqual([]);
    expect(s.focus.streakStartAt).toBe(T0);
  });

  it('ignores an out-of-order timestamp without resetting the streak', () => {
    const s = initialRewardState();
    runFocus(s, 0, [T0, T0 + MINUTE]);
    const r = trackFocusInput(s, 0, { at: T0 + 30_000 }, { pomodoroActive: false }); // in the past
    expect(r.awards).toEqual([]);
    const { awards } = runFocus(s, 0, everyMinute(T0 + 2 * MINUTE, T0 + FOCUS_BLOCK_MS));
    expect(awards).toHaveLength(1); // streak from T0 survived intact
  });
});

describe('applyUserInput', () => {
  it('routes one event through both input sources, chaining the balance', () => {
    const s = initialRewardState();
    s.lifetimeInputCount = INPUT_MILESTONE_STEP - 1;
    runFocus(s, 0, everyMinute(T0, T0 + 9 * MINUTE));
    const r = applyUserInput(s, 0, inputEv(1, T0 + FOCUS_BLOCK_MS), { pomodoroActive: false });
    expect(r.awards).toHaveLength(2);
    expect(r.awards[0].source).toBe('input_milestone');
    expect(r.awards[1].source).toBe('focus_minutes');
    expect(r.coinsAfter).toBe(INPUT_MILESTONE_COINS + FOCUS_BLOCK_COINS);
  });

  it('still counts milestones during a pomodoro while focus accrues nothing', () => {
    const s = initialRewardState();
    s.lifetimeInputCount = INPUT_MILESTONE_STEP - 1;
    const r = applyUserInput(s, 0, inputEv(1, T0), { pomodoroActive: true });
    expect(r.awards).toHaveLength(1);
    expect(r.awards[0].source).toBe('input_milestone');
    expect(s.focus.streakStartAt).toBeNull();
  });
});

describe('snapshotRewardState / restoreRewardState', () => {
  it('round-trips totals, recent, input count, and milestone; runtime focus comes back fresh', () => {
    const s = initialRewardState();
    let coins = awardAgentStop(s, 0, stopEv('s1', T0)).coinsAfter;
    coins = trackInputCount(s, coins, { count: INPUT_MILESTONE_STEP, at: T0 + 1 }).coinsAfter;
    runFocus(s, coins, [T0 + 2, T0 + MINUTE]);
    const restored = restoreRewardState(snapshotRewardState(s));
    expect(restored.totals).toEqual(s.totals);
    expect(restored.recent).toEqual(s.recent);
    expect(restored.lifetimeInputCount).toBe(s.lifetimeInputCount);
    expect(restored.lastAwardedMilestone).toBe(s.lastAwardedMilestone);
    expect(restored.focus).toEqual({ streakStartAt: null, lastInputAt: null });
  });

  it('returns the initial state for null and undefined snapshots', () => {
    expect(restoreRewardState(null)).toEqual(initialRewardState());
    expect(restoreRewardState(undefined)).toEqual(initialRewardState());
  });

  it('backfills totals for any source missing from an older snapshot', () => {
    const s = initialRewardState();
    applyAward(s, 0, { source: 'daily_gift', amount: 50, at: T0 });
    const snap = snapshotRewardState(s);
    delete (snap.totals as Partial<Record<CoinSource, CoinSourceTotals>>).focus_minutes;
    const restored = restoreRewardState(snap);
    expect(restored.totals.focus_minutes).toEqual({ earned: 0, spent: 0, count: 0 });
    expect(restored.totals.daily_gift.earned).toBe(50);
  });

  it('re-trims an oversized restored ledger to LEDGER_RECENT_CAP', () => {
    const s = initialRewardState();
    let coins = 0;
    for (let i = 0; i < LEDGER_RECENT_CAP; i++) {
      coins = applyAward(s, coins, { source: 'daily_gift', amount: 1, at: T0 + i }).coinsAfter;
    }
    const snap = snapshotRewardState(s);
    snap.recent = [{ source: 'daily_gift', amount: 1, at: T0 - 1 }, ...snap.recent]; // 101 entries
    const restored = restoreRewardState(snap);
    expect(restored.recent).toHaveLength(LEDGER_RECENT_CAP);
    expect(restored.recent[0].at).toBe(T0); // oldest (injected) entry trimmed
  });

  it('snapshots copies: mutating the live state afterwards does not alter the snapshot', () => {
    const s = initialRewardState();
    applyAward(s, 0, { source: 'daily_gift', amount: 50, at: T0 });
    const snap = snapshotRewardState(s);
    applyAward(s, 50, { source: 'feed', amount: -5, at: T0 + 1 });
    expect(snap.recent).toHaveLength(1);
    expect(snap.totals.feed).toEqual({ earned: 0, spent: 0, count: 0 });
  });

  it('sanitizes corrupt numeric counters on restore', () => {
    const s = initialRewardState();
    const snap = snapshotRewardState(s);
    snap.lifetimeInputCount = Number.POSITIVE_INFINITY;
    snap.lastAwardedMilestone = -500;
    const restored = restoreRewardState(snap);
    expect(restored.lifetimeInputCount).toBe(0);
    expect(restored.lastAwardedMilestone).toBe(0);
  });

  it('re-arms session cooldowns from the restored ledger so a restart cannot farm +20', () => {
    const s = initialRewardState();
    awardAgentStop(s, 0, stopEv('s1', T0));
    const restored = restoreRewardState(snapshotRewardState(s));
    const dup = awardAgentStop(restored, 0, stopEv('s1', T0 + 10_000));
    expect(dup.awards).toEqual([]);
    const later = awardAgentStop(restored, 0, stopEv('s1', T0 + AGENT_STOP_COOLDOWN_MS));
    expect(later.awards).toHaveLength(1);
  });
});
