import { describe, expect, it } from 'vitest';
import * as C from './constants';

describe('physics constants', () => {
  it('TICK_MS is a positive integer under 100ms', () => {
    expect(C.TICK_MS).toBeGreaterThan(0);
    expect(C.TICK_MS).toBeLessThan(100);
    expect(Number.isInteger(C.TICK_MS)).toBe(true);
  });

  it('GRAVITY is positive', () => {
    expect(C.GRAVITY).toBeGreaterThan(0);
  });

  it('WALK_SPEED and CLIMB_SPEED are positive', () => {
    expect(C.WALK_SPEED).toBeGreaterThan(0);
    expect(C.CLIMB_SPEED).toBeGreaterThan(0);
  });

  it('BOUNCE_DAMPING is between 0 and 1', () => {
    expect(C.BOUNCE_DAMPING).toBeGreaterThan(0);
    expect(C.BOUNCE_DAMPING).toBeLessThan(1);
  });

  it('MAX_THROW_SPEED is positive', () => {
    expect(C.MAX_THROW_SPEED).toBeGreaterThan(0);
  });

  it('TERMINAL_VY is greater than GRAVITY', () => {
    expect(C.TERMINAL_VY).toBeGreaterThan(C.GRAVITY);
  });
});
