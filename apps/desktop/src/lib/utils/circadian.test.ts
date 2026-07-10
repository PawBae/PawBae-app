import { describe, expect, it } from 'vitest';
import { actionWeight, type DayPart, dayPartFor, toneOf } from './circadian';

describe('dayPartFor', () => {
  it('buckets representative hours', () => {
    expect(dayPartFor(23)).toBe('night');
    expect(dayPartFor(2)).toBe('night');
    expect(dayPartFor(7)).toBe('morning');
    expect(dayPartFor(14)).toBe('day');
    expect(dayPartFor(20)).toBe('evening');
  });

  it('handles every boundary exactly', () => {
    expect(dayPartFor(5)).toBe('morning'); // night ends
    expect(dayPartFor(4)).toBe('night');
    expect(dayPartFor(11)).toBe('day'); // morning ends
    expect(dayPartFor(10)).toBe('morning');
    expect(dayPartFor(18)).toBe('evening'); // day ends
    expect(dayPartFor(17)).toBe('day');
    expect(dayPartFor(22)).toBe('night'); // evening ends
    expect(dayPartFor(21)).toBe('evening');
  });

  it('falls back to the neutral day part for a corrupt hour', () => {
    expect(dayPartFor(Number.NaN)).toBe('day');
    expect(dayPartFor(Number.POSITIVE_INFINITY)).toBe('day');
  });
});

describe('toneOf', () => {
  it('classifies calm, lively and neutral rows', () => {
    expect(toneOf('blink')).toBe('calm');
    expect(toneOf('sleep')).toBe('calm');
    expect(toneOf('pounce')).toBe('lively');
    expect(toneOf('dance')).toBe('lively');
    expect(toneOf('idle')).toBe('neutral');
    expect(toneOf('whatever')).toBe('neutral');
  });
});

describe('actionWeight', () => {
  it('favors calm at night and lively midday, evenly otherwise', () => {
    expect(actionWeight('blink', 'night')).toBe(3);
    expect(actionWeight('pounce', 'night')).toBe(1);
    expect(actionWeight('pounce', 'day')).toBe(3);
    expect(actionWeight('blink', 'day')).toBe(1);
    for (const part of ['morning', 'evening'] as DayPart[]) {
      expect(actionWeight('blink', part)).toBe(2);
      expect(actionWeight('pounce', part)).toBe(2);
    }
  });

  it('keeps neutral rows mid-weight and never excludes any row', () => {
    for (const part of ['night', 'morning', 'day', 'evening'] as DayPart[]) {
      expect(actionWeight('idle', part)).toBe(2);
      expect(actionWeight('blink', part)).toBeGreaterThanOrEqual(1);
      expect(actionWeight('pounce', part)).toBeGreaterThanOrEqual(1);
    }
  });
});
