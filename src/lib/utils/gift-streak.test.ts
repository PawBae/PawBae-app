// Daily-gift streak math (rewards.ts). Dates are the store's UTC YYYY-MM-DD strings.
import { describe, expect, it } from 'vitest';
import {
  currentGiftStreak,
  DAILY_GIFT_COINS,
  DAILY_GIFT_STREAK_BONUS,
  DAILY_GIFT_STREAK_CAP,
  dailyGiftAmount,
  nextGiftStreak,
} from './rewards';

const TODAY = '2026-06-12';
const YESTERDAY = '2026-06-11';
const LAST_WEEK = '2026-06-05';

describe('nextGiftStreak', () => {
  it('extends a streak claimed yesterday', () => {
    expect(nextGiftStreak(YESTERDAY, TODAY, 3)).toBe(4);
  });

  it('restarts at 1 after a gap, on first claim, and across a month boundary', () => {
    expect(nextGiftStreak(LAST_WEEK, TODAY, 9)).toBe(1);
    expect(nextGiftStreak('', TODAY, 0)).toBe(1);
    // 05-31 → 06-01 is consecutive despite the month rollover.
    expect(nextGiftStreak('2026-05-31', '2026-06-01', 2)).toBe(3);
  });

  it('collapses corrupt stored streaks to a restart', () => {
    expect(nextGiftStreak(YESTERDAY, TODAY, Number.NaN)).toBe(1);
    expect(nextGiftStreak(YESTERDAY, TODAY, -5)).toBe(1);
  });

  it('survives a garbage today string without throwing', () => {
    expect(nextGiftStreak(YESTERDAY, 'not-a-date', 3)).toBe(1);
  });
});

describe('currentGiftStreak', () => {
  it('reports the stored streak while alive (claimed today or yesterday)', () => {
    expect(currentGiftStreak(TODAY, TODAY, 5)).toBe(5);
    expect(currentGiftStreak(YESTERDAY, TODAY, 5)).toBe(5);
  });

  it('reports 0 once the streak is broken or never started', () => {
    expect(currentGiftStreak(LAST_WEEK, TODAY, 5)).toBe(0);
    expect(currentGiftStreak('', TODAY, 0)).toBe(0);
  });
});

describe('dailyGiftAmount', () => {
  it('pays the base on day 1 and adds the bonus per consecutive day', () => {
    expect(dailyGiftAmount(1)).toBe(DAILY_GIFT_COINS);
    expect(dailyGiftAmount(3)).toBe(DAILY_GIFT_COINS + 2 * DAILY_GIFT_STREAK_BONUS);
  });

  it('caps the bonus and tolerates degenerate streak values', () => {
    const max = DAILY_GIFT_COINS + (DAILY_GIFT_STREAK_CAP - 1) * DAILY_GIFT_STREAK_BONUS;
    expect(dailyGiftAmount(DAILY_GIFT_STREAK_CAP)).toBe(max);
    expect(dailyGiftAmount(365)).toBe(max);
    expect(dailyGiftAmount(0)).toBe(DAILY_GIFT_COINS);
    expect(dailyGiftAmount(Number.NaN)).toBe(DAILY_GIFT_COINS);
  });
});
