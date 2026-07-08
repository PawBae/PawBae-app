import { describe, expect, it } from 'vitest';
import { type AnimationRow, mealSpriteFor } from './codex-pet';

const row = (r: number): AnimationRow => ({ row: r, frames: 4 });

describe('mealSpriteFor', () => {
  it('prefers a dedicated eat row when the sheet declares one', () => {
    expect(mealSpriteFor({ eat: row(5), happy: row(6) })).toBe('eat');
  });

  it('falls back to the happy row when eat is missing (yoonie-style sheets)', () => {
    expect(mealSpriteFor({ idle: row(0), happy: row(6) })).toBe('happy');
  });

  it('returns null when neither row exists — the meal stays a stat-only event', () => {
    expect(mealSpriteFor({ idle: row(0), running: row(1) })).toBeNull();
    expect(mealSpriteFor(undefined)).toBeNull();
    expect(mealSpriteFor(null)).toBeNull();
  });
});
