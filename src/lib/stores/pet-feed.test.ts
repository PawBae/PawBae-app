// Feed-loop store test (P1-C follow-up): the canFeed gate and applyFeed's
// hunger/affection/coin effects through the live store. Reward MATH (clamp,
// ledger caps) is covered by utils/rewards.test.ts; this checks the gate glue.
import { describe, expect, it, vi } from 'vitest';

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

import { FEED_COST_COINS } from '../utils/rewards';
import { AFFECTION_FEED_HUNGRY, HUNGER_MAX, petStore } from './pet.svelte';

describe('petStore feed loop', () => {
  it('refuses to feed when coins are short or hunger is full', () => {
    // Fresh store: 0 coins AND full hunger — both gates closed.
    expect(petStore.canFeed).toBe(false);
    expect(petStore.applyFeed()).toBe(false);
    expect(petStore.currentAction).toBe('idle');

    // Coins alone don't open the gate while hunger is at max.
    petStore.loadPetData({ ...petStore.defaultPetData(), coins: 100, hunger: HUNGER_MAX });
    expect(petStore.canFeed).toBe(false);
    expect(petStore.applyFeed()).toBe(false);
    expect(petStore.petData.coins).toBe(100); // refused feed never spends

    // Hunger alone doesn't either while the cost isn't covered.
    petStore.loadPetData({
      ...petStore.defaultPetData(),
      coins: FEED_COST_COINS - 1,
      hunger: 50,
    });
    expect(petStore.canFeed).toBe(false);
    expect(petStore.applyFeed()).toBe(false);
    expect(petStore.petData.hunger).toBe(50);
  });

  it('feeds when affordable: +hunger, -cost, ledger entry, eat action', () => {
    petStore.loadPetData({ ...petStore.defaultPetData(), coins: 12, hunger: 50 });
    expect(petStore.canFeed).toBe(true);

    expect(petStore.applyFeed()).toBe(true);
    expect(petStore.petData.hunger).toBe(70);
    expect(petStore.petData.coins).toBe(12 - FEED_COST_COINS);
    expect(petStore.currentAction).toBe('eat');
    const last = petStore.rewards.recent[petStore.rewards.recent.length - 1];
    expect(last.source).toBe('feed');
    expect(last.amount).toBe(-FEED_COST_COINS);
    // Not hungry (>= 30), so no affection bonus.
    expect(petStore.petData.affection).toBe(petStore.defaultPetData().affection);
  });

  it('grants the affection bonus when fed while hungry', () => {
    petStore.loadPetData({
      ...petStore.defaultPetData(),
      coins: 10,
      hunger: 20,
      affection: 50,
    });
    expect(petStore.applyFeed()).toBe(true);
    expect(petStore.petData.hunger).toBe(40);
    expect(petStore.petData.affection).toBe(50 + AFFECTION_FEED_HUNGRY);
  });

  it('clamps hunger at the max on a near-full feed', () => {
    petStore.loadPetData({ ...petStore.defaultPetData(), coins: 10, hunger: 95 });
    expect(petStore.applyFeed()).toBe(true);
    expect(petStore.petData.hunger).toBe(HUNGER_MAX);
  });
});
