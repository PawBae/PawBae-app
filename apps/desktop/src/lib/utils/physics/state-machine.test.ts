import { describe, expect, it } from 'vitest';
import { initialState, spriteNameFor } from './state-machine';

describe('initialState', () => {
  it('starts in falling state', () => {
    const s = initialState();
    expect(s.state).toBe('falling');
  });

  it('starts with zero velocity', () => {
    const s = initialState();
    expect(s.vx).toBe(0);
    expect(s.vy).toBe(0);
  });

  it('starts on screen surface', () => {
    const s = initialState();
    expect(s.surface).toBe('screen');
  });

  it('starts with no window attachment', () => {
    const s = initialState();
    expect(s.surfaceWindowId).toBeNull();
  });
});

describe('spriteNameFor', () => {
  it('returns falling for falling state', () => {
    const s = initialState();
    expect(spriteNameFor(s)).toBe('falling');
  });

  it('returns idle for on_floor with zero velocity', () => {
    const s = initialState();
    s.state = 'on_floor';
    s.vx = 0;
    expect(spriteNameFor(s)).toBe('idle');
  });

  it('returns run-right for on_floor moving right', () => {
    const s = initialState();
    s.state = 'on_floor';
    s.vx = 2;
    expect(spriteNameFor(s)).toBe('run-right');
  });

  it('returns run-left for on_floor moving left', () => {
    const s = initialState();
    s.state = 'on_floor';
    s.vx = -2;
    expect(spriteNameFor(s)).toBe('run-left');
  });

  it('returns bouncing for bouncing state', () => {
    const s = initialState();
    s.state = 'bouncing';
    expect(spriteNameFor(s)).toBe('bouncing');
  });

  it('returns waiting for pinched state', () => {
    const s = initialState();
    s.state = 'pinched';
    expect(spriteNameFor(s)).toBe('waiting');
  });

  it('returns grab-wall for on_wall early ticks', () => {
    const s = initialState();
    s.state = 'on_wall';
    s.facing = 1;
    s.ticksInState = 0;
    expect(spriteNameFor(s)).toBe('grab-wall');
  });

  it('returns grab-wall-flipped for on_wall facing left', () => {
    const s = initialState();
    s.state = 'on_wall';
    s.facing = -1;
    s.ticksInState = 0;
    expect(spriteNameFor(s)).toBe('grab-wall-flipped');
  });
});
