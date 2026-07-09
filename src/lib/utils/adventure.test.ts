import { describe, expect, it } from 'vitest';
import {
  ADVENTURE_MIN_MS,
  awayDisplayGate,
  consumeTrip,
  initialAdventureState,
  stepAdventure,
} from './adventure';

const T0 = 1_700_000_000_000;

describe('stepAdventure', () => {
  it('timestamps a newly busy session and reports no trip yet', () => {
    const s = initialAdventureState();
    const { away } = stepAdventure(s, ['a'], ['a'], T0);
    expect(away).toBe(false);
    expect(s.pending.get('a')).toBe(T0);
  });

  it('reports away once a busy session crosses the threshold', () => {
    const s = initialAdventureState();
    stepAdventure(s, ['a'], ['a'], T0);
    expect(stepAdventure(s, ['a'], ['a'], T0 + ADVENTURE_MIN_MS - 1).away).toBe(false);
    expect(stepAdventure(s, ['a'], ['a'], T0 + ADVENTURE_MIN_MS).away).toBe(true);
  });

  it('keeps the timestamp while the session waits on the user', () => {
    const s = initialAdventureState();
    stepAdventure(s, ['a'], ['a'], T0);
    // Session flips to waiting: not busy, still alive — trip survives, but the
    // pet is home (away only counts CURRENTLY busy sessions).
    const mid = stepAdventure(s, [], ['a'], T0 + ADVENTURE_MIN_MS + 1000);
    expect(mid.away).toBe(false);
    expect(s.pending.get('a')).toBe(T0);
    // Back to busy: threshold already crossed.
    expect(stepAdventure(s, ['a'], ['a'], T0 + ADVENTURE_MIN_MS + 2000).away).toBe(true);
  });

  it('drops sessions that vanish from the list entirely', () => {
    const s = initialAdventureState();
    stepAdventure(s, ['a', 'b'], ['a', 'b'], T0);
    stepAdventure(s, ['b'], ['b'], T0 + 1000);
    expect(s.pending.has('a')).toBe(false);
    expect(s.pending.get('b')).toBe(T0);
  });

  it('tracks concurrent sessions independently', () => {
    const s = initialAdventureState();
    stepAdventure(s, ['a'], ['a'], T0);
    stepAdventure(s, ['a', 'b'], ['a', 'b'], T0 + ADVENTURE_MIN_MS);
    expect(s.pending.get('b')).toBe(T0 + ADVENTURE_MIN_MS);
    expect(stepAdventure(s, ['a', 'b'], ['a', 'b'], T0 + ADVENTURE_MIN_MS).away).toBe(true);
  });

  it('ignores a non-finite clock', () => {
    const s = initialAdventureState();
    expect(stepAdventure(s, ['a'], ['a'], Number.NaN).away).toBe(false);
    expect(s.pending.size).toBe(0);
  });
});

describe('awayDisplayGate', () => {
  const calm = {
    eligible: true,
    waitingCount: 0,
    celebrating: false,
    eating: false,
    settingsOpen: false,
    voiceActive: false,
    physicsState: null,
    physicsPaused: false,
  };

  it('departs when eligible and nothing needs the pet', () => {
    expect(awayDisplayGate(calm)).toBe(true);
    expect(awayDisplayGate({ ...calm, physicsState: 'on_floor' })).toBe(true);
  });

  it('never departs without eligibility', () => {
    expect(awayDisplayGate({ ...calm, eligible: false })).toBe(false);
  });

  it('stays home for a note, a celebration, a meal, settings, or voice', () => {
    expect(awayDisplayGate({ ...calm, waitingCount: 1 })).toBe(false);
    expect(awayDisplayGate({ ...calm, celebrating: true })).toBe(false);
    expect(awayDisplayGate({ ...calm, eating: true })).toBe(false);
    expect(awayDisplayGate({ ...calm, settingsOpen: true })).toBe(false);
    expect(awayDisplayGate({ ...calm, voiceActive: true })).toBe(false);
  });

  it('a live manipulation blocks; the same state frozen by pause does not', () => {
    // Live fall/drag: wait it out.
    expect(awayDisplayGate({ ...calm, physicsState: 'falling' })).toBe(false);
    expect(awayDisplayGate({ ...calm, physicsState: 'pinched' })).toBe(false);
    // Panel expanded pauses the loop mid-'falling' and freezes it there forever —
    // the frozen state must not block (the pet could never leave with the panel
    // open before this).
    expect(awayDisplayGate({ ...calm, physicsState: 'falling', physicsPaused: true })).toBe(true);
  });
});

describe('consumeTrip', () => {
  it('returns the elapsed time and forgets the trip', () => {
    const s = initialAdventureState();
    stepAdventure(s, ['a'], ['a'], T0);
    expect(consumeTrip(s, 'a', T0 + 240_000)).toBe(240_000);
    expect(s.pending.has('a')).toBe(false);
    expect(consumeTrip(s, 'a', T0 + 240_000)).toBeNull();
  });

  it('returns null for a session it never saw busy', () => {
    const s = initialAdventureState();
    expect(consumeTrip(s, 'ghost', T0)).toBeNull();
  });

  it('clamps a clock regression to 0, never a negative trip', () => {
    const s = initialAdventureState();
    stepAdventure(s, ['a'], ['a'], T0);
    expect(consumeTrip(s, 'a', T0 - 5000)).toBe(0);
  });
});
