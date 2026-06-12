// Growth-loop store test (Phase 6): daily-gift streaks, achievement unlocks and the
// evolution celebration queue through the live store. The pure math is covered by
// utils/{evolution,achievements,gift-streak}.test.ts; this checks the glue.
import { beforeEach, describe, expect, it, vi } from 'vitest';

const harness = vi.hoisted(() => ({
  data: new Map<string, unknown>(),
}));

vi.mock('@tauri-apps/plugin-store', () => ({
  load: async () => ({
    get: async (key: string) => harness.data.get(key),
    set: async (key: string, value: unknown) => {
      harness.data.set(key, value);
    },
    save: async () => {},
  }),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: async () => () => {},
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: async () => null,
}));

import { EVOLUTION_STAGES } from '../utils/evolution';
import { dailyGiftAmount, initialRewardState } from '../utils/rewards';
import { petStore } from './pet.svelte';

function yesterdayUtc(): string {
  return new Date(Date.now() - 86_400_000).toISOString().slice(0, 10);
}

beforeEach(() => {
  // The store is a module singleton — reset every public slice the growth loop touches.
  petStore.rewards = initialRewardState();
  petStore.achievements = {};
  petStore.evolutionStageSeen = 0;
  petStore.celebrations = [];
  petStore.loadPetData(petStore.defaultPetData());
});

describe('daily gift streak', () => {
  it('extends a yesterday-claimed streak and pays the bonus', () => {
    petStore.loadPetData({
      ...petStore.defaultPetData(),
      coins: 0,
      lastDailyGift: yesterdayUtc(),
      giftStreak: 2,
    });
    expect(petStore.canClaimDailyGift).toBe(true);
    expect(petStore.nextGiftAmount).toBe(dailyGiftAmount(3));

    expect(petStore.claimDailyGift()).toBe(true);
    expect(petStore.petData.giftStreak).toBe(3);
    expect(petStore.petData.coins).toBe(dailyGiftAmount(3));
    expect(petStore.giftStreakLive).toBe(3);

    // Same-day double claim is refused and pays nothing.
    expect(petStore.canClaimDailyGift).toBe(false);
    expect(petStore.claimDailyGift()).toBe(false);
    expect(petStore.petData.coins).toBe(dailyGiftAmount(3));
  });

  it('restarts a broken streak at 1', () => {
    petStore.loadPetData({
      ...petStore.defaultPetData(),
      lastDailyGift: '2020-01-01',
      giftStreak: 30,
    });
    expect(petStore.giftStreakLive).toBe(0); // broken → display 0
    expect(petStore.claimDailyGift()).toBe(true);
    expect(petStore.petData.giftStreak).toBe(1);
    expect(petStore.petData.coins).toBe(dailyGiftAmount(1));
  });
});

describe('growth checks on coin commits', () => {
  it('unlocks achievements from an award and queues their celebrations', () => {
    expect(petStore.claimDailyGift()).toBe(true); // first gift ever
    expect(petStore.achievements.gift_first).toBeTypeOf('number');
    expect(petStore.celebrations).toContainEqual({ kind: 'achievement', id: 'gift_first' });
  });

  it('celebrates only the stage actually reached on a multi-stage jump', () => {
    const juniorXp = EVOLUTION_STAGES[2].minXp;
    petStore.awardCoins('agent_stop', juniorXp, { at: Date.now() });
    expect(petStore.evolutionStageSeen).toBe(2);
    const evolutions = petStore.celebrations.filter((c) => c.kind === 'evolution');
    expect(evolutions).toEqual([{ kind: 'evolution', stageIndex: 2 }]);
    // The style branch is visible from this stage and follows the XP's origin.
    expect(petStore.evolution.style).toBe('commander');
    // Stage achievements ride along.
    expect(petStore.achievements.evolved_junior).toBeTypeOf('number');
  });

  it('shiftCelebration pops exactly the queue head', () => {
    petStore.awardCoins('agent_stop', EVOLUTION_STAGES[1].minXp, { at: Date.now() });
    const before = petStore.celebrations.length;
    expect(before).toBeGreaterThan(0);
    const head = petStore.celebrations[0];
    petStore.shiftCelebration();
    expect(petStore.celebrations.length).toBe(before - 1);
    expect(petStore.celebrations[0]).not.toEqual(head);
  });

  it('never double-celebrates an already-seen stage', () => {
    petStore.awardCoins('agent_stop', EVOLUTION_STAGES[1].minXp, { at: Date.now() });
    petStore.celebrations = [];
    petStore.awardCoins('agent_stop', 1, { at: Date.now() }); // still within stage 1
    expect(petStore.celebrations.filter((c) => c.kind === 'evolution')).toEqual([]);
  });
});
