// Pure reward reducer (Phase 1-C). Pure logic, zero Svelte/Tauri imports — mirrors the
// physics state-machine precedent. Everything is timestamp-driven (`at` comes from the
// caller or the event payload), so unit tests need no timers or mocks.
//
// `applyAward` is the single gate: every coin ever minted or spent passes through it and
// is ledgered with source attribution. The per-source helpers (`awardAgentStop`,
// `trackInputCount`, `trackFocusInput`) are gatekeepers that decide whether/how often to
// call it. The Svelte store feeds events in, applies `coinsAfter` to petData (its own
// immutable-spread discipline), and persists snapshots.
import type {
  CoinAward,
  CoinSource,
  CoinSourceTotals,
  RewardLedgerSnapshot,
  UserInputEvent,
} from '../types';

export const AGENT_STOP_COINS = 20;
export const AGENT_STOP_COOLDOWN_MS = 60_000;
export const FOCUS_BLOCK_COINS = 5;
export const FOCUS_BLOCK_MS = 600_000; // 10 min of continuous input activity per block
export const FOCUS_GAP_RESET_MS = 90_000; // input silence beyond this resets the streak
export const INPUT_MILESTONE_STEP = 500;
export const INPUT_MILESTONE_COINS = 2;
export const LEDGER_RECENT_CAP = 100;
export const DAILY_GIFT_COINS = 50;
export const DAILY_GIFT_STREAK_BONUS = 5; // extra coins per consecutive day beyond the first
export const DAILY_GIFT_STREAK_CAP = 7; // bonus stops growing here (50 → 80 coins)
export const FEED_COST_COINS = 5;

const COIN_SOURCES: readonly CoinSource[] = [
  'agent_stop',
  'focus_minutes',
  'input_milestone',
  'pomodoro',
  'daily_gift',
  'feed',
];

export interface FocusStreakState {
  streakStartAt: number | null; // baseline of the current streak; null = no streak
  lastInputAt: number | null; // most recent counted input timestamp
}

export interface MutableRewardState {
  // Persisted via snapshotRewardState():
  totals: Record<CoinSource, CoinSourceTotals>;
  recent: CoinAward[]; // newest-last, capped at LEDGER_RECENT_CAP
  lifetimeInputCount: number;
  lastAwardedMilestone: number; // highest paid boundary: 0, 500, 1000, ...
  // Ephemeral (rebuilt each run): a focus streak intentionally does not survive a
  // restart, and cooldowns are re-armed from the restored ledger in restoreRewardState().
  // A plain Record (not a Map) so Svelte's $state deep proxy covers it; created with a
  // null prototype so a hostile sessionId like "__proto__" stays an ordinary key.
  sessionCooldowns: Record<string, number>; // sessionId -> at of the last agent_stop AWARD
  focus: FocusStreakState;
}

export interface AwardInput {
  source: CoinSource;
  amount: number; // requested delta, pre-clamp
  at: number; // epoch ms
  reason?: string;
  sessionId?: string;
}

export interface AwardResult {
  awards: CoinAward[]; // 0..n entries actually ledgered (effective amounts)
  coinsAfter: number; // balance after applying all of them
}

export interface AgentStopInput {
  sessionId: string;
  waiting: boolean;
  at: number;
}

export interface FocusContext {
  pomodoroActive: boolean;
}

/**
 * Coerce a value read from disk into a safe non-negative integer counter. Guards the
 * reducers' while-loops (and the displayed balance) against a corrupt or hand-edited
 * pet.json: strings, negatives, NaN, and Infinity all collapse to 0.
 */
export function sanitizeStoredCount(raw: unknown): number {
  const n = typeof raw === 'number' ? raw : Number(raw);
  return Number.isFinite(n) && n >= 0 ? Math.floor(n) : 0;
}

function zeroSourceTotals(): CoinSourceTotals {
  return { earned: 0, spent: 0, count: 0 };
}

function zeroTotals(): Record<CoinSource, CoinSourceTotals> {
  return {
    agent_stop: zeroSourceTotals(),
    focus_minutes: zeroSourceTotals(),
    input_milestone: zeroSourceTotals(),
    pomodoro: zeroSourceTotals(),
    daily_gift: zeroSourceTotals(),
    feed: zeroSourceTotals(),
  };
}

export function initialRewardState(): MutableRewardState {
  return {
    totals: zeroTotals(),
    recent: [],
    lifetimeInputCount: 0,
    lastAwardedMilestone: 0,
    sessionCooldowns: Object.create(null) as Record<string, number>,
    focus: { streakStartAt: null, lastInputAt: null },
  };
}

/**
 * The single coin-mutation gate. Clamps the balance at zero (preserving the historical
 * `Math.max(0, coins - 5)` feed behavior) and ledgers the EFFECTIVE delta; a zero-effect
 * award (fully clamped spend, empty pomodoro commit) is not ledgered at all.
 */
export function applyAward(
  s: MutableRewardState,
  coinsBefore: number,
  input: AwardInput,
): AwardResult {
  if (!Number.isFinite(input.amount) || !Number.isFinite(input.at)) {
    return { awards: [], coinsAfter: coinsBefore }; // corrupt input — never poison the ledger
  }
  const coinsAfter = Math.max(0, coinsBefore + input.amount);
  const effective = coinsAfter - coinsBefore;
  if (effective === 0) return { awards: [], coinsAfter: coinsBefore };
  const entry: CoinAward = { source: input.source, amount: effective, at: input.at };
  if (input.reason !== undefined) entry.reason = input.reason;
  if (input.sessionId !== undefined) entry.sessionId = input.sessionId;
  s.recent.push(entry);
  if (s.recent.length > LEDGER_RECENT_CAP) {
    s.recent.splice(0, s.recent.length - LEDGER_RECENT_CAP);
  }
  const totals = s.totals[input.source];
  if (effective > 0) totals.earned += effective;
  else totals.spent += -effective;
  totals.count += 1;
  return { awards: [entry], coinsAfter };
}

/**
 * Agent-completion reward. The wire event is already filtered by Rust (no subagent stops,
 * ESC interrupts, or compaction); here we drop permission-waits and dedupe per session
 * with a cooldown anchored to the last AWARD — dropped duplicates do not refresh the
 * anchor, so a duplicate stream can never starve a later legitimate turn.
 */
export function awardAgentStop(
  s: MutableRewardState,
  coinsBefore: number,
  input: AgentStopInput,
): AwardResult {
  if (input.waiting || !Number.isFinite(input.at)) {
    return { awards: [], coinsAfter: coinsBefore };
  }
  const lastAwardAt = s.sessionCooldowns[input.sessionId];
  if (lastAwardAt !== undefined && input.at - lastAwardAt < AGENT_STOP_COOLDOWN_MS) {
    return { awards: [], coinsAfter: coinsBefore };
  }
  s.sessionCooldowns[input.sessionId] = input.at;
  return applyAward(s, coinsBefore, {
    source: 'agent_stop',
    amount: AGENT_STOP_COINS,
    at: input.at,
    sessionId: input.sessionId,
  });
}

/**
 * Lifetime input counting with milestone payouts: +INPUT_MILESTONE_COINS per
 * INPUT_MILESTONE_STEP boundary crossed, one ledger entry per boundary (a batched count
 * can cross several at once). `lastAwardedMilestone` persists, so milestones are
 * lifetime-once.
 */
export function trackInputCount(
  s: MutableRewardState,
  coinsBefore: number,
  ev: { count: number; at: number },
): AwardResult {
  if (!Number.isFinite(ev.count) || ev.count <= 0 || !Number.isFinite(ev.at)) {
    return { awards: [], coinsAfter: coinsBefore }; // corrupt batch — also keeps the loop finite
  }
  s.lifetimeInputCount += ev.count;
  let coins = coinsBefore;
  const awards: CoinAward[] = [];
  while (s.lastAwardedMilestone + INPUT_MILESTONE_STEP <= s.lifetimeInputCount) {
    s.lastAwardedMilestone += INPUT_MILESTONE_STEP;
    const r = applyAward(s, coins, {
      source: 'input_milestone',
      amount: INPUT_MILESTONE_COINS,
      at: ev.at,
      reason: `milestone ${s.lastAwardedMilestone}`,
    });
    coins = r.coinsAfter;
    awards.push(...r.awards);
  }
  return { awards, coinsAfter: coins };
}

/**
 * Drop the current focus streak. Called eagerly when a pomodoro STARTS: pomodoro time
 * must never count toward focus_minutes, and waiting for the next input event to clear
 * the streak lazily would let a short/canceled pomodoro with no input during it carry
 * the old streak across the gap window (double-count).
 */
export function clearFocusStreak(s: MutableRewardState): void {
  s.focus.streakStartAt = null;
  s.focus.lastInputAt = null;
}

/**
 * Focus streak: a run of input events with no gap over FOCUS_GAP_RESET_MS. Every full
 * FOCUS_BLOCK_MS of streak pays FOCUS_BLOCK_COINS and advances the baseline by exactly
 * one block (the remainder carries — no drift). While a pomodoro is active the streak is
 * CLEARED, not frozen: pomodoro already pays for that time, and freezing would let a
 * pre-pomodoro streak pay out instantly afterwards (double-count).
 */
export function trackFocusInput(
  s: MutableRewardState,
  coinsBefore: number,
  ev: { at: number },
  ctx: FocusContext,
): AwardResult {
  if (!Number.isFinite(ev.at)) {
    return { awards: [], coinsAfter: coinsBefore }; // corrupt timestamp would never exit the loop
  }
  const focus = s.focus;
  if (ctx.pomodoroActive) {
    clearFocusStreak(s);
    return { awards: [], coinsAfter: coinsBefore };
  }
  if (focus.lastInputAt !== null && ev.at < focus.lastInputAt) {
    return { awards: [], coinsAfter: coinsBefore }; // clock went backwards — ignore
  }
  if (focus.lastInputAt !== null && ev.at - focus.lastInputAt > FOCUS_GAP_RESET_MS) {
    focus.streakStartAt = ev.at; // silence broke the streak; this event starts a new one
  } else if (focus.streakStartAt === null) {
    focus.streakStartAt = ev.at;
  }
  focus.lastInputAt = ev.at;
  let coins = coinsBefore;
  const awards: CoinAward[] = [];
  while (focus.streakStartAt !== null && ev.at - focus.streakStartAt >= FOCUS_BLOCK_MS) {
    focus.streakStartAt += FOCUS_BLOCK_MS;
    const r = applyAward(s, coins, {
      source: 'focus_minutes',
      amount: FOCUS_BLOCK_COINS,
      at: ev.at,
      reason: 'focus 10min',
    });
    coins = r.coinsAfter;
    awards.push(...r.awards);
  }
  return { awards, coinsAfter: coins };
}

// ── Daily-gift streak ──────────────────────────────────────────────
// Dates are the store's UTC YYYY-MM-DD strings (todayStr() in pet.svelte.ts), so
// "yesterday" is computed in the same calendar and DST can't split a streak.

function yesterdayOf(today: string): string {
  const parsed = Date.parse(`${today}T00:00:00Z`);
  if (!Number.isFinite(parsed)) return '';
  return new Date(parsed - 86_400_000).toISOString().slice(0, 10);
}

/**
 * The streak value for a gift claimed `today`: consecutive with the previous claim
 * (yesterday) extends it, anything else — including a corrupt stored streak — restarts
 * at 1. Same-day double claims never reach this (claimDailyGift gates on the date).
 */
export function nextGiftStreak(prevGiftDate: string, today: string, prevStreak: number): number {
  const prev = sanitizeStoredCount(prevStreak);
  if (prev >= 1 && prevGiftDate !== '' && prevGiftDate === yesterdayOf(today)) return prev + 1;
  return 1;
}

/** Display streak: the stored value while it's still alive (claimed today or yesterday), else 0. */
export function currentGiftStreak(
  lastGiftDate: string,
  today: string,
  storedStreak: number,
): number {
  const streak = sanitizeStoredCount(storedStreak);
  if (lastGiftDate === today || (lastGiftDate !== '' && lastGiftDate === yesterdayOf(today))) {
    return streak;
  }
  return 0;
}

/** Gift payout for a given streak: base + bonus per extra consecutive day, capped. */
export function dailyGiftAmount(streak: number): number {
  const s = Math.max(1, Math.min(DAILY_GIFT_STREAK_CAP, sanitizeStoredCount(streak)));
  return DAILY_GIFT_COINS + DAILY_GIFT_STREAK_BONUS * (s - 1);
}

/** One batched user-input event drives both input-derived sources, chaining the balance. */
export function applyUserInput(
  s: MutableRewardState,
  coinsBefore: number,
  ev: UserInputEvent,
  ctx: FocusContext,
): AwardResult {
  const milestones = trackInputCount(s, coinsBefore, ev);
  const focus = trackFocusInput(s, milestones.coinsAfter, ev, ctx);
  return { awards: [...milestones.awards, ...focus.awards], coinsAfter: focus.coinsAfter };
}

/** Deep-copied persisted slice; later mutation of the live state cannot alias into it. */
export function snapshotRewardState(s: MutableRewardState): RewardLedgerSnapshot {
  const totals = zeroTotals();
  for (const src of COIN_SOURCES) {
    const t = s.totals[src];
    totals[src] = { earned: t.earned, spent: t.spent, count: t.count };
  }
  return {
    totals,
    recent: s.recent.map((e) => ({ ...e })),
    lifetimeInputCount: s.lifetimeInputCount,
    lastAwardedMilestone: s.lastAwardedMilestone,
  };
}

/**
 * Rebuild reward state from a persisted snapshot. Defensive about old/corrupt data:
 * missing snapshot -> initial state; missing source keys are backfilled with zeros; the
 * ledger is re-trimmed to the cap. Session cooldowns are re-armed from the restored
 * agent_stop entries so a quick app restart cannot farm the same completion twice.
 */
export function restoreRewardState(
  snap: RewardLedgerSnapshot | null | undefined,
): MutableRewardState {
  const s = initialRewardState();
  if (!snap) return s;
  for (const src of COIN_SOURCES) {
    const t = snap.totals ? snap.totals[src] : undefined;
    if (t) s.totals[src] = { earned: t.earned, spent: t.spent, count: t.count };
  }
  const recent = Array.isArray(snap.recent) ? snap.recent : [];
  s.recent = recent.slice(-LEDGER_RECENT_CAP).map((e) => ({ ...e }));
  s.lifetimeInputCount = sanitizeStoredCount(snap.lifetimeInputCount);
  s.lastAwardedMilestone = sanitizeStoredCount(snap.lastAwardedMilestone);
  for (const entry of s.recent) {
    if (
      entry.source === 'agent_stop' &&
      entry.sessionId !== undefined &&
      Number.isFinite(entry.at)
    ) {
      s.sessionCooldowns[entry.sessionId] = entry.at;
    }
  }
  return s;
}
