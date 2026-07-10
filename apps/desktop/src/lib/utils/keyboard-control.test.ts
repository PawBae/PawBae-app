import { describe, expect, it } from 'vitest';
import { KEYBOARD_MOVE_FAST_STEP, KEYBOARD_MOVE_STEP, keyboardMoveDelta } from './keyboard-control';

describe('keyboardMoveDelta', () => {
  it('maps WASD to mascot movement deltas', () => {
    expect(keyboardMoveDelta({ key: 'w' })).toEqual({ dx: 0, dy: -KEYBOARD_MOVE_STEP });
    expect(keyboardMoveDelta({ key: 'A' })).toEqual({ dx: -KEYBOARD_MOVE_STEP, dy: 0 });
    expect(keyboardMoveDelta({ key: 's' })).toEqual({ dx: 0, dy: KEYBOARD_MOVE_STEP });
    expect(keyboardMoveDelta({ key: 'D' })).toEqual({ dx: KEYBOARD_MOVE_STEP, dy: 0 });
  });

  it('maps arrow keys to the same movement deltas', () => {
    expect(keyboardMoveDelta({ key: 'ArrowUp' })).toEqual({ dx: 0, dy: -KEYBOARD_MOVE_STEP });
    expect(keyboardMoveDelta({ key: 'ArrowLeft' })).toEqual({ dx: -KEYBOARD_MOVE_STEP, dy: 0 });
    expect(keyboardMoveDelta({ key: 'ArrowDown' })).toEqual({ dx: 0, dy: KEYBOARD_MOVE_STEP });
    expect(keyboardMoveDelta({ key: 'ArrowRight' })).toEqual({ dx: KEYBOARD_MOVE_STEP, dy: 0 });
  });

  it('uses the fast step while shift is held', () => {
    expect(keyboardMoveDelta({ key: 'd', shiftKey: true })).toEqual({
      dx: KEYBOARD_MOVE_FAST_STEP,
      dy: 0,
    });
  });

  it('ignores shortcut and composing key events', () => {
    expect(keyboardMoveDelta({ key: 'w', metaKey: true })).toBeNull();
    expect(keyboardMoveDelta({ key: 'w', ctrlKey: true })).toBeNull();
    expect(keyboardMoveDelta({ key: 'w', altKey: true })).toBeNull();
    expect(keyboardMoveDelta({ key: 'w', isComposing: true })).toBeNull();
  });

  it('ignores unrelated keys', () => {
    expect(keyboardMoveDelta({ key: 'Enter' })).toBeNull();
  });
});
