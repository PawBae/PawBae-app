import { describe, expect, it } from 'vitest';
import type { AnimationRow } from './codex-pet';
import {
  availableIdleActions,
  IDLE_JITTER_FRACTION,
  nextIdleDelayMs,
  pickIdleAction,
} from './idle-actions';

const row = (r: number): AnimationRow => ({ row: r, frames: 4 });

describe('availableIdleActions', () => {
  it('keeps only declared rows, in preference order', () => {
    const anims: Record<string, AnimationRow> = {
      idle: row(0),
      pounce: row(7),
      blink: row(8),
      happy: row(6),
    };
    // Declaration order in IDLE_ACTION_CANDIDATES: blink, happy, thinking, pounce, …
    expect(availableIdleActions(anims)).toEqual(['blink', 'happy', 'pounce']);
  });

  it('falls back to the standard waving row when present', () => {
    expect(availableIdleActions({ idle: row(0), waving: row(3) })).toEqual(['waving']);
  });

  it('returns nothing for a pet with no candidate rows or no animations', () => {
    expect(availableIdleActions({ idle: row(0), 'run-left': row(2) })).toEqual([]);
    expect(availableIdleActions(undefined)).toEqual([]);
  });
});

describe('pickIdleAction', () => {
  it('indexes deterministically by the injected random fraction', () => {
    const actions = ['blink', 'happy', 'pounce'];
    expect(pickIdleAction(actions, 0)).toBe('blink');
    expect(pickIdleAction(actions, 0.5)).toBe('happy');
    expect(pickIdleAction(actions, 0.99)).toBe('pounce');
  });

  it('clamps out-of-range / corrupt randoms into bounds', () => {
    const actions = ['a', 'b'];
    expect(pickIdleAction(actions, 1)).toBe('b'); // clamped below 1
    expect(pickIdleAction(actions, Number.NaN)).toBe('a');
    expect(pickIdleAction([], 0.5)).toBeNull();
  });
});

describe('nextIdleDelayMs', () => {
  it('disables the loop for a non-positive or corrupt interval', () => {
    expect(nextIdleDelayMs(0, 0.5)).toBeNull();
    expect(nextIdleDelayMs(-2, 0.5)).toBeNull();
    expect(nextIdleDelayMs(Number.NaN, 0.5)).toBeNull();
  });

  it('centres on the interval and jitters within the band', () => {
    const base = 2 * 60_000;
    expect(nextIdleDelayMs(2, 0.5)).toBe(base); // mid random → no jitter
    expect(nextIdleDelayMs(2, 1)).toBe(Math.round(base * (1 + IDLE_JITTER_FRACTION)));
    expect(nextIdleDelayMs(2, 0)).toBe(Math.round(base * (1 - IDLE_JITTER_FRACTION)));
  });

  it('never schedules below the 1s floor', () => {
    expect(nextIdleDelayMs(0.001, 0)).toBe(1_000);
  });
});
