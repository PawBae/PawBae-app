// Time-of-day pet behavior (Phase 5 动画丰富化: 时间段状态/深夜犯困). Pure helpers — the
// caller passes the current hour (0–23) so tests need no clock. Day part biases which
// idle micro-action the pet picks: calm rows at night, lively rows midday. It only
// reweights what the pet ALREADY shows (see idle-actions feature detection); it never
// invents a row a pet lacks.

export type DayPart = 'night' | 'morning' | 'day' | 'evening';

/**
 * Bucket an hour into a day part. Boundaries: night 22:00–04:59, morning 05:00–10:59,
 * day 11:00–17:59, evening 18:00–21:59. A corrupt/out-of-range hour falls back to 'day'
 * (the neutral part), so a bad clock can never bias the pet oddly.
 */
export function dayPartFor(hour: number): DayPart {
  if (!Number.isFinite(hour)) return 'day';
  const h = Math.floor(hour);
  if (h >= 22 || h < 5) return 'night';
  if (h < 11) return 'morning';
  if (h < 18) return 'day';
  return 'evening';
}

// Tone of each known idle row. Rows not listed are neutral. These names are matched
// against whatever the pet declares — unknown pets simply contribute neutrals.
const CALM_ACTIONS: ReadonlySet<string> = new Set([
  'blink',
  'thinking',
  'review',
  'reading',
  'sleep',
  'rest',
  'float',
  'yawn',
  'peek',
]);

const LIVELY_ACTIONS: ReadonlySet<string> = new Set([
  'happy',
  'pounce',
  'waving',
  'jumping',
  'dance',
  'spin',
]);

export type ActionTone = 'calm' | 'lively' | 'neutral';

export function toneOf(action: string): ActionTone {
  if (CALM_ACTIONS.has(action)) return 'calm';
  if (LIVELY_ACTIONS.has(action)) return 'lively';
  return 'neutral';
}

/**
 * Selection weight for an action given the day part. Night favors calm (3:1), midday
 * favors lively (3:1); morning/evening are even. Weights are always ≥1 so any available
 * row can still be picked — circadian tilts the odds, it never excludes.
 */
export function actionWeight(action: string, part: DayPart): number {
  const tone = toneOf(action);
  if (part === 'night') return tone === 'calm' ? 3 : tone === 'lively' ? 1 : 2;
  if (part === 'day') return tone === 'lively' ? 3 : tone === 'calm' ? 1 : 2;
  return 2; // morning / evening: no bias
}
