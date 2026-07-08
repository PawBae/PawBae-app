// Token feeding loop reducer tests: baseline priming, delta settlement, meal tiers,
// crumb carry, counter resets, and corrupt input. Store glue (hunger/affection/eat
// action) is covered in stores/pet-feed.test.ts.
import { describe, expect, it } from 'vitest';

import type { ClaudeStats } from '../types';
import {
  FEAST_HUNGER,
  FEAST_MIN_TOKENS,
  initialTokenFeedState,
  MEAL_HUNGER,
  MEAL_MIN_TOKENS,
  mealForTokens,
  nutritionOf,
  primeTokenBaseline,
  SNACK_HUNGER,
  SNACK_MIN_TOKENS,
  settleTokenMeal,
} from './token-feed';

describe('mealForTokens', () => {
  it('returns null below the snack threshold and for corrupt deltas', () => {
    expect(mealForTokens(0)).toBeNull();
    expect(mealForTokens(SNACK_MIN_TOKENS - 1)).toBeNull();
    expect(mealForTokens(-5_000)).toBeNull();
    expect(mealForTokens(Number.NaN)).toBeNull();
    expect(mealForTokens(Number.POSITIVE_INFINITY)).toBeNull();
  });

  it('maps deltas onto the three tiers at exact boundaries', () => {
    expect(mealForTokens(SNACK_MIN_TOKENS)).toEqual({
      tier: 'snack',
      restore: SNACK_HUNGER,
      tokens: SNACK_MIN_TOKENS,
    });
    expect(mealForTokens(MEAL_MIN_TOKENS - 1)?.tier).toBe('snack');
    expect(mealForTokens(MEAL_MIN_TOKENS)).toEqual({
      tier: 'meal',
      restore: MEAL_HUNGER,
      tokens: MEAL_MIN_TOKENS,
    });
    expect(mealForTokens(FEAST_MIN_TOKENS - 1)?.tier).toBe('meal');
    expect(mealForTokens(FEAST_MIN_TOKENS)).toEqual({
      tier: 'feast',
      restore: FEAST_HUNGER,
      tokens: FEAST_MIN_TOKENS,
    });
  });
});

describe('nutritionOf', () => {
  it('sums input and output tokens, excluding cache traffic', () => {
    const stats: ClaudeStats = {
      totalInputTokens: 10_000,
      totalOutputTokens: 4_000,
      totalCacheReadTokens: 9_999_999,
      totalCacheWriteTokens: 8_888_888,
      totalMessages: 42,
      totalSessions: 3,
    };
    expect(nutritionOf(stats)).toBe(14_000);
  });
});

describe('settleTokenMeal', () => {
  it('first sighting sets the baseline and feeds nothing (no lifetime-total feast)', () => {
    const s = initialTokenFeedState();
    expect(settleTokenMeal(s, 'cc', 5_000_000)).toBeNull();
    expect(s.baselines.cc).toBe(5_000_000);
  });

  it('feeds the tier for the delta since the last meal and advances the baseline', () => {
    const s = initialTokenFeedState();
    settleTokenMeal(s, 'cc', 100_000);
    const meal = settleTokenMeal(s, 'cc', 100_000 + MEAL_MIN_TOKENS);
    expect(meal).toEqual({ tier: 'meal', restore: MEAL_HUNGER, tokens: MEAL_MIN_TOKENS });
    expect(s.baselines.cc).toBe(100_000 + MEAL_MIN_TOKENS);
  });

  it('lets crumbs accumulate: sub-snack deltas leave the baseline unmoved', () => {
    const s = initialTokenFeedState();
    settleTokenMeal(s, 'cc', 10_000);
    expect(settleTokenMeal(s, 'cc', 10_000 + SNACK_MIN_TOKENS - 1)).toBeNull();
    expect(s.baselines.cc).toBe(10_000); // crumbs carry
    const meal = settleTokenMeal(s, 'cc', 10_000 + SNACK_MIN_TOKENS);
    expect(meal?.tier).toBe('snack');
    expect(s.baselines.cc).toBe(10_000 + SNACK_MIN_TOKENS);
  });

  it('re-baselines silently when the counter shrinks (deleted session files)', () => {
    const s = initialTokenFeedState();
    settleTokenMeal(s, 'cc', 500_000);
    expect(settleTokenMeal(s, 'cc', 100)).toBeNull();
    expect(s.baselines.cc).toBe(100);
    // And the next legitimate delta feeds from the new baseline.
    expect(settleTokenMeal(s, 'cc', 100 + SNACK_MIN_TOKENS)?.tier).toBe('snack');
  });

  it('rejects corrupt nutrition without touching state', () => {
    const s = initialTokenFeedState();
    settleTokenMeal(s, 'cc', 10_000);
    expect(settleTokenMeal(s, 'cc', Number.NaN)).toBeNull();
    expect(settleTokenMeal(s, 'cc', -1)).toBeNull();
    expect(settleTokenMeal(s, 'cc', Number.POSITIVE_INFINITY)).toBeNull();
    expect(s.baselines.cc).toBe(10_000);
    // A source with no baseline stays untouched too.
    expect(settleTokenMeal(s, 'codex', Number.NaN)).toBeNull();
    expect(s.baselines.codex).toBeUndefined();
  });

  it('tracks sources independently', () => {
    const s = initialTokenFeedState();
    settleTokenMeal(s, 'cc', 50_000);
    settleTokenMeal(s, 'codex', 1_000);
    const ccMeal = settleTokenMeal(s, 'cc', 50_000 + FEAST_MIN_TOKENS);
    expect(ccMeal?.tier).toBe('feast');
    expect(s.baselines.codex).toBe(1_000);
  });

  it('treats a hostile source key as an ordinary key', () => {
    const s = initialTokenFeedState();
    expect(settleTokenMeal(s, '__proto__', 1_000)).toBeNull();
    expect(s.baselines.__proto__).toBe(1_000);
    expect(initialTokenFeedState().baselines.__proto__).toBeUndefined(); // no pollution
  });
});

describe('primeTokenBaseline', () => {
  it('sets an absent baseline and refuses to rewind an existing one', () => {
    const s = initialTokenFeedState();
    primeTokenBaseline(s, 'cc', 42_000);
    expect(s.baselines.cc).toBe(42_000);
    // A late prime (init fetch racing a fast first completion) must not rewind the
    // baseline a settle already advanced — that would double-feed eaten tokens.
    primeTokenBaseline(s, 'cc', 10_000);
    expect(s.baselines.cc).toBe(42_000);
  });

  it('ignores corrupt values', () => {
    const s = initialTokenFeedState();
    primeTokenBaseline(s, 'cc', Number.NaN);
    primeTokenBaseline(s, 'codex', -5);
    primeTokenBaseline(s, 'cursor', Number.POSITIVE_INFINITY);
    expect(s.baselines.cc).toBeUndefined();
    expect(s.baselines.codex).toBeUndefined();
    expect(s.baselines.cursor).toBeUndefined();
  });
});
