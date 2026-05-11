// Pet physics loop.
//
// Drives ShimejiEE-style behaviour for pets that opt in via
// `pet.physics.enabled` in their pet.json — the pet walks along the
// floor, climbs the side walls, traverses the ceiling, and falls under
// gravity when thrown. State machine is independent of the existing
// PetAction enum so we don't disrupt the pet-mode (large-mascot)
// peek/walkout/codex flows that share the same window.
//
// macOS only. The physics tick reads the current mascot origin and
// monitor rect via Tauri commands and applies position deltas via
// `move_mini_by(dx, dy)` in top-down coords.

import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import type { CodexPet } from './codexPet'
import {
  detectEdges,
  invalidateMonitorCache,
  invalidateFloorCache,
  invalidateActiveWindowCache,
  measureSpriteAnchorsCSS,
  setRuntimeSpritePadCSS,
  resetRuntimeSpritePadCSS,
  type EdgeState,
} from './edgeDetect'

export type PhysicsState =
  | 'on_floor'
  | 'on_wall'
  | 'on_ceiling'
  | 'falling'
  | 'jumping'
  | 'bouncing'
  | 'pinched'

export interface PhysicsHandle {
  // Called by the drag-end emitter. Switches the state machine into
  // `falling` with an initial velocity.
  beginThrow: (vx: number, vy: number) => void
  // Suspend the physics loop without unmounting (useful when the panel
  // expands and we want the mascot to freeze at its current spot).
  setPaused: (paused: boolean) => void
  // Called externally when the OS reports a drag start, so the loop
  // pauses position updates while NSEvent moves the window.
  setPinched: (pinched: boolean) => void
  // For the sprite render pipeline to derive the right animation key.
  getSpriteAnimationName: () => string
  getPhysicsState: () => PhysicsState
  // Reactive — re-renders the consumer when the sprite name changes,
  // so SpritePet/MiniPetMascot can swap animations smoothly. Falls
  // back to 'idle' when physics is disabled.
  spriteName: string
  physicsState: PhysicsState
}

interface PhysicsOptions {
  pet: CodexPet | null
  enabled: boolean
  onState?: (state: PhysicsState) => void
}

// Tunables. Values approximate ShimejiEE defaults — feel free to
// tweak. Velocities are in CSS pixels per tick (TICK_MS).
const TICK_MS = 30
const GRAVITY = 0.5
const TERMINAL_VY = 12
const WALK_SPEED = 2
const CLIMB_SPEED = 1
const BOUNCE_DAMPING = 0.4
const RESISTANCE_X = 0.05
export const MAX_THROW_SPEED = 30
const BOUNCE_FRAMES = 6 // hold `bouncing` for ~180ms after impact
// While ascending a wall, the mascot eventually mounts the ceiling.
// Apply a small probability of voluntarily detaching once climbed
// halfway up the screen to avoid a stuck-on-wall feel.
const WALL_DETACH_CHANCE_PER_TICK = 0.005

// Probability per on-floor tick that the pet jumps up to climb onto a
// foreground app window's title bar — but only while it's standing
// directly underneath the window. ~1/600 with a 30 ms tick gives one
// trigger per ~18 s of overlap, which matches Shimeji's "JumpFromBottomOfIE"
// frequency intuition (rare enough to feel spontaneous, frequent enough
// to be a regular occurrence when the user actually has windows open).
const APPROACH_WINDOW_JUMP_PROB_PER_TICK = 1 / 600
// Overshoot the window top by this many CSS pixels so the falling-
// detection in `stepOnScreen` cleanly catches the landing on the way
// back down instead of grazing the title bar at apex.
const APPROACH_WINDOW_JUMP_OVERSHOOT_PX = 30
// Minimum ticks the pet must spend in `falling` before it can re-attach
// to a window's side or bottom. Without this cooldown, walking off the
// end of a title bar drops the pet a few pixels, immediately re-attaches
// it to the wall, the wall logic climbs it back up to the title bar,
// it walks across, falls off again — an infinite "lap" loop. ~20 ticks
// ≈ 600 ms combined with the outward kick on title-bar detach gives
// the pet enough distance to clear the window entirely.
const MIN_FALL_TICKS_BEFORE_WINDOW_ATTACH = 20
// Horizontal kick (CSS px/tick) applied when the pet walks off the end
// of a title bar. The kick continues the walking direction, so a pet
// that walked off the right corner sails further right while it falls
// rather than dropping straight down and immediately re-grabbing the
// window's right vertical. RESISTANCE_X damps it back to ~0 over the
// cooldown window.
const TITLE_BAR_WALKOFF_KICK = 4

interface MutablePhysicsState {
  state: PhysicsState
  vx: number
  vy: number
  // Direction the mascot is "facing" (-1 left / 1 right). Used for
  // sprite flipX without re-deriving from velocity each frame (which
  // can be noisy when velocity dips through zero during bounces).
  facing: -1 | 1
  bounceTicksRemaining: number
  // Animation time in ticks since the last state transition; lets the
  // sprite resolver insert a pose hold (e.g. brief grab-wall before
  // climbing).
  ticksInState: number
  // Last computed walking floor velocity sign — preserved so the
  // mascot keeps walking the same way after a bounce instead of
  // randomly flipping each tick.
  lastFloorDir: -1 | 1
  // Ticks spent walking on the floor since the last rest. Used to
  // gate the rest-dwell trigger so the cat doesn't immediately
  // re-enter rest after just having rested.
  floorWalkTicks: number
  // Remaining ticks of the current idle-rest dwell. >0 means the
  // cat is sitting still in `idle` pose on the floor; 0 means it's
  // walking (or eligible to start walking).
  restTicksRemaining: number
  // Which world the (state, vx, vy) is anchored to. 'screen' is the
  // classic Shimeji floor/walls/menu-bar play area; 'window' means
  // the pet is currently interacting with the frontmost app window
  // (sitting on its title bar, climbing its side, hanging from its
  // bottom edge). on_floor/on_wall/on_ceiling state names map onto
  // each surface — the sprite renderer doesn't need to care which
  // surface because the animations are the same (foot-on-edge,
  // hand-on-side, …).
  surface: 'screen' | 'window'
  // CGWindowID we're currently tracking. When the cached active
  // window's windowId stops matching this, we know the window we
  // were sitting on is gone / occluded / replaced and we should
  // detach to falling.
  surfaceWindowId: number | null
  // Last-observed active-window rect (Cocoa bottom-left). Lets the
  // physics loop apply the per-tick "FallWithIE" delta — when the
  // user drags the window, the pet rides along by adding the rect
  // change to its own position. `null` means we don't have a prior
  // snapshot yet (first tick on the window surface).
  lastWindowX: number | null
  lastWindowY: number | null
  lastWindowW: number | null
  lastWindowH: number | null
}

function initialState(): MutablePhysicsState {
  // Start in `falling` so the cat drops from wherever the mini window
  // happens to be (typically the notch at the top of the screen) and
  // settles on the floor naturally. Starting in `on_floor` would make
  // the cat walk along the top of the screen as if it were the ground
  // until it bumped into a wall, which looks broken.
  return {
    state: 'falling',
    vx: 0,
    vy: 0,
    facing: 1,
    bounceTicksRemaining: 0,
    ticksInState: 0,
    lastFloorDir: 1,
    floorWalkTicks: 0,
    restTicksRemaining: 0,
    surface: 'screen',
    surfaceWindowId: null,
    lastWindowX: null,
    lastWindowY: null,
    lastWindowW: null,
    lastWindowH: null,
  }
}

// Drop the window anchor: forget the tracked windowId and last-rect
// snapshot, return to screen surface, and enter `falling` so gravity
// takes over. Called whenever the window we were on disappears, the
// pet walks off the edge, or any other "no longer on this surface"
// condition fires.
function detachFromWindow(s: MutablePhysicsState) {
  s.state = 'falling'
  s.surface = 'screen'
  s.surfaceWindowId = null
  s.lastWindowX = null
  s.lastWindowY = null
  s.lastWindowW = null
  s.lastWindowH = null
  s.ticksInState = 0
}

// Rest-dwell tuning. The cat walks for at least ~1 second of ticks
// before becoming eligible to rest, then a small per-tick probability
// kicks off a 1-4 second pause in `idle` pose. Result: on the Dock the
// cat is in `idle` (flush feet) most of the time, with brief walking
// bursts that produce the per-pose anchor offset.
const FLOOR_WALK_MIN_TICKS_BEFORE_REST = 33   // ~1 s at 30 ms/tick
const FLOOR_REST_TRIGGER_PROB_PER_TICK = 0.005 // ~average 1 trigger per 200 ticks (~6 s)
const FLOOR_REST_MIN_TICKS = 33   // 1 s minimum dwell
const FLOOR_REST_MAX_EXTRA_TICKS = 100 // up to ~3 s extra (4 s total max)

// Map a (state, vx, facing) tuple to the animation key the sprite
// renderer should display. Names line up with the Phase-2 atlas rows
// added to the shimeji-bola pet.json (idle / running / run-right /
// run-left / grab-wall / climb-wall / climb-ceiling / falling /
// jumping / bouncing / waiting).
function spriteNameFor(s: MutablePhysicsState): string {
  switch (s.state) {
    case 'on_floor':
      if (Math.abs(s.vx) < 0.01) return 'idle'
      return s.vx > 0 ? 'run-right' : 'run-left'
    case 'on_wall': {
      // The grab-wall / climb-wall sprite is drawn with the cat body
      // on the *left* side of the cell (artist's native frame). That
      // hugs the LEFT wall correctly when the window is flush with
      // the screen's left edge. On the RIGHT wall (facing=-1) we need
      // to flip the sprite so the body sits on the right side of the
      // cell and touches the screen's right edge — otherwise the cat
      // would visually float far from the wall it's supposedly
      // clinging to. The pet.json declares paired -flipped variants
      // that point at the same atlas row with flipX:true.
      const flip = s.facing === -1 ? '-flipped' : ''
      if (s.ticksInState < 6 || Math.abs(s.vy) < 0.01) return 'grab-wall' + flip
      return 'climb-wall' + flip
    }
    case 'on_ceiling': {
      // On the ceiling the cat crawls in vx direction; mirror when
      // moving rightward so head/tail orient consistently with the
      // wall states.
      return s.vx > 0 ? 'climb-ceiling-flipped' : 'climb-ceiling'
    }
    case 'falling':
      return 'falling'
    case 'jumping':
      return 'jumping'
    case 'bouncing':
      return 'bouncing'
    case 'pinched':
      // Reuse the existing waiting animation row (Pinched in
      // ShimejiEE) — the user holds the cursor on the mascot during
      // drag.
      return 'waiting'
  }
}

// Inspect edges + state and pick the next state / velocity. Mutates
// `s` in place.
//
// Dispatches by surface: when the pet is on the window surface the
// edge semantics are different (window's top edge is a floor, walking
// off the side is a fall not a wall-bump, …) so the window-anchored
// states are handled by `stepOnWindow`. When the window we were on
// disappears (close, minimize, occlude, switch app, etc.) we detach
// to screen+falling unconditionally.
function step(s: MutablePhysicsState, edge: EdgeState) {
  s.ticksInState += 1
  if (s.surface === 'window') {
    if (
      !edge.activeWindow
      || edge.activeWindow.windowId !== s.surfaceWindowId
    ) {
      detachFromWindow(s)
      return
    }
    stepOnWindow(s, edge)
    return
  }
  stepOnScreen(s, edge)
}

// Window-surface state machine. Mirrors the screen state machine in
// structure (on_floor walks/rests, on_wall climbs, on_ceiling hangs)
// but the edges come from the window rect instead of the screen rect:
//   - "floor" = window's top edge (pet sits on title bar)
//   - "walls" = window's left & right verticals (pet climbs from outside)
//   - "ceiling" = window's bottom edge (pet hangs underneath)
// Walking off the side of a title bar is a fall, not a wall transition
// — there is no wall above the title bar in the screen world.
function stepOnWindow(s: MutablePhysicsState, edge: EdgeState) {
  const w = edge.activeWindow!
  switch (s.state) {
    case 'on_floor': {
      if (s.ticksInState === 1) {
        s.floorWalkTicks = 0
        s.restTicksRemaining = 0
      }
      // Walked off the end of the title bar, or the window moved away
      // from under our feet → fall back to the screen world.
      if (!w.withinHorizontalRange || !w.onTopOfWindow) {
        // When the cause is "walked past either end horizontally" we
        // kick the pet outward in its walking direction. Without the
        // kick, the pet drops straight down (vx ≈ WALK_SPEED only) and
        // can land within the side-attach window of the very edge it
        // just walked off, producing a corner-loop. With the kick + the
        // 20-tick cooldown, the pet's horizontal drift clears the
        // window's side-attach range before the cooldown expires.
        if (!w.withinHorizontalRange) {
          const walkOffDir = s.lastFloorDir
          detachFromWindow(s)
          s.vx = walkOffDir * TITLE_BAR_WALKOFF_KICK
        } else {
          detachFromWindow(s)
        }
        return
      }
      s.vy = 0
      // Idle-rest dwell (same logic as screen floor; the cat naps on
      // the title bar just like it does on the Dock).
      if (s.restTicksRemaining > 0) {
        s.restTicksRemaining -= 1
        s.vx = 0
        s.facing = s.lastFloorDir
        return
      }
      const skidding = Math.abs(s.vx) > WALK_SPEED * 1.5
      if (skidding) {
        s.vx *= 1 - RESISTANCE_X * 2
      } else {
        s.vx = WALK_SPEED * s.lastFloorDir
      }
      s.facing = s.vx >= 0 ? 1 : -1
      s.floorWalkTicks += 1
      if (
        s.floorWalkTicks > FLOOR_WALK_MIN_TICKS_BEFORE_REST
        && Math.random() < FLOOR_REST_TRIGGER_PROB_PER_TICK
      ) {
        s.restTicksRemaining =
          FLOOR_REST_MIN_TICKS + Math.floor(Math.random() * FLOOR_REST_MAX_EXTRA_TICKS)
        s.floorWalkTicks = 0
        s.vx = 0
        return
      }
      return
    }
    case 'on_wall': {
      // Climbing the window's side from outside. Up to the title bar
      // (climbed past window.top) → walk onto top edge. Down past the
      // window's bottom → detach (the screen-side wall logic uses the
      // dock floor; here we just fall back into the screen world).
      s.vx = 0
      if (s.vy === 0) s.vy = -CLIMB_SPEED
      // Re-assert facing every tick so even transient transitions
      // through other states (on_ceiling → on_wall, edge flag flickers
      // from window drag, etc.) can't leave the climb sprite mirrored
      // against the side it's clinging to. LEFT side uses flipped
      // (body on right of cell, looks outward to the left). RIGHT side
      // uses native (body on left of cell, looks outward to the right).
      // See the attachment block in `stepOnScreen.falling` for the
      // geometry rationale.
      s.facing = w.onLeftOfWindow ? -1 : 1
      // Reached title bar — switch to on_floor on the window surface
      // and walk inward across the bar.
      if (s.vy < 0 && !w.withinVerticalRange) {
        s.state = 'on_floor'
        s.ticksInState = 0
        s.vy = 0
        s.vx = 0
        s.lastFloorDir = w.onLeftOfWindow ? 1 : -1
        s.facing = s.lastFloorDir
        return
      }
      // Lost contact with the wall — detach.
      if (!w.onLeftOfWindow && !w.onRightOfWindow) {
        detachFromWindow(s)
        return
      }
      // Voluntary detach mid-climb (rare; matches screen wall behavior
      // so the cat doesn't get stuck spinning on the same side).
      if (s.ticksInState > 30 && Math.random() < WALL_DETACH_CHANCE_PER_TICK) {
        detachFromWindow(s)
        s.vx = w.onLeftOfWindow ? -1 : 1
        return
      }
      return
    }
    case 'on_ceiling': {
      // Hanging upside-down from the window's bottom edge. The pet
      // crawls in `vx` direction; hitting a side from below transitions
      // to wall-climbing.
      s.vy = 0
      if (!w.onBottomOfWindow) {
        detachFromWindow(s)
        return
      }
      if ((s.vx < 0 && w.onLeftOfWindow) || (s.vx > 0 && w.onRightOfWindow)) {
        s.state = 'on_wall'
        s.ticksInState = 0
        s.vx = 0
        s.vy = -CLIMB_SPEED
        // Match the reversed window-wall facing convention: LEFT side
        // uses the flipped sprite (body on right of cell), RIGHT side
        // uses native (body on left of cell). Without this assignment
        // we'd keep the on_ceiling facing, which doesn't reflect the
        // body-position the side cling requires.
        s.facing = w.onLeftOfWindow ? -1 : 1
        return
      }
      if (s.ticksInState > 60 && Math.random() < WALL_DETACH_CHANCE_PER_TICK) {
        detachFromWindow(s)
        return
      }
      return
    }
    default:
      // Any other state (falling/bouncing/jumping/pinched) while
      // surface='window' is a contradiction — detach and let the
      // screen step machine handle it next tick.
      detachFromWindow(s)
      return
  }
}

// Check whether the pet, currently walking on the screen floor, should
// spontaneously jump up to climb onto the foreground window's title
// bar. Returns true (and reconfigures `s` into a vertical jump in the
// `falling` state) when the trigger fires. Caller should `return` in
// that case so the rest of the on-floor logic is skipped.
//
// The trigger only fires when the pet is *already under* the window's
// horizontal footprint — the pet doesn't actively path-find toward
// windows for v1; instead, the natural left/right wandering carries
// it past every visible window often enough.
function maybeJumpToWindow(s: MutablePhysicsState, edge: EdgeState): boolean {
  if (!edge.activeWindow) return false
  if (Math.random() >= APPROACH_WINDOW_JUMP_PROB_PER_TICK) return false
  const winLeft = edge.activeWindow.rect.x
  const winRight = winLeft + edge.activeWindow.rect.width
  const petCenterX = edge.mascot.x + edge.mascot.width / 2
  if (petCenterX < winLeft || petCenterX > winRight) return false
  // Window top in Cocoa coords. Pet's current foot is at floor level
  // (roughly edge.mascot.y + sprite pad bottom, but using mascot bottom
  // is close enough — the overshoot constant absorbs the difference).
  const winTopY = edge.activeWindow.rect.y + edge.activeWindow.rect.height
  const apexNeeded = (winTopY - edge.mascot.y) + APPROACH_WINDOW_JUMP_OVERSHOOT_PX
  // If the window is below us (e.g. clipping into the Dock area) the
  // jump trigger doesn't apply — falling-detection would land us on
  // air. Bail.
  if (apexNeeded <= 0) return false
  // Kinematic: apex_height = v² / (2g). Cap at MAX_THROW_SPEED so we
  // can never launch off the top of the screen.
  const v = Math.min(MAX_THROW_SPEED, Math.sqrt(2 * GRAVITY * apexNeeded))
  // Enter falling state with upward initial velocity. The existing
  // falling-window-landing detection in stepOnScreen will catch us on
  // the way back down and switch to surface='window'.
  s.state = 'falling'
  s.ticksInState = 0
  s.vy = -v  // top-down: negative = up
  s.vx = 0
  return true
}

// Screen-surface state machine — the classic ShimejiEE behavior. This
// is the exact prior `step()` body, lifted into a named function so
// the surface dispatcher above can choose between screen and window.
function stepOnScreen(s: MutablePhysicsState, edge: EdgeState) {
  switch (s.state) {
    case 'on_floor': {
      // Reset walk/rest counters on the first tick of a fresh on_floor
      // visit (ticksInState was just incremented to 1 at top of step).
      // This makes the rest-dwell timing reset cleanly each time the
      // cat lands from a wall/ceiling/fall, instead of inheriting stale
      // counters from a previous on_floor session.
      if (s.ticksInState === 1) {
        s.floorWalkTicks = 0
        s.restTicksRemaining = 0
      }
      // If the floor disappeared under the pet (walked off the side of
      // the macOS Dock platform), start falling. The next tick of
      // gravity will carry it toward the actual screen bottom; horizontal
      // momentum from the walk is preserved for natural arc.
      if (!edge.onBottom) {
        s.state = 'falling'
        s.ticksInState = 0
        // Don't zero vx — the walk direction becomes the fall direction.
        return
      }
      s.vy = 0
      // Spontaneous "approach window" trigger — when the pet is already
      // under a foreground window's horizontal footprint, occasionally
      // jump up to land on its title bar. The jump uses gravity and the
      // existing falling→title-bar landing detection to enter the window
      // surface organically.
      if (maybeJumpToWindow(s, edge)) {
        return
      }
      // Idle-rest dwell: while the cat has rest ticks remaining, sit
      // still in `idle` pose (vx=0). spriteNameFor maps `on_floor` with
      // |vx|<0.01 to `idle`, so this directly renders the flush-feet
      // resting pose instead of the constantly-bobbing running cycle.
      if (s.restTicksRemaining > 0) {
        s.restTicksRemaining -= 1
        s.vx = 0
        s.facing = s.lastFloorDir
        return
      }
      // Aim to walk in `lastFloorDir`. Preserve any leftover horizontal
      // momentum from a recent throw so the cat skids instead of
      // instantly clamping to WALK_SPEED — feels more physical and
      // matches the "drag direction = throw direction" expectation.
      const skidding = Math.abs(s.vx) > WALK_SPEED * 1.5
      if (skidding) {
        s.vx *= 1 - RESISTANCE_X * 2
      } else {
        s.vx = WALK_SPEED * s.lastFloorDir
      }
      s.facing = s.vx >= 0 ? 1 : -1
      // Once we've been walking for the minimum time, roll the dice
      // each tick for a rest dwell. Tuned so the cat rests on average
      // every ~6 seconds for 1-4 seconds.
      s.floorWalkTicks += 1
      if (
        s.floorWalkTicks > FLOOR_WALK_MIN_TICKS_BEFORE_REST
        && Math.random() < FLOOR_REST_TRIGGER_PROB_PER_TICK
      ) {
        s.restTicksRemaining =
          FLOOR_REST_MIN_TICKS + Math.floor(Math.random() * FLOOR_REST_MAX_EXTRA_TICKS)
        s.floorWalkTicks = 0
        s.vx = 0
        return
      }
      if (edge.onLeft && s.vx < 0) {
        s.state = 'on_wall'
        s.ticksInState = 0
        s.vx = 0
        s.vy = -CLIMB_SPEED
        s.facing = 1
        return
      }
      if (edge.onRight && s.vx > 0) {
        s.state = 'on_wall'
        s.ticksInState = 0
        s.vx = 0
        s.vy = -CLIMB_SPEED
        s.facing = -1
        return
      }
      return
    }
    case 'on_wall': {
      s.vx = 0
      // Climb upward by default; once we hit the ceiling, transition.
      if (s.vy === 0) s.vy = -CLIMB_SPEED
      if (edge.onTop) {
        s.state = 'on_ceiling'
        s.ticksInState = 0
        s.vy = 0
        // Move toward the opposite wall along the ceiling.
        s.vx = edge.onLeft ? CLIMB_SPEED : -CLIMB_SPEED
        return
      }
      // Reached the floor while descending — settle and walk again.
      if (edge.onBottom && s.vy > 0) {
        s.state = 'on_floor'
        s.ticksInState = 0
        s.vy = 0
        // Walk away from the wall we were on.
        s.lastFloorDir = edge.onLeft ? 1 : -1
        s.facing = s.lastFloorDir
        return
      }
      // Voluntary detach mid-wall once climbed for a while.
      if (s.ticksInState > 30 && Math.random() < WALL_DETACH_CHANCE_PER_TICK) {
        s.state = 'falling'
        s.ticksInState = 0
        s.vx = edge.onLeft ? 1 : -1
        s.vy = 0
        return
      }
      // If we somehow drift away from the wall, fall.
      if (!edge.onLeft && !edge.onRight) {
        s.state = 'falling'
        s.ticksInState = 0
        return
      }
      return
    }
    case 'on_ceiling': {
      s.vy = 0
      // If we hit the opposite wall, transition to wall climbing
      // downward.
      if ((s.vx < 0 && edge.onLeft) || (s.vx > 0 && edge.onRight)) {
        s.state = 'on_wall'
        s.ticksInState = 0
        s.vx = 0
        s.vy = CLIMB_SPEED
        return
      }
      // Voluntary detach.
      if (s.ticksInState > 60 && Math.random() < WALL_DETACH_CHANCE_PER_TICK) {
        s.state = 'falling'
        s.ticksInState = 0
        s.vy = 0
        return
      }
      return
    }
    case 'falling': {
      // Apply gravity and air resistance.
      s.vy = Math.min(s.vy + GRAVITY, TERMINAL_VY)
      s.vx *= 1 - RESISTANCE_X
      // Window-landing detection — if there's an active app window
      // below us and we're horizontally within its footprint, land on
      // its title bar instead of continuing to the screen floor. This
      // is the natural way to enter the window surface: the pet falls
      // off the menu bar, drops past mid-screen, and lands on Finder's
      // title bar with a soft pose.
      if (
        edge.activeWindow
        && edge.activeWindow.onTopOfWindow
        && edge.activeWindow.withinHorizontalRange
        && s.vy > 0
      ) {
        s.state = 'on_floor'
        s.surface = 'window'
        s.surfaceWindowId = edge.activeWindow.windowId
        s.lastWindowX = edge.activeWindow.rect.x
        s.lastWindowY = edge.activeWindow.rect.y
        s.lastWindowW = edge.activeWindow.rect.width
        s.lastWindowH = edge.activeWindow.rect.height
        s.vy = 0
        s.ticksInState = 0
        s.lastFloorDir = s.vx >= 0 ? 1 : -1
        s.facing = s.lastFloorDir
        s.vx = 0
        return
      }
      // Window-side attachment — Shimeji's "HoldOntoIEWall" entry. When
      // the pet is falling next to a window's left/right vertical and
      // its body is within the window's vertical range, grab the side
      // and start climbing.
      //
      // FACING: the sprite atlas only ships native (body on LEFT of
      // cell, facing right) and flipped (body on RIGHT of cell, facing
      // left). For SCREEN walls the cat is *inside* the world, so
      // left-screen-wall uses native (body touches wall on its left =
      // correct). For WINDOW walls the cat is *outside* the window —
      // its body must sit on the OPPOSITE side of the cell, otherwise
      // the body floats away from the wall with empty space between.
      // So the facing assignment is REVERSED vs screen walls:
      //   - Window LEFT side: use FLIPPED sprite (facing=-1). Body on
      //     right of cell touches window's left vertical from outside.
      //     Cat looks left, away from the window — a natural "clinging
      //     from outside" posture.
      //   - Window RIGHT side: use NATIVE sprite (facing=+1). Body on
      //     left of cell touches window's right vertical from outside.
      //     Cat looks right, away from the window.
      //
      // COOLDOWN: re-attaching immediately after walking off a title
      // bar's end creates an infinite lap loop (walk off → fall →
      // re-attach → climb up → walk across → fall off → …). Require
      // some falling ticks first so the pet actually clears the
      // window vertically before snapping back on.
      if (
        edge.activeWindow
        && edge.activeWindow.withinVerticalRange
        && (edge.activeWindow.onLeftOfWindow || edge.activeWindow.onRightOfWindow)
        && s.ticksInState >= MIN_FALL_TICKS_BEFORE_WINDOW_ATTACH
      ) {
        s.state = 'on_wall'
        s.surface = 'window'
        s.surfaceWindowId = edge.activeWindow.windowId
        s.lastWindowX = edge.activeWindow.rect.x
        s.lastWindowY = edge.activeWindow.rect.y
        s.lastWindowW = edge.activeWindow.rect.width
        s.lastWindowH = edge.activeWindow.rect.height
        s.vx = 0
        s.vy = 0
        // Reversed vs screen wall (see comment above).
        s.facing = edge.activeWindow.onLeftOfWindow ? -1 : 1
        s.ticksInState = 0
        return
      }
      // Window-bottom attachment — the pet jumped upward, passed under
      // a window's bottom edge, and now hangs from it (Shimeji's
      // "ClimbIEBottom"). Mirror of the screen ceiling logic: only
      // engage on a soft upward contact, so a fast jump arcs past
      // without unexpected snagging. Same cooldown applies.
      if (
        edge.activeWindow
        && edge.activeWindow.onBottomOfWindow
        && edge.activeWindow.withinHorizontalRange
        && s.vy < 0
        && Math.abs(s.vy) < 1
        && s.ticksInState >= MIN_FALL_TICKS_BEFORE_WINDOW_ATTACH
      ) {
        s.state = 'on_ceiling'
        s.surface = 'window'
        s.surfaceWindowId = edge.activeWindow.windowId
        s.lastWindowX = edge.activeWindow.rect.x
        s.lastWindowY = edge.activeWindow.rect.y
        s.lastWindowW = edge.activeWindow.rect.width
        s.lastWindowH = edge.activeWindow.rect.height
        s.vy = 0
        s.vx = s.vx >= 0 ? CLIMB_SPEED : -CLIMB_SPEED
        s.ticksInState = 0
        return
      }
      // Side collision while falling.
      //   Hard hit (|vx| ≥ 1) → bounce inward with damping, keep falling.
      //                        Fall through (no early return) so a corner
      //                        hit can also bounce off the floor/ceiling
      //                        in the same tick.
      //   Soft contact         → grab the wall and start climbing.
      if (edge.onLeft && s.vx < 0) {
        if (Math.abs(s.vx) >= 1) {
          s.vx = Math.abs(s.vx) * BOUNCE_DAMPING
          s.facing = 1
        } else {
          s.state = 'on_wall'
          s.ticksInState = 0
          s.vx = 0
          s.vy = 0
          s.facing = 1
          return
        }
      } else if (edge.onRight && s.vx > 0) {
        if (Math.abs(s.vx) >= 1) {
          s.vx = -Math.abs(s.vx) * BOUNCE_DAMPING
          s.facing = -1
        } else {
          s.state = 'on_wall'
          s.ticksInState = 0
          s.vx = 0
          s.vy = 0
          s.facing = -1
          return
        }
      }
      // Ceiling collision (only when moving upward — physics vy is
      // top-down, so vy < 0 means rising). Hard hit bounces back down;
      // soft contact mounts the ceiling and crawls in current vx dir.
      if (edge.onTop && s.vy < 0) {
        if (Math.abs(s.vy) >= 1) {
          s.vy = Math.abs(s.vy) * BOUNCE_DAMPING
        } else {
          s.state = 'on_ceiling'
          s.ticksInState = 0
          s.vy = 0
          s.vx = s.vx >= 0 ? CLIMB_SPEED : -CLIMB_SPEED
          return
        }
      }
      // Bottom collision → bounce.
      if (edge.onBottom) {
        s.state = 'bouncing'
        s.ticksInState = 0
        s.bounceTicksRemaining = BOUNCE_FRAMES
        s.vy = -Math.abs(s.vy) * BOUNCE_DAMPING
        if (Math.abs(s.vy) < 1) {
          // Tiny bounce — settle immediately.
          s.vy = 0
          s.state = 'on_floor'
          s.lastFloorDir = s.vx >= 0 ? 1 : -1
        }
        return
      }
      s.facing = s.vx >= 0 ? 1 : -1
      return
    }
    case 'bouncing': {
      // Short hold then either continue falling or land.
      s.bounceTicksRemaining -= 1
      // Apply remaining vy to actually leave the floor.
      s.vy += GRAVITY
      if (s.bounceTicksRemaining <= 0) {
        if (s.vy >= 0 || edge.onBottom) {
          s.state = 'on_floor'
          s.ticksInState = 0
          s.vy = 0
          s.vx = 0
          s.lastFloorDir = s.facing
        } else {
          s.state = 'falling'
          s.ticksInState = 0
        }
      }
      return
    }
    case 'jumping': {
      // Currently unused — reserved for a future "jump from wall"
      // playful action. For now treat as falling.
      s.state = 'falling'
      s.ticksInState = 0
      return
    }
    case 'pinched': {
      // Position is owned by NSEvent drag in Rust; physics tick is a
      // no-op and just waits for `setPinched(false)` to release.
      s.vx = 0
      s.vy = 0
      return
    }
  }
}

export function usePhysicsLoop(opts: PhysicsOptions): PhysicsHandle {
  const optsRef = useRef(opts)
  const stateRef = useRef<MutablePhysicsState>(initialState())
  const pausedRef = useRef(false)
  // Reactive snapshot of (spriteName, physicsState). Updated only when
  // the sprite key changes so we don't re-render on every tick — the
  // physics tick may run 33×/sec but most ticks don't transition the
  // sprite (e.g. consecutive `falling` ticks all map to 'falling').
  const [snapshot, setSnapshot] = useState<{ spriteName: string; physicsState: PhysicsState }>(
    { spriteName: 'idle', physicsState: 'on_floor' },
  )
  const lastSpriteRef = useRef('idle')

  optsRef.current = opts

  // Stable callback identities. Recreating these per render would
  // detach event listeners that closed over the previous handle.
  const beginThrow = useRef((vx: number, vy: number) => {
    const s = stateRef.current
    s.state = 'falling'
    s.ticksInState = 0
    s.vx = Math.max(-MAX_THROW_SPEED, Math.min(MAX_THROW_SPEED, vx))
    s.vy = Math.max(-MAX_THROW_SPEED, Math.min(MAX_THROW_SPEED, vy))
    s.facing = s.vx >= 0 ? 1 : -1
    s.bounceTicksRemaining = 0
  }).current
  const setPaused = useRef((paused: boolean) => {
    pausedRef.current = paused
  }).current
  const setPinched = useRef((pinched: boolean) => {
    const s = stateRef.current
    if (pinched) {
      s.state = 'pinched'
      s.ticksInState = 0
      s.vx = 0
      s.vy = 0
    } else if (s.state === 'pinched') {
      s.state = 'falling'
      s.ticksInState = 0
    }
  }).current
  const getSpriteAnimationName = useRef(() => spriteNameFor(stateRef.current)).current
  const getPhysicsState = useRef(() => stateRef.current.state).current

  useEffect(() => {
    if (!opts.enabled || !opts.pet?.physics?.enabled) {
      stateRef.current = initialState()
      lastSpriteRef.current = 'idle'
      setSnapshot({ spriteName: 'idle', physicsState: 'on_floor' })
      return
    }
    let cancelled = false
    let tickInFlight = false

    const tick = async () => {
      if (cancelled || tickInFlight) return
      if (pausedRef.current || stateRef.current.state === 'pinched') return
      tickInFlight = true
      try {
        const edge = await detectEdges()
        const s = stateRef.current
        const before = s.state
        const beforeSurface = s.surface
        step(s, edge)
        if (s.state !== before && optsRef.current.onState) {
          optsRef.current.onState(s.state)
        }
        const newSprite = spriteNameFor(s)
        if (newSprite !== lastSpriteRef.current) {
          lastSpriteRef.current = newSprite
          setSnapshot({ spriteName: newSprite, physicsState: s.state })
        }

        // FallWithIE / WalkWithIE — when sitting on a window surface,
        // add the window's per-tick rect delta to the physics velocity
        // so the pet rides along with the window as the user drags it.
        // The window's bottom-left moves in Cocoa coords, but
        // move_mini_by expects top-down dy, so the Y delta is negated.
        let dx = s.vx
        let dy = s.vy
        if (
          s.surface === 'window'
          && edge.activeWindow
          && edge.activeWindow.windowId === s.surfaceWindowId
        ) {
          if (
            beforeSurface === 'window'
            && s.lastWindowX !== null
            && s.lastWindowY !== null
          ) {
            const wdx = edge.activeWindow.rect.x - s.lastWindowX
            const wdy = -(edge.activeWindow.rect.y - s.lastWindowY)
            // Clamp absurd jumps (window minimized → restored at new
            // origin) so we don't teleport the cat across the screen.
            // 300 logical px per 30 ms tick is already ~10× a fast
            // user-drag; anything beyond that is a window-state event
            // we should just detach from instead of "ride".
            if (Math.abs(wdx) > 300 || Math.abs(wdy) > 300) {
              detachFromWindow(s)
            } else {
              dx += wdx
              dy += wdy
            }
          }
          // Snapshot the new rect for next tick's delta computation.
          // Always update — even on the first tick after entering the
          // window surface, so the *next* tick has a baseline.
          s.lastWindowX = edge.activeWindow.rect.x
          s.lastWindowY = edge.activeWindow.rect.y
          s.lastWindowW = edge.activeWindow.rect.width
          s.lastWindowH = edge.activeWindow.rect.height
        }

        if (dx !== 0 || dy !== 0) {
          // detectEdges returns OS-native frame coords; move_mini_by
          // expects top-down dy on every platform.
          await invoke('move_mini_by', { dx, dy })
        }
      } catch {
        // Transient IPC errors (e.g. mini window not yet ready) — drop
        // the tick and keep ticking.
      } finally {
        tickInFlight = false
      }
    }

    const interval = setInterval(tick, TICK_MS)
    // Refresh the cached monitor + floor rects on enable so the very
    // first tick doesn't run with a stale value from before the user
    // dragged the mascot to a new screen or changed Dock layout.
    invalidateMonitorCache()
    invalidateFloorCache()
    invalidateActiveWindowCache()

    // Measure the absolute CSS-pixel offset between the rendered
    // sprite's visible edges and the surrounding window edges, for
    // all four sides. The physics loop subtracts each from the
    // corresponding visibleFrame edge so the visible character sits
    // flush with each screen edge (Dock top, menubar, side walls).
    //
    // The measurement combines alpha scans of every relevant
    // animation row with a DOM read of the rendered sprite's
    // bounding rect — both are needed because a fraction-of-window
    // formula misses (a) per-pose foot/head/side reach differences
    // and (b) any centering offset between the sprite div and the
    // surrounding window.
    //
    // Reset overrides first so a stale measurement from the previous
    // pet doesn't leak into the new pet's first physics tick. The
    // DOM anchor (`[data-physics-anchor]`) may not be in the tree
    // yet on the first attempt (effect fires before React commits);
    // retry with 100 ms backoff for up to ~2 s, then give up — the
    // hardcoded fraction fallback in `spritePadFor` still works.
    //
    // Push the result to Rust so `move_mini_by`'s safety-net clamp
    // agrees with the frontend edge detection. If they disagree the
    // clamp fights physics and the pet jitters at the boundaries.
    resetRuntimeSpritePadCSS()
    invoke('set_sprite_pad_fractions', { resetPx: true }).catch(() => {})

    const petForMeasure = opts.pet
    if (petForMeasure) {
      let attempt = 0
      const tryMeasure = async () => {
        if (cancelled) return
        const anchors = await measureSpriteAnchorsCSS(petForMeasure)
        if (cancelled) return
        if (anchors === null) {
          attempt += 1
          if (attempt >= 20) return // ~2 s of retries with 100 ms gap
          setTimeout(tryMeasure, 100)
          return
        }
        setRuntimeSpritePadCSS(anchors)
        // Only push the px fields that were actually measured. Fields
        // left out keep their reset (None) state in Rust, which falls
        // back to the fraction defaults — identical to what the
        // frontend uses for the same null field. So the two sides
        // stay in sync regardless of which animations the pet
        // declares.
        const payload: Record<string, number | boolean> = { resetPx: true }
        if (anchors.topPx !== null)    payload.topPx    = anchors.topPx
        if (anchors.rightPx !== null)  payload.rightPx  = anchors.rightPx
        if (anchors.bottomPx !== null) payload.bottomPx = anchors.bottomPx
        if (anchors.leftPx !== null)   payload.leftPx   = anchors.leftPx
        invoke('set_sprite_pad_fractions', payload).catch(() => {
          // Older Rust builds may not have the px fields; the
          // frontend override is still effective for visible landing
          // even if the safety-clamp disagrees by a couple of px.
        })
      }
      // requestAnimationFrame defers past the React commit of the
      // mascot so the very first attempt usually succeeds.
      requestAnimationFrame(() => { void tryMeasure() })
    }

    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [opts.enabled, opts.pet])

  return {
    beginThrow,
    setPaused,
    setPinched,
    getSpriteAnimationName,
    getPhysicsState,
    spriteName: snapshot.spriteName,
    physicsState: snapshot.physicsState,
  }
}
