import type { CodexPet } from '../codex-pet';

export type PhysicsState =
  | 'on_floor'
  | 'on_wall'
  | 'on_ceiling'
  | 'falling'
  | 'jumping'
  | 'bouncing'
  | 'pinched';

export interface PhysicsHandle {
  beginThrow: (vx: number, vy: number) => void;
  setPaused: (paused: boolean) => void;
  setPinched: (pinched: boolean) => void;
  getSpriteAnimationName: () => string;
  getPhysicsState: () => PhysicsState;
  spriteName: string;
  physicsState: PhysicsState;
  updateOpts?: (opts: PhysicsOptions) => void;
}

export interface PhysicsOptions {
  pet: CodexPet | null;
  enabled: boolean;
  onState?: (state: PhysicsState) => void;
}

export interface MutablePhysicsState {
  state: PhysicsState;
  vx: number;
  vy: number;
  facing: -1 | 1;
  bounceTicksRemaining: number;
  ticksInState: number;
  lastFloorDir: -1 | 1;
  floorWalkTicks: number;
  restTicksRemaining: number;
  surface: 'screen' | 'window';
  surfaceWindowId: number | null;
  lastWindowX: number | null;
  lastWindowY: number | null;
  lastWindowW: number | null;
  lastWindowH: number | null;
}
