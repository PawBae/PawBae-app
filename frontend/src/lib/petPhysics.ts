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
import { detectEdges, invalidateMonitorCache, type EdgeState } from './edgeDetect'

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
  }
}

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
    case 'on_wall':
      // Hold a static grab-wall pose for the first ~6 ticks (180 ms)
      // so the transition reads as "grabbed → started climbing".
      if (s.ticksInState < 6 || Math.abs(s.vy) < 0.01) return 'grab-wall'
      return 'climb-wall'
    case 'on_ceiling':
      return 'climb-ceiling'
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
function step(s: MutablePhysicsState, edge: EdgeState) {
  s.ticksInState += 1
  switch (s.state) {
    case 'on_floor': {
      // Aim to walk in `lastFloorDir`. Preserve any leftover horizontal
      // momentum from a recent throw so the cat skids instead of
      // instantly clamping to WALK_SPEED — feels more physical and
      // matches the "drag direction = throw direction" expectation.
      s.vy = 0
      const skidding = Math.abs(s.vx) > WALK_SPEED * 1.5
      if (skidding) {
        s.vx *= 1 - RESISTANCE_X * 2
      } else {
        s.vx = WALK_SPEED * s.lastFloorDir
      }
      s.facing = s.vx >= 0 ? 1 : -1
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
      // Side collision while falling → grab the wall.
      if (edge.onLeft && s.vx < 0) {
        s.state = 'on_wall'
        s.ticksInState = 0
        s.vx = 0
        s.vy = 0
        s.facing = 1
        return
      }
      if (edge.onRight && s.vx > 0) {
        s.state = 'on_wall'
        s.ticksInState = 0
        s.vx = 0
        s.vy = 0
        s.facing = -1
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
        step(s, edge)
        if (s.state !== before && optsRef.current.onState) {
          optsRef.current.onState(s.state)
        }
        const newSprite = spriteNameFor(s)
        if (newSprite !== lastSpriteRef.current) {
          lastSpriteRef.current = newSprite
          setSnapshot({ spriteName: newSprite, physicsState: s.state })
        }
        if (s.vx !== 0 || s.vy !== 0) {
          // detectEdges returns OS-native frame coords; move_mini_by
          // expects top-down dy on every platform.
          await invoke('move_mini_by', { dx: s.vx, dy: s.vy })
        }
      } catch {
        // Transient IPC errors (e.g. mini window not yet ready) — drop
        // the tick and keep ticking.
      } finally {
        tickInFlight = false
      }
    }

    const interval = setInterval(tick, TICK_MS)
    // Refresh the cached monitor rect on enable so the very first tick
    // doesn't run with a stale value from before the user dragged the
    // mascot to a new screen.
    invalidateMonitorCache()

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
