// The opt-in gate is the whole privacy contract: telemetry defaults OFF, every
// event funnels through track(), and a rejecting plugin (keyless dev build)
// must never surface. The `telemetry_enabled` key name is load-bearing — it is
// what shipped settings.json files persist.
import { beforeEach, describe, expect, it, vi } from 'vitest';

const harness = vi.hoisted(() => ({
  data: new Map<string, unknown>(),
  trackEvent: vi.fn(() => Promise.resolve()),
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

vi.mock('@aptabase/tauri', () => ({
  trackEvent: harness.trackEvent,
}));

import { settingsStore } from '../stores/settings.svelte';
import { track } from './telemetry';

describe('telemetry opt-in gate', () => {
  beforeEach(() => {
    harness.trackEvent.mockClear();
  });

  it('defaults telemetryEnabled to OFF when nothing is persisted', async () => {
    harness.data.clear();
    await settingsStore.loadSettings();
    expect(settingsStore.telemetryEnabled).toBe(false);
  });

  it('sends nothing while the user has not opted in', async () => {
    await settingsStore.setTelemetryEnabled(false);
    track('app_started', { mode: 'coding' });
    expect(harness.trackEvent).not.toHaveBeenCalled();
  });

  it('forwards name and props once opted in', async () => {
    await settingsStore.setTelemetryEnabled(true);
    track('meal_fed', { tier: 'feast' });
    expect(harness.trackEvent).toHaveBeenCalledExactlyOnceWith('meal_fed', { tier: 'feast' });
  });

  it('persists under telemetry_enabled and survives a reload', async () => {
    await settingsStore.setTelemetryEnabled(true);
    expect(harness.data.get('telemetry_enabled')).toBe(true);
    await settingsStore.loadSettings();
    expect(settingsStore.telemetryEnabled).toBe(true);
  });

  it('swallows a rejecting trackEvent (keyless build: plugin not registered)', async () => {
    await settingsStore.setTelemetryEnabled(true);
    harness.trackEvent.mockImplementationOnce(() => Promise.reject(new Error('plugin not found')));
    expect(() => track('app_started')).not.toThrow();
    await Promise.resolve(); // let the rejection settle — an unhandled one fails the run
  });
});
