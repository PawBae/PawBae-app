// Idle micro-actions (Phase 5 动画丰富化). Pure helpers behind the long-dormant
// "Random Action Interval" setting (petIdleIntervalMin) — the mascot occasionally plays
// a short row from the pet's OWN spritesheet while truly idle, so each character uses
// whatever personality rows it ships (yoonie blinks/pounces; standard pets wave).
import { actionWeight, type DayPart } from './circadian';
import type { AnimationRow } from './codex-pet';

// Candidate idle rows, filtered by what the pet actually declares. Mix of lively and
// calm tones (see circadian.toneOf) so time-of-day weighting has something to tilt
// between on rich pets. 'jumping' is last — hover already plays it constantly.
export const IDLE_ACTION_CANDIDATES: readonly string[] = [
  'blink',
  'happy',
  'thinking',
  'pounce',
  'yawn',
  'sleep',
  'rest',
  'dance',
  'spin',
  'peek',
  'waving',
  'review',
  'jumping',
] as const;

/** How long a non-one-shot idle action plays before reverting to the base state. */
export const IDLE_ACTION_MS = 1500;

/** Jitter band around the configured interval so the action never feels metronomic. */
export const IDLE_JITTER_FRACTION = 0.3;

export function availableIdleActions(
  animations: Record<string, AnimationRow> | undefined,
): string[] {
  if (!animations) return [];
  return IDLE_ACTION_CANDIDATES.filter((name) => animations[name] != null);
}

/**
 * Pick the next action uniformly. `rand` ∈ [0, 1) is injected for determinism in tests;
 * the caller passes Math.random(). Returns null when the pet has no usable rows.
 */
export function pickIdleAction(actions: readonly string[], rand: number): string | null {
  if (actions.length === 0) return null;
  const r = Number.isFinite(rand) ? Math.min(Math.max(rand, 0), 0.999999) : 0;
  return actions[Math.floor(r * actions.length)];
}

/**
 * Pick the next action with time-of-day bias: night favors calm rows, midday favors
 * lively ones (see circadian.actionWeight). Falls back to a uniform pick when the day
 * part adds no usable signal. Deterministic for a given `rand` ∈ [0, 1).
 */
export function pickIdleActionFor(
  actions: readonly string[],
  part: DayPart,
  rand: number,
): string | null {
  if (actions.length === 0) return null;
  const weights = actions.map((a) => actionWeight(a, part));
  const total = weights.reduce((sum, w) => sum + w, 0);
  if (total <= 0) return pickIdleAction(actions, rand); // defensive — weights are ≥1 today
  const r = (Number.isFinite(rand) ? Math.min(Math.max(rand, 0), 0.999999) : 0) * total;
  let acc = 0;
  for (let i = 0; i < actions.length; i++) {
    acc += weights[i];
    if (r < acc) return actions[i];
  }
  return actions[actions.length - 1]; // float rounding guard
}

/**
 * Delay until the next idle action: the configured interval ±IDLE_JITTER_FRACTION.
 * A non-positive or corrupt interval disables the feature (returns null).
 */
export function nextIdleDelayMs(intervalMin: number, rand: number): number | null {
  if (!Number.isFinite(intervalMin) || intervalMin <= 0) return null;
  const base = intervalMin * 60_000;
  const r = Number.isFinite(rand) ? Math.min(Math.max(rand, 0), 1) : 0.5;
  const jitter = (r * 2 - 1) * IDLE_JITTER_FRACTION; // -0.3 .. +0.3
  return Math.max(1_000, Math.round(base * (1 + jitter)));
}
