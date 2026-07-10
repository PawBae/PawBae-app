import { describe, expect, it } from 'vitest';
import { DEFAULT_PET_ID } from './codex-pet';
import {
  ADVENTURE_EGG_CHANCE,
  addWarmth,
  EGG_HATCH_WARMTH,
  type EggState,
  eggReady,
  hatchablePool,
  rollNeighbor,
  sanitizeEgg,
  sanitizeMetNeighbors,
  shouldDropEgg,
  unmetNeighbors,
} from './eggs';

const BUILTINS = [DEFAULT_PET_ID, 'solu', 'muru', 'riffi'];

describe('hatchablePool / unmetNeighbors', () => {
  it('excludes Yoonie from the pool — she is never a hatch target', () => {
    expect(hatchablePool(BUILTINS)).toEqual(['solu', 'muru', 'riffi']);
  });

  it('filters met ids and leaves order intact', () => {
    expect(unmetNeighbors(['solu', 'muru', 'riffi'], ['muru'])).toEqual(['solu', 'riffi']);
    expect(unmetNeighbors(['solu'], ['solu'])).toEqual([]);
  });
});

describe('rollNeighbor', () => {
  it('returns null on an empty pool', () => {
    expect(rollNeighbor([], () => 0.5)).toBeNull();
  });

  it('is deterministic under injected entropy and never repeats across a full run', () => {
    let unmet = ['a', 'b', 'c'];
    const met: string[] = [];
    const rolls = [0.9, 0.0, 0.9]; // c, a, b
    for (const r of rolls) {
      const id = rollNeighbor(unmet, () => r);
      expect(id).not.toBeNull();
      expect(met).not.toContain(id);
      met.push(id as string);
      unmet = unmet.filter((u) => u !== id);
    }
    expect(met).toEqual(['c', 'a', 'b']);
  });

  it('clamps a rand() of exactly 1 into range', () => {
    expect(rollNeighbor(['a', 'b'], () => 1)).toBe('b');
  });
});

describe('warmth', () => {
  it('increments and caps at the hatch threshold', () => {
    let egg: EggState = { warmth: 0, since: 1 };
    for (let i = 0; i < EGG_HATCH_WARMTH + 3; i++) egg = addWarmth(egg);
    expect(egg.warmth).toBe(EGG_HATCH_WARMTH);
  });

  it('eggReady flips exactly at the threshold', () => {
    expect(eggReady(null)).toBe(false);
    expect(eggReady({ warmth: EGG_HATCH_WARMTH - 1, since: 1 })).toBe(false);
    expect(eggReady({ warmth: EGG_HATCH_WARMTH, since: 1 })).toBe(true);
  });
});

describe('shouldDropEgg', () => {
  const hit = () => ADVENTURE_EGG_CHANCE - 0.001;
  const miss = () => ADVENTURE_EGG_CHANCE;

  it('requires a long trip, no incubating egg, and someone left to meet', () => {
    expect(shouldDropEgg(true, null, 3, hit)).toBe(true);
    expect(shouldDropEgg(false, null, 3, hit)).toBe(false);
    expect(shouldDropEgg(true, { warmth: 0, since: 1 }, 3, hit)).toBe(false);
    expect(shouldDropEgg(true, null, 0, hit)).toBe(false);
  });

  it('respects the drop chance', () => {
    expect(shouldDropEgg(true, null, 3, miss)).toBe(false);
  });
});

describe('sanitizers', () => {
  it('sanitizeMetNeighbors keeps only non-empty strings and dedupes', () => {
    expect(sanitizeMetNeighbors(['a', 'a', '', 1, null, 'b'])).toEqual(['a', 'b']);
    expect(sanitizeMetNeighbors('nope')).toEqual([]);
    expect(sanitizeMetNeighbors(undefined)).toEqual([]);
  });

  it('sanitizeEgg collapses corrupt shapes to null and clamps warmth', () => {
    expect(sanitizeEgg(null)).toBeNull();
    expect(sanitizeEgg([])).toBeNull();
    expect(sanitizeEgg({ warmth: -1, since: 1 })).toBeNull();
    expect(sanitizeEgg({ warmth: 2, since: 0 })).toBeNull();
    expect(sanitizeEgg({ warmth: '3', since: 1 })).toBeNull();
    expect(sanitizeEgg({ warmth: Number.POSITIVE_INFINITY, since: 1 })).toBeNull();
    expect(sanitizeEgg({ warmth: 3.7, since: 5 })).toEqual({ warmth: 3, since: 5 });
    expect(sanitizeEgg({ warmth: 999, since: 5 })).toEqual({ warmth: EGG_HATCH_WARMTH, since: 5 });
  });
});
