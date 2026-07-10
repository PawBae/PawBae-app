import { invoke } from '@tauri-apps/api/core';
import {
  detectEdges,
  invalidateActiveWindowCache,
  invalidateFloorCache,
  invalidateMonitorCache,
  measureSpriteAnchorsCSS,
  resetRuntimeSpritePadCSS,
  setRuntimeSpritePadCSS,
} from '../edge/detect';
import { tryInvoke } from '../invoke';
import { MAX_THROW_SPEED, TICK_MS } from './constants';
import { detachFromWindow, initialState, spriteNameFor, step } from './state-machine';
import type { PhysicsHandle, PhysicsOptions, PhysicsState } from './types';

export function createPhysicsLoop(
  opts: PhysicsOptions,
): PhysicsHandle & { start: () => void; stop: () => void } {
  const state = initialState();
  let paused = false;
  let cancelled = false;
  let tickInFlight = false;
  let intervalId: ReturnType<typeof setInterval> | null = null;
  let lastSprite = 'idle';
  let currentOpts = opts;
  let snapshotSpriteName = 'idle';
  let snapshotPhysicsState: PhysicsState = 'on_floor';

  function syncSnapshot() {
    const name = spriteNameFor(state);
    lastSprite = name;
    snapshotSpriteName = name;
    snapshotPhysicsState = state.state;
  }

  function beginThrow(vx: number, vy: number) {
    state.state = 'falling';
    state.ticksInState = 0;
    state.vx = Math.max(-MAX_THROW_SPEED, Math.min(MAX_THROW_SPEED, vx));
    state.vy = Math.max(-MAX_THROW_SPEED, Math.min(MAX_THROW_SPEED, vy));
    state.facing = state.vx >= 0 ? 1 : -1;
    state.bounceTicksRemaining = 0;
    syncSnapshot();
  }

  function setPaused(v: boolean) {
    paused = v;
  }

  function setPinched(pinched: boolean) {
    if (pinched) {
      state.state = 'pinched';
      state.ticksInState = 0;
      state.vx = 0;
      state.vy = 0;
    } else if (state.state === 'pinched') {
      state.state = 'falling';
      state.ticksInState = 0;
    }
    syncSnapshot();
  }

  function getSpriteAnimationName() {
    return spriteNameFor(state);
  }
  function getPhysicsState() {
    return state.state;
  }

  function updateOpts(newOpts: PhysicsOptions) {
    currentOpts = newOpts;
  }

  async function pushMeasuredAnchors(): Promise<boolean> {
    if (cancelled || !currentOpts.pet) return false;
    const anchors = await measureSpriteAnchorsCSS(currentOpts.pet);
    if (cancelled || anchors === null) return false;
    setRuntimeSpritePadCSS(anchors);
    const payload: Record<string, number | boolean> = { resetPx: true };
    if (anchors.topPx !== null) payload.topPx = anchors.topPx;
    if (anchors.rightPx !== null) payload.rightPx = anchors.rightPx;
    if (anchors.bottomPx !== null) payload.bottomPx = anchors.bottomPx;
    if (anchors.leftPx !== null) payload.leftPx = anchors.leftPx;
    tryInvoke('set_sprite_pad_fractions', payload);
    return true;
  }

  async function measureWithRetries(attempt = 0) {
    if (cancelled) return;
    const measured = await pushMeasuredAnchors();
    if (!measured && attempt < 19) {
      setTimeout(() => {
        void measureWithRetries(attempt + 1);
      }, 100);
    }
  }

  function scheduleAnchorMeasure(delayMs = 0) {
    const schedule = () => {
      requestAnimationFrame(() => {
        void measureWithRetries();
      });
    };
    if (delayMs > 0) setTimeout(schedule, delayMs);
    else schedule();
  }

  async function tick() {
    if (cancelled || tickInFlight) return;
    if (paused || state.state === 'pinched') return;
    tickInFlight = true;
    try {
      const edge = await detectEdges();
      const before = state.state;
      const beforeSurface = state.surface;
      step(state, edge);
      if (state.state !== before && currentOpts.onState) {
        currentOpts.onState(state.state);
      }
      const newSprite = spriteNameFor(state);
      if (newSprite !== lastSprite) {
        lastSprite = newSprite;
        snapshotSpriteName = newSprite;
        snapshotPhysicsState = state.state;
        scheduleAnchorMeasure();
        scheduleAnchorMeasure(120);
      }

      let dx = state.vx;
      let dy = state.vy;
      if (
        state.surface === 'window' &&
        edge.activeWindow &&
        edge.activeWindow.windowId === state.surfaceWindowId
      ) {
        if (
          beforeSurface === 'window' &&
          state.lastWindowX !== null &&
          state.lastWindowY !== null
        ) {
          const wdx = edge.activeWindow.rect.x - state.lastWindowX;
          const wdy = -(edge.activeWindow.rect.y - state.lastWindowY);
          if (Math.abs(wdx) > 300 || Math.abs(wdy) > 300) {
            detachFromWindow(state);
          } else {
            dx += wdx;
            dy += wdy;
          }
        }
        state.lastWindowX = edge.activeWindow.rect.x;
        state.lastWindowY = edge.activeWindow.rect.y;
        state.lastWindowW = edge.activeWindow.rect.width;
        state.lastWindowH = edge.activeWindow.rect.height;
      }

      if (dx !== 0 || dy !== 0) {
        await invoke('move_mini_by', { dx, dy });
      }
    } catch {
      // Transient IPC errors — drop the tick
    } finally {
      tickInFlight = false;
    }
  }

  function start() {
    if (!currentOpts.enabled || !currentOpts.pet?.physics?.enabled) return;
    cancelled = false;
    invalidateMonitorCache();
    invalidateFloorCache();
    invalidateActiveWindowCache();
    resetRuntimeSpritePadCSS();
    tryInvoke('set_sprite_pad_fractions', { resetPx: true });
    if (currentOpts.pet) {
      scheduleAnchorMeasure();
      scheduleAnchorMeasure(120);
    }
    intervalId = setInterval(tick, TICK_MS);
  }

  function stop() {
    cancelled = true;
    if (intervalId) {
      clearInterval(intervalId);
      intervalId = null;
    }
  }

  return {
    beginThrow,
    setPaused,
    setPinched,
    getSpriteAnimationName,
    getPhysicsState,
    get spriteName() {
      return snapshotSpriteName;
    },
    get physicsState() {
      return snapshotPhysicsState;
    },
    updateOpts,
    start,
    stop,
  };
}
