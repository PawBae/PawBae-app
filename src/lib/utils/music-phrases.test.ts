import { describe, expect, it } from 'vitest';
import { MUSIC_PHRASE_KEYS, pickPhraseIndex } from './music-phrases';

describe('MUSIC_PHRASE_KEYS', () => {
  it('has at least two distinct phrases (so anti-repeat is meaningful)', () => {
    expect(MUSIC_PHRASE_KEYS.length).toBeGreaterThanOrEqual(2);
    expect(new Set(MUSIC_PHRASE_KEYS).size).toBe(MUSIC_PHRASE_KEYS.length);
  });
});

describe('pickPhraseIndex', () => {
  it('returns -1 when there are no phrases', () => {
    expect(pickPhraseIndex(0, -1, 0.5)).toBe(-1);
  });

  it('always returns 0 for a single phrase, even if it was last', () => {
    expect(pickPhraseIndex(1, 0, 0.99)).toBe(0);
  });

  it('is deterministic for a given rand', () => {
    expect(pickPhraseIndex(5, -1, 0)).toBe(0);
    expect(pickPhraseIndex(5, -1, 0.999999)).toBe(4);
    expect(pickPhraseIndex(5, -1, 0.5)).toBe(2);
  });

  it('never repeats the last index', () => {
    // rand 0 would pick index 0; lastIndex 0 forces a bump to 1.
    expect(pickPhraseIndex(5, 0, 0)).toBe(1);
    // rand 0.999 picks 4; lastIndex 4 bumps to 0 (wraps).
    expect(pickPhraseIndex(5, 4, 0.999999)).toBe(0);
  });

  it('tolerates a corrupt rand without throwing or going out of range', () => {
    const idx = pickPhraseIndex(5, -1, Number.NaN);
    expect(idx).toBeGreaterThanOrEqual(0);
    expect(idx).toBeLessThan(5);
  });

  it('stays in range across the whole rand interval', () => {
    for (let i = 0; i < 100; i++) {
      const idx = pickPhraseIndex(MUSIC_PHRASE_KEYS.length, -1, i / 100);
      expect(idx).toBeGreaterThanOrEqual(0);
      expect(idx).toBeLessThan(MUSIC_PHRASE_KEYS.length);
    }
  });
});
