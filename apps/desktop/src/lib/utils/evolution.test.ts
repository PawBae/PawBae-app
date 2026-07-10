import { describe, expect, it } from 'vitest';
import type { CoinSource, CoinSourceTotals } from '../types';
import {
  dominantStyle,
  EVOLUTION_STAGES,
  evolutionInfo,
  evolutionXp,
  STYLE_FROM_STAGE,
  stageIndexFor,
} from './evolution';
import { initialRewardState } from './rewards';

function totalsWith(
  earned: Partial<Record<CoinSource, number>>,
): Record<CoinSource, CoinSourceTotals> {
  const totals = initialRewardState().totals;
  for (const [src, amount] of Object.entries(earned)) {
    totals[src as CoinSource].earned = amount as number;
  }
  return totals;
}

describe('evolutionXp', () => {
  it('sums lifetime earnings across all sources, ignoring spends', () => {
    const totals = totalsWith({ agent_stop: 100, focus_minutes: 40, daily_gift: 50 });
    totals.feed.spent = 500; // spends never reduce XP
    expect(evolutionXp(totals)).toBe(190);
  });

  it('treats corrupt totals as zero instead of poisoning the sum', () => {
    const totals = totalsWith({ agent_stop: 100 });
    totals.pomodoro.earned = Number.NaN;
    totals.daily_gift.earned = -50;
    expect(evolutionXp(totals)).toBe(100);
  });
});

describe('stageIndexFor', () => {
  it('maps XP to stages with inclusive thresholds', () => {
    expect(stageIndexFor(0)).toBe(0);
    for (let i = 1; i < EVOLUTION_STAGES.length; i++) {
      expect(stageIndexFor(EVOLUTION_STAGES[i].minXp - 1)).toBe(i - 1);
      expect(stageIndexFor(EVOLUTION_STAGES[i].minXp)).toBe(i);
    }
    expect(stageIndexFor(1_000_000)).toBe(EVOLUTION_STAGES.length - 1);
  });

  it('keeps stages strictly ascending (config sanity)', () => {
    for (let i = 1; i < EVOLUTION_STAGES.length; i++) {
      expect(EVOLUTION_STAGES[i].minXp).toBeGreaterThan(EVOLUTION_STAGES[i - 1].minXp);
    }
    expect(EVOLUTION_STAGES[0].minXp).toBe(0);
  });
});

describe('dominantStyle', () => {
  it('picks the style whose sources earned the most', () => {
    expect(dominantStyle(totalsWith({ agent_stop: 200, focus_minutes: 50 }))).toBe('commander');
    expect(dominantStyle(totalsWith({ focus_minutes: 100, pomodoro: 150, agent_stop: 200 }))).toBe(
      'zen',
    );
    expect(dominantStyle(totalsWith({ daily_gift: 300, feed: 10, agent_stop: 100 }))).toBe(
      'companion',
    );
  });

  it('returns null with no earnings at all', () => {
    expect(dominantStyle(initialRewardState().totals)).toBeNull();
  });

  it('resolves ties deterministically in declaration order', () => {
    // commander 100 vs zen 100 → commander (declared first).
    expect(dominantStyle(totalsWith({ agent_stop: 100, focus_minutes: 100 }))).toBe('commander');
  });
});

describe('evolutionInfo', () => {
  it('reports stage, next stage and clamped progress', () => {
    const sproutMin = EVOLUTION_STAGES[1].minXp;
    const juniorMin = EVOLUTION_STAGES[2].minXp;
    const midway = sproutMin + (juniorMin - sproutMin) / 2;
    const info = evolutionInfo(totalsWith({ agent_stop: midway }));
    expect(info.stageIndex).toBe(1);
    expect(info.stage.id).toBe('sprout');
    expect(info.next?.id).toBe('junior');
    expect(info.progress).toBeCloseTo(0.5);
  });

  it('pins progress to 1 with no next stage at the cap', () => {
    const info = evolutionInfo(
      totalsWith({ agent_stop: EVOLUTION_STAGES[EVOLUTION_STAGES.length - 1].minXp }),
    );
    expect(info.next).toBeNull();
    expect(info.progress).toBe(1);
  });

  it('hides the style branch below the branching stage', () => {
    const belowBranch = EVOLUTION_STAGES[STYLE_FROM_STAGE].minXp - 1;
    expect(evolutionInfo(totalsWith({ agent_stop: belowBranch })).style).toBeNull();
    expect(
      evolutionInfo(totalsWith({ agent_stop: EVOLUTION_STAGES[STYLE_FROM_STAGE].minXp })).style,
    ).toBe('commander');
  });
});
