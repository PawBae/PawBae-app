import { describe, expect, it } from 'vitest';
import type { CoinSource, CoinSourceTotals } from '../types';
import {
  ACHIEVEMENTS,
  type AchievementContext,
  evaluateAchievements,
  sanitizeUnlockMap,
} from './achievements';
import { initialRewardState } from './rewards';

function ctxWith(overrides: Partial<AchievementContext> = {}): AchievementContext {
  return {
    totals: initialRewardState().totals,
    lifetimeInputCount: 0,
    giftStreak: 0,
    daysTogether: 0,
    stageIndex: 0,
    ...overrides,
  };
}

function totalsWith(
  parts: Partial<Record<CoinSource, Partial<CoinSourceTotals>>>,
): Record<CoinSource, CoinSourceTotals> {
  const totals = initialRewardState().totals;
  for (const [src, t] of Object.entries(parts)) {
    Object.assign(totals[src as CoinSource], t);
  }
  return totals;
}

describe('ACHIEVEMENTS config', () => {
  it('has unique ids (they are persisted — collisions would merge unlocks)', () => {
    const ids = ACHIEVEMENTS.map((d) => d.id);
    expect(new Set(ids).size).toBe(ids.length);
  });
});

describe('evaluateAchievements', () => {
  it('unlocks nothing on a fresh context', () => {
    expect(evaluateAchievements(ctxWith(), {})).toEqual([]);
  });

  it('unlocks count-driven achievements at their thresholds', () => {
    const ctx = ctxWith({ totals: totalsWith({ agent_stop: { count: 10, earned: 200 } }) });
    const ids = evaluateAchievements(ctx, {}).map((d) => d.id);
    expect(ids).toContain('agent_first');
    expect(ids).toContain('agent_10');
    expect(ids).not.toContain('agent_100');
  });

  it('never re-reports an already-unlocked achievement', () => {
    const ctx = ctxWith({ totals: totalsWith({ agent_stop: { count: 1 } }) });
    const first = evaluateAchievements(ctx, {});
    expect(first.map((d) => d.id)).toContain('agent_first');
    const unlocked = Object.fromEntries(first.map((d) => [d.id, 123]));
    expect(evaluateAchievements(ctx, unlocked)).toEqual([]);
  });

  it('covers streak, time-together and evolution predicates', () => {
    const ids = evaluateAchievements(
      ctxWith({ giftStreak: 7, daysTogether: 30, stageIndex: 3 }),
      {},
    ).map((d) => d.id);
    expect(ids).toEqual(
      expect.arrayContaining([
        'streak_3',
        'streak_7',
        'week_together',
        'month_together',
        'evolved_junior',
        'evolved_master',
      ]),
    );
    expect(ids).not.toContain('streak_30');
    expect(ids).not.toContain('hundred_days');
    expect(ids).not.toContain('evolved_legend');
  });

  it('sums lifetime earnings for the treasury achievement', () => {
    const ctx = ctxWith({
      totals: totalsWith({
        agent_stop: { earned: 300 },
        daily_gift: { earned: 200 },
      }),
    });
    expect(evaluateAchievements(ctx, {}).map((d) => d.id)).toContain('rich_500');
  });
});

describe('sanitizeUnlockMap', () => {
  it('keeps finite timestamps and drops garbage values', () => {
    const out = sanitizeUnlockMap({
      agent_first: 1000,
      feed_first: 'soon',
      gift_first: Number.NaN,
      unknown_future_id: 2000, // forward compat: newer builds' ids survive a downgrade
    });
    expect(out.agent_first).toBe(1000);
    expect(out.unknown_future_id).toBe(2000);
    expect('feed_first' in out).toBe(false);
    expect('gift_first' in out).toBe(false);
  });

  it('handles non-object input and keeps __proto__ an ordinary key', () => {
    expect(sanitizeUnlockMap(null)).toEqual({});
    expect(sanitizeUnlockMap('x')).toEqual({});
    const hostile = JSON.parse('{"__proto__": 1234}');
    const out = sanitizeUnlockMap(hostile);
    expect(Object.getPrototypeOf(out)).toBeNull();
    expect(out.__proto__).toBe(1234);
  });
});
