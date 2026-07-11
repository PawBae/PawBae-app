// Evolution model (Phase 6 growth system). Pure logic, zero Svelte/Tauri imports —
// mirrors the rewards.ts precedent so unit tests need no timers or mocks.
//
// XP is the pet's lifetime EARNED coins (Σ totals[src].earned): it is monotonic, already
// persisted/restored by the reward ledger, and survives spends — feeding the pet never
// un-evolves it. Stage thresholds are tuned against the reward constants: an active
// coding day lands around 250–300 XP (10 agent stops + daily gift + focus blocks), a
// pet-mode-only day around 60, so Legend is ~3 weeks of heavy use.
import type { CoinSource, CoinSourceTotals } from '../types';

export type EvolutionStageId = 'newborn' | 'sprout' | 'junior' | 'master' | 'legend';

// Work-style branch, decided by where the XP actually came from (not by items):
// commander = agent completions dominate, zen = focus/pomodoro dominate,
// companion = nurture sources dominate. Visible from the 'junior' stage up.
export type EvolutionStyleId = 'commander' | 'zen' | 'companion';

export interface EvolutionStage {
  id: EvolutionStageId;
  minXp: number;
  emoji: string;
}

export const EVOLUTION_STAGES: readonly EvolutionStage[] = [
  { id: 'newborn', minXp: 0, emoji: '🥚' },
  { id: 'sprout', minXp: 60, emoji: '🌱' },
  { id: 'junior', minXp: 300, emoji: '⭐' },
  { id: 'master', minXp: 1200, emoji: '🌟' },
  { id: 'legend', minXp: 4000, emoji: '👑' },
] as const;

// Index of the first stage that shows a work-style branch (and its aura tint).
export const STYLE_FROM_STAGE = 2;

const STYLE_SOURCES: Record<EvolutionStyleId, readonly CoinSource[]> = {
  commander: ['agent_stop'],
  zen: ['focus_minutes', 'pomodoro'],
  companion: ['daily_gift', 'feed', 'input_milestone'],
};

export interface EvolutionInfo {
  xp: number;
  stageIndex: number;
  stage: EvolutionStage;
  next: EvolutionStage | null;
  /** 0..1 progress from the current stage's floor toward the next stage (1 at max stage). */
  progress: number;
  /** Work-style branch; null below STYLE_FROM_STAGE or with no earnings yet. */
  style: EvolutionStyleId | null;
}

/** Lifetime XP: every coin ever EARNED (spends don't subtract — evolution never regresses). */
export function evolutionXp(totals: Record<CoinSource, CoinSourceTotals>): number {
  let xp = 0;
  for (const t of Object.values(totals)) {
    if (t && Number.isFinite(t.earned) && t.earned > 0) xp += t.earned;
  }
  return xp;
}

export function stageIndexFor(xp: number): number {
  let idx = 0;
  for (let i = 0; i < EVOLUTION_STAGES.length; i++) {
    if (xp >= EVOLUTION_STAGES[i].minXp) idx = i;
  }
  return idx;
}

/**
 * Work-style branch: the style whose sources earned the most lifetime XP. Ties resolve
 * in declaration order (commander > zen > companion) so the outcome is deterministic.
 */
export function dominantStyle(
  totals: Record<CoinSource, CoinSourceTotals>,
): EvolutionStyleId | null {
  let best: EvolutionStyleId | null = null;
  let bestEarned = 0;
  for (const [style, sources] of Object.entries(STYLE_SOURCES) as [
    EvolutionStyleId,
    readonly CoinSource[],
  ][]) {
    let earned = 0;
    for (const src of sources) {
      const t = totals[src];
      if (t && Number.isFinite(t.earned) && t.earned > 0) earned += t.earned;
    }
    if (earned > bestEarned) {
      best = style;
      bestEarned = earned;
    }
  }
  return best;
}

export function evolutionInfo(totals: Record<CoinSource, CoinSourceTotals>): EvolutionInfo {
  const xp = evolutionXp(totals);
  const stageIndex = stageIndexFor(xp);
  const stage = EVOLUTION_STAGES[stageIndex];
  const next = EVOLUTION_STAGES[stageIndex + 1] ?? null;
  const progress = next
    ? Math.min(1, Math.max(0, (xp - stage.minXp) / (next.minXp - stage.minXp)))
    : 1;
  const style = stageIndex >= STYLE_FROM_STAGE ? dominantStyle(totals) : null;
  return { xp, stageIndex, stage, next, progress, style };
}
