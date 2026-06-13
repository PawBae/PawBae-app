// Idle micro-actions (Phase 5 动画丰富化). Pure helpers behind the long-dormant
// "Random Action Interval" setting (petIdleIntervalMin) — the mascot occasionally plays
// a short row from the pet's OWN spritesheet while truly idle, so each character uses
// whatever personality rows it ships (yoonie blinks/pounces; standard pets wave).
import type { AnimationRow } from './codex-pet';

// Preference order, filtered by what the pet actually declares. Rich manifests
// (yoonie-style) hit the first group; STANDARD_ANIMATION_ROWS pets hit the second.
// 'jumping' is deliberately last — hover already plays it constantly.
export const IDLE_ACTION_CANDIDATES: readonly string[] = [
  'blink',
  'happy',
  'thinking',
  'pounce',
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
 * Pick the next action. `rand` ∈ [0, 1) is injected for determinism in tests; the
 * caller passes Math.random(). Returns null when the pet has no usable rows.
 */
export function pickIdleAction(actions: readonly string[], rand: number): string | null {
  if (actions.length === 0) return null;
  const r = Number.isFinite(rand) ? Math.min(Math.max(rand, 0), 0.999999) : 0;
  return actions[Math.floor(r * actions.length)];
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
