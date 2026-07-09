// OBS 直播舞台 (spec: docs/superpowers/specs/2026-07-09-obs-stage-design.md)
//
// The stage window is a dumb render mirror of the main window's mascot: it runs
// no petStore and persists nothing — the main window stays the single brain (a
// second store instance would double-count rewards and race on pet.json). This
// module owns the snapshot the main window pushes over the `stage-state` event.

import type { GrowthCelebration } from '../types';
import type { AgentActivity } from './agent-activity';

/** Chroma-key backdrop presets. Blue/magenta rescue skins that read green. */
export const STAGE_BGS = ['green', 'blue', 'magenta'] as const;
export type StageBg = (typeof STAGE_BGS)[number];

export const STAGE_BG_COLORS: Record<StageBg, string> = {
  green: '#00ff00',
  blue: '#0000ff',
  magenta: '#ff00ff',
};

export function sanitizeStageBg(raw: unknown): StageBg {
  return STAGE_BGS.includes(raw as StageBg) ? (raw as StageBg) : 'green';
}

/**
 * Render snapshot broadcast to the stage webview. Bubble payloads travel
 * structured, NOT pre-rendered: the stage reuses the same props-driven bubble
 * components (CelebrationBubble / AgentBubble), so future celebration kinds
 * show up on stream without touching this protocol. The locale rides along
 * because each webview boots svelte-i18n from the navigator — the stage must
 * follow the main window's in-app language choice instead.
 */
export interface StageSnapshot {
  /** Skin id — the stage resolves it against its own read-only skins load. */
  petId: string;
  /** Base animation row (physics / agent-state result). */
  spriteState: string;
  /** One-shot overlay row (meal beat, input reaction, idle action) or null. */
  overlaySprite: string | null;
  /** Pet is off adventuring — the stage shows the tent marker, like the desktop. */
  away: boolean;
  celebration: GrowthCelebration | null;
  activity: AgentActivity;
  locale: string;
  bg: StageBg;
}

export interface StageSnapshotInput {
  petId: string;
  spriteState: string;
  overlaySprite?: string | null;
  away?: boolean;
  celebration?: GrowthCelebration | null;
  activity?: AgentActivity | null;
  locale?: string;
  bg?: unknown;
}

const IDLE_ACTIVITY: AgentActivity = { waiting: 0, compacting: 0, working: 0 };

export function buildStageSnapshot(input: StageSnapshotInput): StageSnapshot {
  return {
    petId: input.petId,
    spriteState: input.spriteState,
    overlaySprite: input.overlaySprite ?? null,
    away: input.away ?? false,
    celebration: input.celebration ?? null,
    activity: input.activity ?? IDLE_ACTIVITY,
    locale: input.locale || 'en',
    bg: sanitizeStageBg(input.bg),
  };
}

/**
 * Canonical dedup key. buildStageSnapshot fixes the field order, so JSON is
 * stable for snapshots built by it; the main window skips re-emitting when the
 * key hasn't changed.
 */
export function snapshotKey(snap: StageSnapshot): string {
  return JSON.stringify(snap);
}

export function snapshotsEqual(a: StageSnapshot, b: StageSnapshot): boolean {
  return snapshotKey(a) === snapshotKey(b);
}
