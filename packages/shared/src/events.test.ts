import { describe, expect, it } from 'vitest';
import {
  AGENT_SOURCES,
  EVENT_KINDS,
  EVENT_RARITIES,
  createEggHatchedEvent,
  createEvent,
  createSouvenirFoundEvent,
  createStreakMilestoneEvent,
  createTaskCompletedEvent,
} from './events';

describe('event constructors', () => {
  it('constructs frozen, minimal events for every dictionary entry', () => {
    const events = [
      createTaskCompletedEvent({ source: 'cc' }),
      createEggHatchedEvent({ rarity: 'rare' }),
      createSouvenirFoundEvent({ rarity: 'legendary' }),
      createStreakMilestoneEvent({ days: 3650 }),
    ];

    expect(events).toEqual([
      { kind: 'task_completed', params: { source: 'cc' } },
      { kind: 'egg_hatched', params: { rarity: 'rare' } },
      { kind: 'souvenir_found', params: { rarity: 'legendary' } },
      { kind: 'streak_milestone', params: { days: 3650 } },
    ]);
    for (const event of events) {
      expect(Object.isFrozen(event)).toBe(true);
      expect(Object.isFrozen(event.params)).toBe(true);
    }
  });

  it('keeps its accepted dictionaries immutable', () => {
    expect(AGENT_SOURCES).toEqual(['cc', 'codex', 'cursor']);
    expect(EVENT_RARITIES).toEqual(['common', 'rare', 'legendary']);
    expect(EVENT_KINDS).toEqual([
      'task_completed',
      'egg_hatched',
      'souvenir_found',
      'streak_milestone',
    ]);
    expect(Object.isFrozen(AGENT_SOURCES)).toBe(true);
    expect(Object.isFrozen(EVENT_RARITIES)).toBe(true);
    expect(Object.isFrozen(EVENT_KINDS)).toBe(true);
  });

  it('rejects free text and every other unknown parameter key', () => {
    expect(() =>
      createTaskCompletedEvent({ source: 'cc', prompt: 'private user text' }),
    ).toThrowError(/task_completed params.*unknown key.*prompt/i);
    expect(() => createEggHatchedEvent({ rarity: 'rare', name: 'secret' })).toThrowError(
      /egg_hatched params.*unknown key.*name/i,
    );
    expect(() => createStreakMilestoneEvent({ days: 7, reason: 'private' })).toThrowError(
      /streak_milestone params.*unknown key.*reason/i,
    );
  });

  it('rejects missing, malformed, and out-of-dictionary enum parameters', () => {
    for (const value of [
      null,
      [],
      new Date(),
      'cc',
      { source: 'claude' },
      {},
      { source: 1 },
    ]) {
      expect(() => createTaskCompletedEvent(value)).toThrow(TypeError);
    }
    for (const value of [{ rarity: 'epic' }, { rarity: '' }, { rarity: 1 }]) {
      expect(() => createSouvenirFoundEvent(value)).toThrow(TypeError);
    }
  });

  it('accepts only integral streak days from 1 through 3650', () => {
    expect(createStreakMilestoneEvent({ days: 1 }).params.days).toBe(1);
    for (const days of [0, -1, 1.5, 3651, Number.NaN, Number.POSITIVE_INFINITY, '7']) {
      expect(() => createStreakMilestoneEvent({ days })).toThrowError(/days.*integer.*1.*3650/i);
    }
  });

  it('dispatches unknown runtime input through the same strict constructors', () => {
    expect(createEvent('task_completed', { source: 'cursor' })).toEqual(
      createTaskCompletedEvent({ source: 'cursor' }),
    );
    expect(createEvent('egg_hatched', { rarity: 'common' })).toEqual(
      createEggHatchedEvent({ rarity: 'common' }),
    );
    expect(createEvent('souvenir_found', { rarity: 'rare' })).toEqual(
      createSouvenirFoundEvent({ rarity: 'rare' }),
    );
    expect(createEvent('streak_milestone', { days: 30 })).toEqual(
      createStreakMilestoneEvent({ days: 30 }),
    );
    expect(() => createEvent('task_failed', {})).toThrowError(/unknown event kind/i);
    expect(() => createEvent(42, {})).toThrowError(/event kind/i);
  });

  it('does not reflect an unknown caller-supplied kind in its error', () => {
    const secretKind = 'private prompt text';
    let caught: unknown;

    try {
      createEvent(secretKind, {});
    } catch (error) {
      caught = error;
    }

    expect(caught).toBeInstanceOf(TypeError);
    expect((caught as Error).message).toBe('unknown event kind');
    expect((caught as Error).message).not.toContain(secretKind);
  });
});
