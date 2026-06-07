// Minimal one-shot input-reaction state machine (PawBae Phase 1-B).
//
// Pure logic, zero Svelte/Tauri imports — mirrors the physics `state-machine.ts`
// precedent so it is unit-testable without mounting a component or running on macOS.
// The Svelte layer (MascotView) feeds `user-input` events in and reverts on a timer.

import type { UserInputEvent } from '../types';

export type ReactionKind = 'keyboard' | 'mouse';

/** Sprite-codex animation keys the reaction overlay renders. Defined in pet.json. */
export const REACTION_SPRITE_KEYBOARD = 'react-keyboard';
export const REACTION_SPRITE_MOUSE = 'react-mouse';

/** What the pet is doing right now — a reaction must not start over an active beat. */
export interface ReactionContext {
  /** true while dragging/throwing/hovering/headpat/non-resting physics. */
  busy: boolean;
}

export interface MutableReactionState {
  playing: boolean;
  kind: ReactionKind | null;
}

export function initialReactionState(): MutableReactionState {
  return { playing: false, kind: null };
}

/**
 * Try to start a reaction for an incoming `user-input` event. Mutates `s` in place
 * (mirrors the physics `step()` convention). Returns true iff a reaction started.
 *
 * - Coalesces: ignores the event while a beat is already playing (one beat per window).
 * - Guards: ignores the event while the pet is busy (drag/throw/hover/headpat/physics).
 */
export function requestReaction(
  s: MutableReactionState,
  ev: UserInputEvent,
  ctx: ReactionContext,
): boolean {
  if (s.playing) return false;
  if (ctx.busy) return false;
  s.playing = true;
  s.kind = ev.kind;
  return true;
}

/** End the current beat and return to the base state. */
export function endReaction(s: MutableReactionState): void {
  s.playing = false;
  s.kind = null;
}

/** Overlay sprite name to render, or null when the base state should show. */
export function reactionSpriteFor(s: MutableReactionState): string | null {
  if (!s.playing || !s.kind) return null;
  return s.kind === 'keyboard' ? REACTION_SPRITE_KEYBOARD : REACTION_SPRITE_MOUSE;
}
