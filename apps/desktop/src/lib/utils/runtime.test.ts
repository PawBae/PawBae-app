import { describe, expect, it, vi } from 'vitest';
import { isStageRuntime, resolveDevPreview } from './runtime';

describe('isStageRuntime', () => {
  it('does not call the Tauri label reader in a normal browser', () => {
    const readLabel = vi.fn(() => 'stage');
    expect(isStageRuntime({}, readLabel)).toBe(false);
    expect(readLabel).not.toHaveBeenCalled();
  });

  it('recognizes only the Tauri stage window', () => {
    const runtime = { __TAURI_INTERNALS__: {} };
    expect(isStageRuntime(runtime, () => 'stage')).toBe(true);
    expect(isStageRuntime(runtime, () => 'main')).toBe(false);
  });
});

describe('resolveDevPreview', () => {
  it('resolves one explicit development preview', () => {
    expect(resolveDevPreview(true, '?onboarding-preview')).toBe('onboarding');
    expect(resolveDevPreview(true, '?home-preview')).toBe('home');
    expect(resolveDevPreview(true, '?home-preview&onboarding-preview')).toBe('home');
    expect(resolveDevPreview(false, '?home-preview')).toBeNull();
    expect(resolveDevPreview(true, '')).toBeNull();
  });
});
