// Egg hatching + species dex (阶段二 孵蛋与物种图鉴). Pure logic, zero Svelte/Tauri
// imports — mirrors souvenirs.ts. The hatchable pool is DERIVED from the builtin
// roster minus Yoonie, so shrinking the default bundle (pre-Steam IP cleanup) or
// growing it later needs no changes here. Custom skins never enter the pool and are
// never gated. See docs/superpowers/specs/2026-07-09-egg-dex-design.md.

import { DEFAULT_PET_ID } from './codex-pet';

export const EGG_COST_COINS = 150;
export const EGG_HATCH_WARMTH = 8;
/** Long adventures (≥ LONG_TRIP_MS) may bring a free egg home instead of a souvenir. */
export const ADVENTURE_EGG_CHANCE = 0.1;

/** One incubating egg, persisted in pet.json. Ready-to-hatch is derived from warmth. */
export interface EggState {
  warmth: number;
  since: number; // epoch ms of purchase/drop — display only, never expires (never punish)
}

/** The ids an egg can ever hatch: every builtin except Yoonie (she's the first arrival). */
export function hatchablePool(builtinIds: readonly string[]): string[] {
  return builtinIds.filter((id) => id !== DEFAULT_PET_ID);
}

/** Neighbors not yet met — the only ids a hatch can reveal (no duplicates, ever). */
export function unmetNeighbors(pool: readonly string[], met: readonly string[]): string[] {
  return pool.filter((id) => !met.includes(id));
}

/** Uniform roll over the unmet pool; the caller owns the entropy (tests inject it). */
export function rollNeighbor(unmet: readonly string[], rand: () => number): string | null {
  if (unmet.length === 0) return null;
  const i = Math.min(unmet.length - 1, Math.floor(rand() * unmet.length));
  return unmet[i];
}

export function eggReady(egg: EggState | null): boolean {
  return egg !== null && egg.warmth >= EGG_HATCH_WARMTH;
}

/** One unit of 完工暖香 (a genuine agent completion or a meal). Capped at the threshold. */
export function addWarmth(egg: EggState): EggState {
  return { ...egg, warmth: Math.min(EGG_HATCH_WARMTH, egg.warmth + 1) };
}

/**
 * Whether a settled long trip brings back a free egg INSTEAD of a souvenir. Gated on
 * no egg already incubating (one at a time) and someone left to meet — otherwise the
 * roll isn't even made and the souvenir drop proceeds as usual.
 */
export function shouldDropEgg(
  longTrip: boolean,
  egg: EggState | null,
  unmetCount: number,
  rand: () => number,
): boolean {
  if (!longTrip || egg !== null || unmetCount <= 0) return false;
  return rand() < ADVENTURE_EGG_CHANCE;
}

/** Defensive read of the persisted met list: strings only, deduped. */
export function sanitizeMetNeighbors(raw: unknown): string[] {
  if (!Array.isArray(raw)) return [];
  return [...new Set(raw.filter((v): v is string => typeof v === 'string' && v.length > 0))];
}

/** Defensive read of the persisted egg: a corrupt shape collapses to "no egg". */
export function sanitizeEgg(raw: unknown): EggState | null {
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) return null;
  const o = raw as Record<string, unknown>;
  const warmth = o.warmth;
  const since = o.since;
  if (typeof warmth !== 'number' || !Number.isFinite(warmth) || warmth < 0) return null;
  if (typeof since !== 'number' || !Number.isFinite(since) || since <= 0) return null;
  return { warmth: Math.min(EGG_HATCH_WARMTH, Math.floor(warmth)), since };
}
