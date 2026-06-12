// Achievement definitions + evaluator (Phase 6 growth system). Pure logic over the
// already-persisted reward totals — no new counters to maintain: every achievement is a
// predicate on state the ledger/store tracks anyway, so re-evaluating is idempotent and
// a lost unlock self-heals on the next check.
import type { CoinSource, CoinSourceTotals } from '../types';

export interface AchievementContext {
  totals: Record<CoinSource, CoinSourceTotals>;
  lifetimeInputCount: number;
  /** Validated consecutive-day gift streak (0 when broken). */
  giftStreak: number;
  /** Whole days since firstMeetAt. */
  daysTogether: number;
  /** Current evolution stage index. */
  stageIndex: number;
}

export interface AchievementDef {
  id: string;
  emoji: string;
  /** Hidden from the locked list (surprise unlocks). */
  secret?: boolean;
  check: (ctx: AchievementContext) => boolean;
}

function count(ctx: AchievementContext, src: CoinSource): number {
  const t = ctx.totals[src];
  return t && Number.isFinite(t.count) ? t.count : 0;
}

function earnedTotal(ctx: AchievementContext): number {
  let sum = 0;
  for (const t of Object.values(ctx.totals)) {
    if (t && Number.isFinite(t.earned) && t.earned > 0) sum += t.earned;
  }
  return sum;
}

// Order is display order in the panel. IDs are persisted in pet.json — never rename.
export const ACHIEVEMENTS: readonly AchievementDef[] = [
  // Agent companionship — the signature sources.
  { id: 'agent_first', emoji: '🤖', check: (c) => count(c, 'agent_stop') >= 1 },
  { id: 'agent_10', emoji: '🎖️', check: (c) => count(c, 'agent_stop') >= 10 },
  { id: 'agent_100', emoji: '🏆', check: (c) => count(c, 'agent_stop') >= 100 },
  { id: 'agent_500', emoji: '🛡️', check: (c) => count(c, 'agent_stop') >= 500 },
  // Focus.
  { id: 'focus_first', emoji: '🧘', check: (c) => count(c, 'focus_minutes') >= 1 },
  { id: 'focus_30', emoji: '🔥', check: (c) => count(c, 'focus_minutes') >= 30 },
  { id: 'pomodoro_first', emoji: '🍅', check: (c) => count(c, 'pomodoro') >= 1 },
  { id: 'pomodoro_20', emoji: '⏰', check: (c) => count(c, 'pomodoro') >= 20 },
  // Typing milestones.
  { id: 'typist_1k', emoji: '⌨️', check: (c) => c.lifetimeInputCount >= 1_000 },
  { id: 'typist_50k', emoji: '🎹', check: (c) => c.lifetimeInputCount >= 50_000 },
  { id: 'typist_500k', emoji: '🚀', check: (c) => c.lifetimeInputCount >= 500_000 },
  // Nurture.
  { id: 'feed_first', emoji: '🍖', check: (c) => count(c, 'feed') >= 1 },
  { id: 'feed_50', emoji: '🍱', check: (c) => count(c, 'feed') >= 50 },
  { id: 'gift_first', emoji: '🎁', check: (c) => count(c, 'daily_gift') >= 1 },
  { id: 'streak_3', emoji: '📅', check: (c) => c.giftStreak >= 3 },
  { id: 'streak_7', emoji: '🗓️', check: (c) => c.giftStreak >= 7 },
  { id: 'streak_30', emoji: '💎', check: (c) => c.giftStreak >= 30 },
  // Time together.
  { id: 'week_together', emoji: '💛', check: (c) => c.daysTogether >= 7 },
  { id: 'month_together', emoji: '🧡', check: (c) => c.daysTogether >= 30 },
  { id: 'hundred_days', emoji: '❤️', check: (c) => c.daysTogether >= 100 },
  // Wealth + evolution.
  { id: 'rich_500', emoji: '🪙', check: (c) => earnedTotal(c) >= 500 },
  { id: 'evolved_junior', emoji: '⭐', secret: true, check: (c) => c.stageIndex >= 2 },
  { id: 'evolved_master', emoji: '🌟', secret: true, check: (c) => c.stageIndex >= 3 },
  { id: 'evolved_legend', emoji: '👑', secret: true, check: (c) => c.stageIndex >= 4 },
] as const;

/**
 * Returns the defs newly satisfied by `ctx` that aren't in `unlocked` yet, in display
 * order. The caller records unlock timestamps; passing the updated map next time makes
 * the evaluation idempotent.
 */
export function evaluateAchievements(
  ctx: AchievementContext,
  unlocked: Record<string, number>,
): AchievementDef[] {
  const fresh: AchievementDef[] = [];
  for (const def of ACHIEVEMENTS) {
    if (unlocked[def.id] !== undefined) continue;
    if (def.check(ctx)) fresh.push(def);
  }
  return fresh;
}

/**
 * Coerce a persisted unlock map back to a safe shape: unknown ids are kept (forward
 * compat with newer builds), but values must be finite timestamps. Null-prototype for
 * the same reason as sessionCooldowns — a hostile "__proto__" key stays an ordinary key.
 */
export function sanitizeUnlockMap(raw: unknown): Record<string, number> {
  const out: Record<string, number> = Object.create(null);
  if (typeof raw !== 'object' || raw === null) return out;
  for (const [k, v] of Object.entries(raw as Record<string, unknown>)) {
    const n = typeof v === 'number' ? v : Number(v);
    if (Number.isFinite(n) && n > 0) out[k] = n;
  }
  return out;
}
