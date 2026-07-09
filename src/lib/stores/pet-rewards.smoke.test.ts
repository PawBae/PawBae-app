// Smoke test for the P1-C store integration: hydration, listener wiring, and the
// agent-completion reward path, with all three Tauri modules mocked in-memory.
// Reward MATH is covered by src/lib/utils/rewards.test.ts; this only checks the glue.
import { describe, expect, it, vi } from 'vitest';

const harness = vi.hoisted(() => ({
  data: new Map<string, unknown>(),
  calls: [] as string[],
  handlers: new Map<string, (e: { payload: unknown }) => void>(),
  saves: { count: 0 },
}));

vi.mock('@tauri-apps/plugin-store', () => ({
  load: async () => ({
    get: async (key: string) => harness.data.get(key),
    set: async (key: string, value: unknown) => {
      harness.data.set(key, value);
    },
    save: async () => {
      harness.saves.count += 1;
    },
  }),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: async (event: string, handler: (e: { payload: unknown }) => void) => {
    harness.handlers.set(event, handler);
    harness.calls.push(`listen:${event}`);
    return () => {
      harness.handlers.delete(event);
    };
  },
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: async (cmd: string) => {
    harness.calls.push(`invoke:${cmd}`);
    return null;
  },
}));

import { petStore } from './pet.svelte';

describe('petStore reward integration (smoke)', () => {
  it('hydrates persisted state, wires listeners, and routes agent completions to coins', async () => {
    // LOCAL calendar date — must match the store's todayStr() (switched off UTC
    // when the daily task board made the date line user-visible).
    const d = new Date();
    const today = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(
      d.getDate(),
    ).padStart(2, '0')}`;
    harness.data.set('coins', 30);
    harness.data.set('last_daily_gift', today);

    const dispose = await petStore.init();

    // Hydration restored the persisted slice (and the same-day gift stays claimed).
    expect(petStore.petData.coins).toBe(30);
    expect(petStore.claimDailyGift()).toBe(false);

    // Input tracking is enabled only AFTER the user-input listener registered.
    const listenIdx = harness.calls.indexOf('listen:user-input');
    const invokeIdx = harness.calls.indexOf('invoke:set_input_tracking');
    expect(listenIdx).toBeGreaterThanOrEqual(0);
    expect(invokeIdx).toBeGreaterThan(listenIdx);

    // A genuine completion (+20) lands in coins, the ledger, and the persisted store.
    const fireComplete = harness.handlers.get('claude-task-complete');
    expect(fireComplete).toBeDefined();
    fireComplete?.({ payload: { sessionId: 's1', waiting: false, source: 'cc' } });
    expect(petStore.petData.coins).toBe(50);
    const last = petStore.rewards.recent[petStore.rewards.recent.length - 1];
    expect(last.source).toBe('agent_stop');
    expect(last.sessionId).toBe('s1');
    await vi.waitFor(() => {
      expect(harness.data.get('coins')).toBe(50);
    });
    expect(harness.saves.count).toBeGreaterThan(0);

    // An immediate duplicate for the same session is deduped by the cooldown.
    fireComplete?.({ payload: { sessionId: 's1', waiting: false, source: 'cc' } });
    expect(petStore.petData.coins).toBe(50);

    // A permission-wait awards nothing.
    fireComplete?.({ payload: { sessionId: 's2', waiting: true, source: 'cc' } });
    expect(petStore.petData.coins).toBe(50);

    // init() is idempotent: same promise, no duplicate listener registrations.
    const again = await petStore.init();
    expect(again).toBe(dispose);
    expect(harness.calls.filter((c) => c === 'listen:claude-task-complete')).toHaveLength(1);

    dispose();
    expect(harness.handlers.size).toBe(0);
  });
});
