// Token feeding loop reducer (Phase 1 core loop). Pure logic, zero Svelte/Tauri
// imports — mirrors the rewards.ts precedent. A completed agent task converts the
// tokens it burned into a meal that restores hunger; token spend never DRAINS hunger
// (the strategy's no-punishment red line).
//
// Baselines are per-run in-memory watermarks of "nutrition" (input + output tokens,
// cache traffic excluded) per stats source. The Svelte store primes them at init from
// get_claude_stats and settles a delta against them on each genuine completion.
// See docs/superpowers/specs/2026-07-07-token-feeding-loop-design.md.
import type { ClaudeStats, ClaudeStatsSource } from '../types';

export const SNACK_MIN_TOKENS = 2_000;
export const MEAL_MIN_TOKENS = 60_000;
export const FEAST_MIN_TOKENS = 300_000;
export const SNACK_HUNGER = 5;
export const MEAL_HUNGER = 12;
export const FEAST_HUNGER = 20;

/** Every stats source the init-time baseline priming sweeps. */
export const TOKEN_FEED_SOURCES: readonly ClaudeStatsSource[] = ['cc', 'codex', 'cursor'];

export type MealTier = 'snack' | 'meal' | 'feast';

export interface TokenMeal {
  tier: MealTier;
  restore: number; // hunger points this meal restores
  tokens: number; // the settled nutrition delta
}

export interface TokenFeedState {
  // Nutrition watermark at the last settled meal (or the init prime) per source.
  // Null prototype so a hostile key like "__proto__" stays an ordinary key.
  baselines: Record<string, number>;
}

export function initialTokenFeedState(): TokenFeedState {
  return { baselines: Object.create(null) as Record<string, number> };
}

/**
 * The "nutrition" of a stats snapshot: input + output tokens. Cache read/write
 * traffic is deliberately excluded — cache reads are cheap and would inflate every
 * meal to a feast. Corrupt fields propagate to NaN, which settleTokenMeal rejects.
 */
export function nutritionOf(
  stats: Pick<ClaudeStats, 'totalInputTokens' | 'totalOutputTokens'>,
): number {
  return stats.totalInputTokens + stats.totalOutputTokens;
}

/** Map a settled nutrition delta onto a meal tier; below a snack (or corrupt) is null. */
export function mealForTokens(delta: number): TokenMeal | null {
  if (!Number.isFinite(delta) || delta < SNACK_MIN_TOKENS) return null;
  if (delta >= FEAST_MIN_TOKENS) return { tier: 'feast', restore: FEAST_HUNGER, tokens: delta };
  if (delta >= MEAL_MIN_TOKENS) return { tier: 'meal', restore: MEAL_HUNGER, tokens: delta };
  return { tier: 'snack', restore: SNACK_HUNGER, tokens: delta };
}

/**
 * Init-time baseline priming. Only fills an ABSENT baseline: a late prime (the init
 * fetch racing a fast first completion) must never rewind a watermark a settle already
 * advanced — that would feed the same tokens twice.
 */
export function primeTokenBaseline(s: TokenFeedState, source: string, nutrition: number): void {
  if (!Number.isFinite(nutrition) || nutrition < 0) return;
  if (s.baselines[source] !== undefined) return;
  s.baselines[source] = nutrition;
}

/**
 * Settle a completion against the source's watermark and return the meal it earned.
 * First sighting and shrinking counters (deleted session files) re-baseline silently —
 * never a lifetime-total feast, never a negative meal. A sub-snack delta returns null
 * WITHOUT moving the baseline, so crumbs from tiny turns accumulate into a snack.
 */
export function settleTokenMeal(
  s: TokenFeedState,
  source: string,
  nutrition: number,
): TokenMeal | null {
  if (!Number.isFinite(nutrition) || nutrition < 0) return null;
  const baseline = s.baselines[source];
  if (baseline === undefined || nutrition < baseline) {
    s.baselines[source] = nutrition;
    return null;
  }
  const meal = mealForTokens(nutrition - baseline);
  if (meal) s.baselines[source] = nutrition;
  return meal;
}
