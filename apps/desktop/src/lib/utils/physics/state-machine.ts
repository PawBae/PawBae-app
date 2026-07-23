import type { EdgeState } from '../edge/types';
import {
  APPROACH_WINDOW_JUMP_OVERSHOOT_PX,
  APPROACH_WINDOW_JUMP_PROB_PER_TICK,
  BOUNCE_DAMPING,
  BOUNCE_FRAMES,
  CLIMB_SPEED,
  FLOOR_REST_MAX_EXTRA_TICKS,
  FLOOR_REST_MIN_TICKS,
  FLOOR_REST_TRIGGER_PROB_PER_TICK,
  FLOOR_WALK_MIN_TICKS_BEFORE_REST,
  GRAVITY,
  MAX_THROW_SPEED,
  MIN_FALL_TICKS_BEFORE_WINDOW_ATTACH,
  RESISTANCE_X,
  TERMINAL_VY,
  TITLE_BAR_WALKOFF_KICK,
  WALK_SPEED,
  WALL_DETACH_CHANCE_PER_TICK,
  WALL_GRAB_HOLD_TICKS,
} from './constants';
import type { MutablePhysicsState } from './types';

export function initialState(): MutablePhysicsState {
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
  };
}

export function detachFromWindow(s: MutablePhysicsState) {
  s.state = 'falling';
  s.surface = 'screen';
  s.surfaceWindowId = null;
  s.lastWindowX = null;
  s.lastWindowY = null;
  s.lastWindowW = null;
  s.lastWindowH = null;
  s.ticksInState = 0;
}

export function spriteNameFor(s: MutablePhysicsState): string {
  switch (s.state) {
    case 'on_floor':
      if (Math.abs(s.vx) < 0.01) return 'idle';
      return s.vx > 0 ? 'run-right' : 'run-left';
    case 'on_wall': {
      const flip = s.facing === -1 ? '-flipped' : '';
      if (s.ticksInState < WALL_GRAB_HOLD_TICKS || Math.abs(s.vy) < 0.01) return `grab-wall${flip}`;
      return `climb-wall${flip}`;
    }
    case 'on_ceiling':
      return s.vx > 0 ? 'climb-ceiling-flipped' : 'climb-ceiling';
    case 'falling':
      return 'falling';
    case 'jumping':
      return 'jumping';
    case 'bouncing':
      return 'bouncing';
    case 'pinched':
      return 'waiting';
  }
}

export function step(s: MutablePhysicsState, edge: EdgeState) {
  s.ticksInState += 1;
  if (s.surface === 'window') {
    if (!edge.activeWindow || edge.activeWindow.windowId !== s.surfaceWindowId) {
      detachFromWindow(s);
      return;
    }
    stepOnWindow(s, edge);
    return;
  }
  stepOnScreen(s, edge);
}

function stepOnWindow(s: MutablePhysicsState, edge: EdgeState) {
  const w = edge.activeWindow as NonNullable<typeof edge.activeWindow>;
  switch (s.state) {
    case 'on_floor': {
      if (s.ticksInState === 1) {
        s.floorWalkTicks = 0;
        s.restTicksRemaining = 0;
      }
      if (!w.withinHorizontalRange || !w.onTopOfWindow) {
        if (!w.withinHorizontalRange) {
          const walkOffDir = s.lastFloorDir;
          detachFromWindow(s);
          s.vx = walkOffDir * TITLE_BAR_WALKOFF_KICK;
        } else {
          detachFromWindow(s);
        }
        return;
      }
      s.vy = 0;
      if (s.restTicksRemaining > 0) {
        s.restTicksRemaining -= 1;
        s.vx = 0;
        s.facing = s.lastFloorDir;
        return;
      }
      const skidding = Math.abs(s.vx) > WALK_SPEED * 1.5;
      if (skidding) {
        s.vx *= 1 - RESISTANCE_X * 2;
      } else {
        s.vx = WALK_SPEED * s.lastFloorDir;
      }
      s.facing = s.vx >= 0 ? 1 : -1;
      s.floorWalkTicks += 1;
      if (
        s.floorWalkTicks > FLOOR_WALK_MIN_TICKS_BEFORE_REST &&
        Math.random() < FLOOR_REST_TRIGGER_PROB_PER_TICK
      ) {
        s.restTicksRemaining =
          FLOOR_REST_MIN_TICKS + Math.floor(Math.random() * FLOOR_REST_MAX_EXTRA_TICKS);
        s.floorWalkTicks = 0;
        s.vx = 0;
        return;
      }
      return;
    }
    case 'on_wall': {
      s.vx = 0;
      if (s.vy === 0) s.vy = -CLIMB_SPEED;
      s.facing = w.onLeftOfWindow ? 1 : -1;
      if (s.vy < 0 && !w.withinVerticalRange) {
        s.state = 'on_floor';
        s.ticksInState = 0;
        s.vy = 0;
        s.vx = 0;
        s.lastFloorDir = w.onLeftOfWindow ? 1 : -1;
        s.facing = s.lastFloorDir;
        return;
      }
      if (!w.onLeftOfWindow && !w.onRightOfWindow) {
        detachFromWindow(s);
        return;
      }
      if (s.ticksInState > 30 && Math.random() < WALL_DETACH_CHANCE_PER_TICK) {
        detachFromWindow(s);
        s.vx = w.onLeftOfWindow ? -1 : 1;
        return;
      }
      return;
    }
    case 'on_ceiling': {
      s.vy = 0;
      if (!w.onBottomOfWindow) {
        detachFromWindow(s);
        return;
      }
      if ((s.vx < 0 && w.onLeftOfWindow) || (s.vx > 0 && w.onRightOfWindow)) {
        s.state = 'on_wall';
        s.ticksInState = 0;
        s.vx = 0;
        s.vy = -CLIMB_SPEED;
        s.facing = w.onLeftOfWindow ? 1 : -1;
        return;
      }
      if (s.ticksInState > 60 && Math.random() < WALL_DETACH_CHANCE_PER_TICK) {
        detachFromWindow(s);
        return;
      }
      return;
    }
    default:
      detachFromWindow(s);
      return;
  }
}

function maybeJumpToWindow(s: MutablePhysicsState, edge: EdgeState): boolean {
  if (!edge.activeWindow) return false;
  if (Math.random() >= APPROACH_WINDOW_JUMP_PROB_PER_TICK) return false;
  const winLeft = edge.activeWindow.rect.x;
  const winRight = winLeft + edge.activeWindow.rect.width;
  const petCenterX = edge.mascot.x + edge.mascot.width / 2;
  if (petCenterX < winLeft || petCenterX > winRight) return false;
  const winTopY = edge.activeWindow.rect.y + edge.activeWindow.rect.height;
  const apexNeeded = winTopY - edge.mascot.y + APPROACH_WINDOW_JUMP_OVERSHOOT_PX;
  if (apexNeeded <= 0) return false;
  const v = Math.min(MAX_THROW_SPEED, Math.sqrt(2 * GRAVITY * apexNeeded));
  s.state = 'falling';
  s.ticksInState = 0;
  s.vy = -v;
  s.vx = 0;
  return true;
}

function maybeGrabWindowSideFromScreenFloor(s: MutablePhysicsState, edge: EdgeState): boolean {
  const w = edge.activeWindow;
  if (!w?.withinVerticalRange) return false;
  const walkingIntoLeftSide = w.onLeftOfWindow && s.vx > 0;
  const walkingIntoRightSide = w.onRightOfWindow && s.vx < 0;
  if (!walkingIntoLeftSide && !walkingIntoRightSide) return false;
  s.state = 'on_wall';
  s.surface = 'window';
  s.surfaceWindowId = w.windowId;
  s.lastWindowX = w.rect.x;
  s.lastWindowY = w.rect.y;
  s.lastWindowW = w.rect.width;
  s.lastWindowH = w.rect.height;
  s.vx = 0;
  s.vy = -CLIMB_SPEED;
  s.facing = w.onLeftOfWindow ? 1 : -1;
  s.ticksInState = 0;
  s.floorWalkTicks = 0;
  s.restTicksRemaining = 0;
  return true;
}

function stepOnScreen(s: MutablePhysicsState, edge: EdgeState) {
  switch (s.state) {
    case 'on_floor': {
      if (s.ticksInState === 1) {
        s.floorWalkTicks = 0;
        s.restTicksRemaining = 0;
      }
      if (!edge.onBottom) {
        s.state = 'falling';
        s.ticksInState = 0;
        return;
      }
      s.vy = 0;
      if (maybeJumpToWindow(s, edge)) return;
      if (s.restTicksRemaining > 0) {
        s.restTicksRemaining -= 1;
        s.vx = 0;
        s.facing = s.lastFloorDir;
        return;
      }
      const skidding = Math.abs(s.vx) > WALK_SPEED * 1.5;
      if (skidding) {
        s.vx *= 1 - RESISTANCE_X * 2;
      } else {
        s.vx = WALK_SPEED * s.lastFloorDir;
      }
      s.facing = s.vx >= 0 ? 1 : -1;
      if (maybeGrabWindowSideFromScreenFloor(s, edge)) return;
      s.floorWalkTicks += 1;
      if (
        s.floorWalkTicks > FLOOR_WALK_MIN_TICKS_BEFORE_REST &&
        Math.random() < FLOOR_REST_TRIGGER_PROB_PER_TICK
      ) {
        s.restTicksRemaining =
          FLOOR_REST_MIN_TICKS + Math.floor(Math.random() * FLOOR_REST_MAX_EXTRA_TICKS);
        s.floorWalkTicks = 0;
        s.vx = 0;
        return;
      }
      if (edge.onLeft && s.vx < 0) {
        s.state = 'on_wall';
        s.ticksInState = 0;
        s.vx = 0;
        s.vy = -CLIMB_SPEED;
        s.facing = -1;
        return;
      }
      if (edge.onRight && s.vx > 0) {
        s.state = 'on_wall';
        s.ticksInState = 0;
        s.vx = 0;
        s.vy = -CLIMB_SPEED;
        s.facing = 1;
        return;
      }
      return;
    }
    case 'on_wall': {
      s.vx = 0;
      if (s.vy === 0) s.vy = -CLIMB_SPEED;
      if (edge.onLeft || edge.onRight) {
        s.facing = edge.onLeft ? -1 : 1;
      }
      if (edge.onTop) {
        s.state = 'on_ceiling';
        s.ticksInState = 0;
        s.vy = 0;
        s.vx = edge.onLeft ? CLIMB_SPEED : -CLIMB_SPEED;
        return;
      }
      if (edge.onBottom && s.vy > 0) {
        s.state = 'on_floor';
        s.ticksInState = 0;
        s.vy = 0;
        s.lastFloorDir = edge.onLeft ? 1 : -1;
        s.facing = s.lastFloorDir;
        return;
      }
      if (s.ticksInState > 30 && Math.random() < WALL_DETACH_CHANCE_PER_TICK) {
        s.state = 'falling';
        s.ticksInState = 0;
        s.vx = edge.onLeft ? 1 : -1;
        s.vy = 0;
        return;
      }
      if (!edge.onLeft && !edge.onRight) {
        s.state = 'falling';
        s.ticksInState = 0;
        return;
      }
      return;
    }
    case 'on_ceiling': {
      s.vy = 0;
      if ((s.vx < 0 && edge.onLeft) || (s.vx > 0 && edge.onRight)) {
        s.state = 'on_wall';
        s.ticksInState = 0;
        s.vx = 0;
        s.vy = CLIMB_SPEED;
        s.facing = edge.onLeft ? -1 : 1;
        return;
      }
      if (s.ticksInState > 60 && Math.random() < WALL_DETACH_CHANCE_PER_TICK) {
        s.state = 'falling';
        s.ticksInState = 0;
        s.vy = 0;
        return;
      }
      return;
    }
    case 'falling': {
      s.vy = Math.min(s.vy + GRAVITY, TERMINAL_VY);
      s.vx *= 1 - RESISTANCE_X;
      if (edge.activeWindow?.onTopOfWindow && edge.activeWindow.withinHorizontalRange && s.vy > 0) {
        s.state = 'on_floor';
        s.surface = 'window';
        s.surfaceWindowId = edge.activeWindow.windowId;
        s.lastWindowX = edge.activeWindow.rect.x;
        s.lastWindowY = edge.activeWindow.rect.y;
        s.lastWindowW = edge.activeWindow.rect.width;
        s.lastWindowH = edge.activeWindow.rect.height;
        s.vy = 0;
        s.ticksInState = 0;
        s.lastFloorDir = s.vx >= 0 ? 1 : -1;
        s.facing = s.lastFloorDir;
        s.vx = 0;
        return;
      }
      if (
        edge.activeWindow?.withinVerticalRange &&
        (edge.activeWindow.onLeftOfWindow || edge.activeWindow.onRightOfWindow) &&
        s.ticksInState >= MIN_FALL_TICKS_BEFORE_WINDOW_ATTACH
      ) {
        s.state = 'on_wall';
        s.surface = 'window';
        s.surfaceWindowId = edge.activeWindow.windowId;
        s.lastWindowX = edge.activeWindow.rect.x;
        s.lastWindowY = edge.activeWindow.rect.y;
        s.lastWindowW = edge.activeWindow.rect.width;
        s.lastWindowH = edge.activeWindow.rect.height;
        s.vx = 0;
        s.vy = 0;
        s.facing = edge.activeWindow.onLeftOfWindow ? 1 : -1;
        s.ticksInState = 0;
        return;
      }
      if (
        edge.activeWindow?.onBottomOfWindow &&
        edge.activeWindow.withinHorizontalRange &&
        s.vy < 0 &&
        Math.abs(s.vy) < 1 &&
        s.ticksInState >= MIN_FALL_TICKS_BEFORE_WINDOW_ATTACH
      ) {
        s.state = 'on_ceiling';
        s.surface = 'window';
        s.surfaceWindowId = edge.activeWindow.windowId;
        s.lastWindowX = edge.activeWindow.rect.x;
        s.lastWindowY = edge.activeWindow.rect.y;
        s.lastWindowW = edge.activeWindow.rect.width;
        s.lastWindowH = edge.activeWindow.rect.height;
        s.vy = 0;
        s.vx = s.vx >= 0 ? CLIMB_SPEED : -CLIMB_SPEED;
        s.ticksInState = 0;
        return;
      }
      if (edge.onLeft && s.vx < 0) {
        if (Math.abs(s.vx) >= 1) {
          s.vx = Math.abs(s.vx) * BOUNCE_DAMPING;
          s.facing = 1;
        } else {
          s.state = 'on_wall';
          s.ticksInState = 0;
          s.vx = 0;
          s.vy = 0;
          s.facing = -1;
          return;
        }
      } else if (edge.onRight && s.vx > 0) {
        if (Math.abs(s.vx) >= 1) {
          s.vx = -Math.abs(s.vx) * BOUNCE_DAMPING;
          s.facing = -1;
        } else {
          s.state = 'on_wall';
          s.ticksInState = 0;
          s.vx = 0;
          s.vy = 0;
          s.facing = 1;
          return;
        }
      }
      if (edge.onTop && s.vy < 0) {
        if (Math.abs(s.vy) >= 1) {
          s.vy = Math.abs(s.vy) * BOUNCE_DAMPING;
        } else {
          s.state = 'on_ceiling';
          s.ticksInState = 0;
          s.vy = 0;
          s.vx = s.vx >= 0 ? CLIMB_SPEED : -CLIMB_SPEED;
          return;
        }
      }
      if (edge.onBottom) {
        s.state = 'bouncing';
        s.ticksInState = 0;
        s.bounceTicksRemaining = BOUNCE_FRAMES;
        s.vy = -Math.abs(s.vy) * BOUNCE_DAMPING;
        if (Math.abs(s.vy) < 1) {
          s.vy = 0;
          s.state = 'on_floor';
          s.lastFloorDir = s.vx >= 0 ? 1 : -1;
        }
        return;
      }
      s.facing = s.vx >= 0 ? 1 : -1;
      return;
    }
    case 'bouncing': {
      s.bounceTicksRemaining -= 1;
      s.vy += GRAVITY;
      if (s.bounceTicksRemaining <= 0) {
        if (s.vy >= 0 || edge.onBottom) {
          s.state = 'on_floor';
          s.ticksInState = 0;
          s.vy = 0;
          s.vx = 0;
          s.lastFloorDir = s.facing;
        } else {
          s.state = 'falling';
          s.ticksInState = 0;
        }
      }
      return;
    }
    case 'jumping': {
      s.state = 'falling';
      s.ticksInState = 0;
      return;
    }
    case 'pinched': {
      s.vx = 0;
      s.vy = 0;
      return;
    }
  }
}
