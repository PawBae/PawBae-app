import { describe, expect, it } from 'vitest';
import { visitInteractionFor } from './visit-stage';

describe('visitInteractionFor', () => {
  const base = {
    leaseId: 'lease-stable',
    localStatus: 'idle' as const,
    guestStatus: 'idle' as const,
    timeBucket: 42,
    reducedMotion: false,
  };

  it('is deterministic for a lease and time bucket', () => {
    expect(visitInteractionFor(base)).toBe(visitInteractionFor({ ...base }));
  });

  it('rests for reduced motion or an offline pet', () => {
    expect(visitInteractionFor({ ...base, reducedMotion: true })).toBe('rest');
    expect(visitInteractionFor({ ...base, guestStatus: 'offline' })).toBe('rest');
  });

  it('uses calm shared actions for active agent states', () => {
    expect(visitInteractionFor({ ...base, localStatus: 'working' })).toBe('side-by-side');
    expect(visitInteractionFor({ ...base, guestStatus: 'waiting' })).toBe('nose-touch');
  });
});
