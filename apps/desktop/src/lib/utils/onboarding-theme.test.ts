import { describe, expect, it } from 'vitest';
import { normalizeOnboardingTheme } from './onboarding-theme';

describe('onboarding theme preference', () => {
  it.each(['system', 'light', 'dark'] as const)('accepts %s', (theme) => {
    expect(normalizeOnboardingTheme(theme)).toBe(theme);
  });

  it('falls back to system for missing or unknown values', () => {
    expect(normalizeOnboardingTheme(null)).toBe('system');
    expect(normalizeOnboardingTheme('sepia')).toBe('system');
  });
});
