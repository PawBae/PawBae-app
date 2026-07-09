// Adventure/souvenir store glue: a genuine completion of a long-enough trip rolls a
// souvenir, queues its celebration, and persists the shelf. Trip TIMING math is
// covered by utils/adventure.test.ts; drop math by utils/souvenirs.test.ts.
import { beforeEach, describe, expect, it, vi } from 'vitest';

const harness = vi.hoisted(() => ({
  data: new Map<string, unknown>(),
  handlers: new Map<string, (e: { payload: unknown }) => void>(),
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
  listen: async (event: string, handler: (e: { payload: unknown }) => void) => {
    harness.handlers.set(event, handler);
    return () => {
      harness.handlers.delete(event);
    };
  },
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: async () => null,
}));

import { ADVENTURE_MIN_MS } from '../utils/adventure';
import { initialRewardState } from '../utils/rewards';
import { SOUVENIR_CATALOG } from '../utils/souvenirs';
import { petStore } from './pet.svelte';

const fireComplete = (sessionId: string, waiting = false) =>
  harness.handlers.get('claude-task-complete')?.({
    payload: { sessionId, waiting, source: 'cc' },
  });

beforeEach(async () => {
  // Let the previous test's queued savePetState land BEFORE clearing, or its stale
  // snapshot would overwrite fixtures set below (saveInFlight is fire-and-forget).
  await new Promise((r) => setTimeout(r, 0));
  harness.data.clear();
  petStore.rewards = initialRewardState();
  petStore.achievements = {};
  petStore.evolutionStageSeen = 0;
  petStore.celebrations = [];
  petStore.souvenirs = {};
  petStore.loadPetData(petStore.defaultPetData());
});

describe('souvenir grant on genuine completion', () => {
  it('a long trip drops one souvenir, queues its celebration, and persists', async () => {
    const dispose = await petStore.init();
    // The session has been busy since 4 minutes ago — well past the 3-minute mark.
    petStore.stepAdventure(['s1'], ['s1'], Date.now() - ADVENTURE_MIN_MS - 60_000);
    fireComplete('s1');

    const ids = Object.keys(petStore.souvenirs);
    expect(ids).toHaveLength(1);
    expect(SOUVENIR_CATALOG.some((d) => d.id === ids[0])).toBe(true);
    expect(petStore.souvenirs[ids[0]].count).toBe(1);
    expect(petStore.celebrations).toContainEqual({ kind: 'souvenir', id: ids[0] });
    await vi.waitFor(() => {
      expect(harness.data.get('souvenirs')).toMatchObject({ [ids[0]]: { count: 1 } });
    });

    // The trip was consumed — a duplicate completion event grants nothing more.
    fireComplete('s1');
    expect(Object.values(petStore.souvenirs).reduce((n, s) => n + s.count, 0)).toBe(1);
    dispose();
  });

  it('a short trip grants nothing', async () => {
    const dispose = await petStore.init();
    petStore.stepAdventure(['s1'], ['s1'], Date.now() - 30_000);
    fireComplete('s1');
    expect(Object.keys(petStore.souvenirs)).toHaveLength(0);
    // The completion still awards coins (and may unlock achievements) — only the
    // souvenir celebration must be absent.
    expect(petStore.celebrations.filter((c) => c.kind === 'souvenir')).toHaveLength(0);
    dispose();
  });

  it('a permission-wait is not a completion — the trip stays live for the real stop', async () => {
    const dispose = await petStore.init();
    petStore.stepAdventure(['s1'], ['s1'], Date.now() - ADVENTURE_MIN_MS - 60_000);
    fireComplete('s1', true);
    expect(Object.keys(petStore.souvenirs)).toHaveLength(0);
    fireComplete('s1');
    expect(Object.keys(petStore.souvenirs)).toHaveLength(1);
    dispose();
  });

  it('a session that never got tracked grants nothing', async () => {
    const dispose = await petStore.init();
    fireComplete('ghost');
    expect(Object.keys(petStore.souvenirs)).toHaveLength(0);
    dispose();
  });
});

describe('adventureAway display flag', () => {
  it('flips on when a busy session crosses the threshold and off when it leaves', () => {
    const t0 = Date.now() - ADVENTURE_MIN_MS - 1000;
    petStore.stepAdventure(['s1'], ['s1'], t0);
    expect(petStore.adventureAway).toBe(false);
    petStore.stepAdventure(['s1'], ['s1'], Date.now());
    expect(petStore.adventureAway).toBe(true);
    // Session vanishes (killed / ESC): quietly home, nothing granted.
    petStore.stepAdventure([], [], Date.now());
    expect(petStore.adventureAway).toBe(false);
    expect(Object.keys(petStore.souvenirs)).toHaveLength(0);
  });
});

describe('shelf hydration', () => {
  it('restores and sanitizes the persisted shelf', async () => {
    harness.data.set('souvenirs', {
      cloud_fluff: { count: 2, firstAt: 1_000 },
      broken: { count: 'no' },
    });
    const dispose = await petStore.init();
    expect(petStore.souvenirs.cloud_fluff).toEqual({ count: 2, firstAt: 1_000 });
    expect(petStore.souvenirs.broken).toBeUndefined();
    dispose();
  });
});
