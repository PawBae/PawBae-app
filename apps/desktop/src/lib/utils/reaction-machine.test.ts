import { describe, expect, it } from 'vitest';
import type { UserInputEvent } from '../types';
import {
  endReaction,
  initialReactionState,
  REACTION_SPRITE_KEYBOARD,
  REACTION_SPRITE_MOUSE,
  type ReactionKind,
  reactionSpriteFor,
  requestReaction,
} from './reaction-machine';

function ev(kind: ReactionKind, count = 1): UserInputEvent {
  return { kind, count, at: 0 };
}

describe('initialReactionState', () => {
  it('starts idle with no kind', () => {
    const s = initialReactionState();
    expect(s.playing).toBe(false);
    expect(s.kind).toBeNull();
  });
});

describe('requestReaction', () => {
  it('starts a keyboard reaction when idle and not busy', () => {
    const s = initialReactionState();
    const started = requestReaction(s, ev('keyboard'), { busy: false });
    expect(started).toBe(true);
    expect(s.playing).toBe(true);
    expect(s.kind).toBe('keyboard');
  });

  it('starts a mouse reaction when idle and not busy', () => {
    const s = initialReactionState();
    const started = requestReaction(s, ev('mouse'), { busy: false });
    expect(started).toBe(true);
    expect(s.kind).toBe('mouse');
  });

  it('coalesces: ignores a new event while a reaction is already playing', () => {
    const s = initialReactionState();
    requestReaction(s, ev('keyboard'), { busy: false });
    const started = requestReaction(s, ev('mouse'), { busy: false });
    expect(started).toBe(false);
    expect(s.kind).toBe('keyboard'); // unchanged — one beat per non-overlapping window
  });

  it('guards: does not start while busy (drag/hover/headpat/physics)', () => {
    const s = initialReactionState();
    const started = requestReaction(s, ev('keyboard'), { busy: true });
    expect(started).toBe(false);
    expect(s.playing).toBe(false);
    expect(s.kind).toBeNull();
  });

  it('can re-trigger after the previous reaction ended', () => {
    const s = initialReactionState();
    requestReaction(s, ev('keyboard'), { busy: false });
    endReaction(s);
    const started = requestReaction(s, ev('keyboard'), { busy: false });
    expect(started).toBe(true);
    expect(s.playing).toBe(true);
  });
});

describe('endReaction', () => {
  it('returns to base state (clears playing and kind)', () => {
    const s = initialReactionState();
    requestReaction(s, ev('mouse'), { busy: false });
    endReaction(s);
    expect(s.playing).toBe(false);
    expect(s.kind).toBeNull();
    expect(reactionSpriteFor(s)).toBeNull();
  });
});

describe('reactionSpriteFor', () => {
  it('maps a playing keyboard reaction to the keyboard sprite', () => {
    const s = initialReactionState();
    requestReaction(s, ev('keyboard'), { busy: false });
    expect(reactionSpriteFor(s)).toBe(REACTION_SPRITE_KEYBOARD);
  });

  it('maps a playing mouse reaction to the mouse sprite', () => {
    const s = initialReactionState();
    requestReaction(s, ev('mouse'), { busy: false });
    expect(reactionSpriteFor(s)).toBe(REACTION_SPRITE_MOUSE);
  });

  it('returns null when idle (base state renders)', () => {
    const s = initialReactionState();
    expect(reactionSpriteFor(s)).toBeNull();
  });
});
