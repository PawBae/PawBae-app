import { describe, expect, it } from 'vitest';
import { deepFreeze } from './validation';

describe('deepFreeze', () => {
  it('returns a recursively frozen clone without freezing or aliasing caller input', () => {
    const input = {
      nested: { value: 1 },
      items: [{ id: 'first' }],
    };

    const frozen = deepFreeze(input);

    expect(frozen).toEqual(input);
    expect(frozen).not.toBe(input);
    expect(frozen.nested).not.toBe(input.nested);
    expect(frozen.items).not.toBe(input.items);
    expect(frozen.items[0]).not.toBe(input.items[0]);
    expect(Object.isFrozen(frozen)).toBe(true);
    expect(Object.isFrozen(frozen.nested)).toBe(true);
    expect(Object.isFrozen(frozen.items)).toBe(true);
    expect(Object.isFrozen(frozen.items[0])).toBe(true);
    expect(Object.isFrozen(input)).toBe(false);
    expect(Object.isFrozen(input.nested)).toBe(false);
    expect(Object.isFrozen(input.items)).toBe(false);
  });
});
