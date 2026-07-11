import { describe, expect, expectTypeOf, it } from 'vitest';
import {
  MEMORY_DURATION_BUCKETS,
  MEMORY_PARAMETER_LOCALIZATIONS,
  MEMORY_TEMPLATE_FIXTURES,
  MEMORY_TEMPLATE_KEYS,
  MEMORY_TEMPLATE_LOCALIZATIONS,
  MEMORY_TIMES_OF_DAY,
  createMemoryTemplatePayload,
  type MemoryTemplateKey,
  type MemoryTemplatePayload,
} from './memories';

describe('memory template contracts', () => {
  it('freezes every safe memory dictionary', () => {
    expect(MEMORY_TEMPLATE_KEYS).toEqual([
      'played_together',
      'worked_together',
      'celebrated_completion',
      'shared_snack',
    ]);
    expect(MEMORY_DURATION_BUCKETS).toEqual(['short', 'full']);
    expect(MEMORY_TIMES_OF_DAY).toEqual(['morning', 'afternoon', 'evening', 'night']);
    expect(Object.isFrozen(MEMORY_TEMPLATE_KEYS)).toBe(true);
    expect(Object.isFrozen(MEMORY_DURATION_BUCKETS)).toBe(true);
    expect(Object.isFrozen(MEMORY_TIMES_OF_DAY)).toBe(true);
    expectTypeOf<MemoryTemplateKey>().toEqualTypeOf<
      'played_together' | 'worked_together' | 'celebrated_completion' | 'shared_snack'
    >();
  });

  it('constructs a deeply frozen exact-key payload with bounded safe parameters', () => {
    const payload = createMemoryTemplatePayload('played_together', {
      durationBucket: 'short',
      timeOfDay: 'morning',
      interactionCount: 0,
    });
    expect(payload).toEqual({
      templateKey: 'played_together',
      params: { durationBucket: 'short', timeOfDay: 'morning', interactionCount: 0 },
    });
    expect(Object.isFrozen(payload)).toBe(true);
    expect(Object.isFrozen(payload.params)).toBe(true);
    expectTypeOf(payload).toMatchTypeOf<MemoryTemplatePayload>();
  });

  it('rejects unknown templates, free text, unknown keys, and invalid numeric values', () => {
    const valid = { durationBucket: 'full', timeOfDay: 'night', interactionCount: 100 };
    expect(() => createMemoryTemplatePayload('wrote_code', valid)).toThrowError(/template key/i);
    expect(() =>
      createMemoryTemplatePayload('worked_together', { ...valid, summary: 'private code' }),
    ).toThrowError(/memory params.*unknown key.*summary/i);
    expect(() =>
      createMemoryTemplatePayload('worked_together', { ...valid, durationBucket: 'long' }),
    ).toThrowError(/durationBucket/i);
    expect(() =>
      createMemoryTemplatePayload('worked_together', { ...valid, timeOfDay: 'dawn' }),
    ).toThrowError(/timeOfDay/i);
    for (const interactionCount of [-1, 1.5, 101, Number.NaN, '3']) {
      expect(() =>
        createMemoryTemplatePayload('worked_together', { ...valid, interactionCount }),
      ).toThrowError(/interactionCount/i);
    }
  });

  it('provides one deeply frozen fixture per template for downstream clients', () => {
    expect(MEMORY_TEMPLATE_FIXTURES.map((fixture) => fixture.templateKey)).toEqual(
      MEMORY_TEMPLATE_KEYS,
    );
    expect(MEMORY_TEMPLATE_FIXTURES).toHaveLength(4);
    expect(Object.isFrozen(MEMORY_TEMPLATE_FIXTURES)).toBe(true);
    for (const fixture of MEMORY_TEMPLATE_FIXTURES) {
      expect(Object.isFrozen(fixture)).toBe(true);
      expect(Object.isFrozen(fixture.params)).toBe(true);
      expect(() => createMemoryTemplatePayload(fixture.templateKey, fixture.params)).not.toThrow();
    }
  });

  it('ships complete, deeply frozen English and Chinese localization data', () => {
    for (const locale of ['en', 'zh'] as const) {
      expect(Object.keys(MEMORY_TEMPLATE_LOCALIZATIONS[locale])).toEqual(MEMORY_TEMPLATE_KEYS);
      expect(Object.isFrozen(MEMORY_TEMPLATE_LOCALIZATIONS[locale])).toBe(true);
      expect(Object.isFrozen(MEMORY_PARAMETER_LOCALIZATIONS[locale])).toBe(true);
      for (const key of MEMORY_TEMPLATE_KEYS) {
        const copy = MEMORY_TEMPLATE_LOCALIZATIONS[locale][key];
        expect(copy.title.length).toBeGreaterThan(0);
        expect(copy.body).toContain('{durationBucket}');
        expect(copy.body).toContain('{timeOfDay}');
        expect(copy.body).toContain('{interactionCount}');
        expect(Object.isFrozen(copy)).toBe(true);
      }
      expect(Object.keys(MEMORY_PARAMETER_LOCALIZATIONS[locale].durationBucket)).toEqual(
        MEMORY_DURATION_BUCKETS,
      );
      expect(Object.keys(MEMORY_PARAMETER_LOCALIZATIONS[locale].timeOfDay)).toEqual(
        MEMORY_TIMES_OF_DAY,
      );
    }
    expect(Object.isFrozen(MEMORY_TEMPLATE_LOCALIZATIONS)).toBe(true);
    expect(Object.isFrozen(MEMORY_PARAMETER_LOCALIZATIONS)).toBe(true);
  });
});
