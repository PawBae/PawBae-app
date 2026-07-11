import { describe, expect, it } from 'vitest';
import {
  agentAvailableOnPlatform,
  deriveOnboardingMode,
  hookCommandForAgent,
  nextOnboardingStep,
  OFFICIAL_PETS,
  previousOnboardingStep,
} from './onboarding';

describe('onboarding flow', () => {
  it('clamps next and previous at flow boundaries', () => {
    expect(previousOnboardingStep('welcome')).toBe('welcome');
    expect(nextOnboardingStep('welcome')).toBe('github');
    expect(nextOnboardingStep('github')).toBe('agents');
    expect(nextOnboardingStep('agents')).toBe('adopt');
    expect(nextOnboardingStep('adopt')).toBe('adopt');
  });

  it('derives coding mode only when at least one agent is selected', () => {
    expect(deriveOnboardingMode([])).toBe('pet');
    expect(deriveOnboardingMode(['claude'])).toBe('coding');
    expect(deriveOnboardingMode(['codex', 'cursor'])).toBe('coding');
  });

  it('defines the four official pets in poster order', () => {
    expect(OFFICIAL_PETS.map((pet) => pet.id)).toEqual(['solu', 'muru', 'riffi', 'luma']);
    expect(OFFICIAL_PETS.map((pet) => pet.posterIndex)).toEqual([0, 1, 2, 3]);
  });

  it('maps integrations to the commands exposed by Tauri', () => {
    expect(hookCommandForAgent('claude')).toBe('install_claude_hooks');
    expect(hookCommandForAgent('codex')).toBe('install_claude_hooks');
    expect(hookCommandForAgent('cursor')).toBe('install_cursor_hooks');
  });

  it('matches current Windows integration availability', () => {
    expect(agentAvailableOnPlatform('claude', true)).toBe(true);
    expect(agentAvailableOnPlatform('codex', true)).toBe(false);
    expect(agentAvailableOnPlatform('cursor', true)).toBe(false);
    expect(agentAvailableOnPlatform('codex', false)).toBe(true);
    expect(agentAvailableOnPlatform('cursor', false)).toBe(true);
  });
});
