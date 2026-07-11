import { trackEvent } from '@aptabase/tauri';
import { settingsStore } from '../stores/settings.svelte';

/**
 * The single exit point for anonymous usage telemetry (Aptabase). Every event in
 * the app goes through here so the opt-in gate lives in exactly one place:
 * nothing is sent unless the user turned on `telemetryEnabled` (default OFF).
 *
 * Property values are restricted to enum-like strings and numbers — never ids,
 * paths, prompts, or anything user-authored. The event dictionary lives in
 * docs/superpowers/specs/2026-07-08-telemetry-aptabase.md; add a row there
 * before adding a call site.
 *
 * Keyless dev builds don't register the Rust plugin at all, so trackEvent
 * rejects with a missing-plugin error — swallowed here on purpose.
 */
export function track(name: string, props?: Record<string, string | number>) {
  if (!settingsStore.telemetryEnabled) return;
  void Promise.resolve(trackEvent(name, props)).catch(() => {});
}
