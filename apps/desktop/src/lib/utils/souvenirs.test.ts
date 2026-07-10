import { describe, expect, it } from 'vitest';
import {
  addSouvenir,
  LONG_TRIP_MS,
  rollSouvenir,
  SOUVENIR_CATALOG,
  sanitizeSouvenirs,
} from './souvenirs';

/** rand() stub yielding the given values in order. */
const seq = (...values: number[]) => {
  let i = 0;
  return () => values[i++] ?? 0;
};

describe('catalog shape', () => {
  it('holds 24 unique ids: 12 common, 8 rare, 4 legendary', () => {
    expect(SOUVENIR_CATALOG).toHaveLength(24);
    expect(new Set(SOUVENIR_CATALOG.map((d) => d.id)).size).toBe(24);
    const byRarity = (r: string) => SOUVENIR_CATALOG.filter((d) => d.rarity === r).length;
    expect(byRarity('common')).toBe(12);
    expect(byRarity('rare')).toBe(8);
    expect(byRarity('legendary')).toBe(4);
  });
});

describe('rollSouvenir rarity tables', () => {
  it('short trip: 78/19/3 boundaries', () => {
    expect(rollSouvenir(0, seq(0.779, 0)).rarity).toBe('common');
    expect(rollSouvenir(0, seq(0.78, 0)).rarity).toBe('rare');
    expect(rollSouvenir(0, seq(0.969, 0)).rarity).toBe('rare');
    expect(rollSouvenir(0, seq(0.97, 0)).rarity).toBe('legendary');
  });

  it('long trip (≥10 min): 60/32/8 boundaries', () => {
    expect(rollSouvenir(LONG_TRIP_MS, seq(0.599, 0)).rarity).toBe('common');
    expect(rollSouvenir(LONG_TRIP_MS, seq(0.6, 0)).rarity).toBe('rare');
    expect(rollSouvenir(LONG_TRIP_MS, seq(0.919, 0)).rarity).toBe('rare');
    expect(rollSouvenir(LONG_TRIP_MS, seq(0.92, 0)).rarity).toBe('legendary');
    // Just short of the long-trip mark still rolls the base table.
    expect(rollSouvenir(LONG_TRIP_MS - 1, seq(0.6, 0)).rarity).toBe('common');
  });

  it('second draw picks uniformly within the rolled rarity', () => {
    const commons = SOUVENIR_CATALOG.filter((d) => d.rarity === 'common');
    expect(rollSouvenir(0, seq(0, 0)).id).toBe(commons[0].id);
    expect(rollSouvenir(0, seq(0, 0.999)).id).toBe(commons[commons.length - 1].id);
    const legends = SOUVENIR_CATALOG.filter((d) => d.rarity === 'legendary');
    expect(rollSouvenir(0, seq(0.99, 0.5)).id).toBe(legends[Math.floor(0.5 * legends.length)].id);
  });

  it('a non-finite elapsed rolls the base table instead of throwing', () => {
    expect(rollSouvenir(Number.NaN, seq(0.6, 0)).rarity).toBe('common');
  });
});

describe('addSouvenir', () => {
  it('records a first find with count 1 and firstAt', () => {
    const owned = addSouvenir({}, 'cloud_fluff', 1_000);
    expect(owned.cloud_fluff).toEqual({ count: 1, firstAt: 1_000 });
  });

  it('a repeat bumps the count and keeps the original firstAt', () => {
    let owned = addSouvenir({}, 'cloud_fluff', 1_000);
    owned = addSouvenir(owned, 'cloud_fluff', 9_999);
    expect(owned.cloud_fluff).toEqual({ count: 2, firstAt: 1_000 });
  });

  it('returns a fresh record and leaves the input untouched', () => {
    const before = addSouvenir({}, 'ding_echo', 1);
    const after = addSouvenir(before, 'cloud_key', 2);
    expect(before.cloud_key).toBeUndefined();
    expect(after.ding_echo).toEqual({ count: 1, firstAt: 1 });
  });
});

describe('sanitizeSouvenirs', () => {
  it('keeps valid entries, including unknown ids (forward compat)', () => {
    const out = sanitizeSouvenirs({
      cloud_fluff: { count: 3, firstAt: 1_000 },
      from_the_future: { count: 1, firstAt: 2_000 },
    });
    expect(out.cloud_fluff).toEqual({ count: 3, firstAt: 1_000 });
    expect(out.from_the_future).toEqual({ count: 1, firstAt: 2_000 });
  });

  it('drops garbage values and floors counts to integers', () => {
    const out = sanitizeSouvenirs({
      a: { count: 0, firstAt: 1 },
      b: { count: 'many', firstAt: 1 },
      c: 42,
      d: null,
      e: { count: 2.9, firstAt: Number.POSITIVE_INFINITY },
    });
    expect(Object.keys(out)).toEqual(['e']);
    expect(out.e).toEqual({ count: 2, firstAt: 0 });
  });

  it('rejects non-object roots', () => {
    expect(Object.keys(sanitizeSouvenirs(null))).toHaveLength(0);
    expect(Object.keys(sanitizeSouvenirs([1, 2]))).toHaveLength(0);
    expect(Object.keys(sanitizeSouvenirs('shelf'))).toHaveLength(0);
  });

  it('treats a hostile __proto__ key as an ordinary own key', () => {
    const out = sanitizeSouvenirs(JSON.parse('{"__proto__":{"count":1,"firstAt":1}}'));
    expect(Object.getPrototypeOf(out)).toBeNull();
    expect(out.__proto__).toEqual({ count: 1, firstAt: 1 });
  });
});
