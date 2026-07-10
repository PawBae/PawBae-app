// Stroll-mode persistence contract. The frontend owns settings.json: the macOS
// tray toggle round-trips through `stroll_mode_enabled` (tray emits
// `stroll-mode-changed` → Main.svelte persists here → MascotView pushes the
// value back to Rust via `set_stroll_mode`), so the key name and the
// default-on behavior are load-bearing beyond this store.
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

import { settingsStore } from './settings.svelte';

describe('settingsStore stroll mode', () => {
  it('defaults strollEnabled to on when nothing is persisted', async () => {
    await settingsStore.loadSettings();
    expect(settingsStore.strollEnabled).toBe(true);
  });

  it('persists under stroll_mode_enabled and survives a reload', async () => {
    await settingsStore.setStrollEnabled(false);
    expect(harness.data.get('stroll_mode_enabled')).toBe(false);

    await settingsStore.loadSettings();
    expect(settingsStore.strollEnabled).toBe(false);
  });
});
