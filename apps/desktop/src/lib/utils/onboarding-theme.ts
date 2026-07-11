export type OnboardingTheme = 'system' | 'light' | 'dark';

export const ONBOARDING_THEME_STORAGE_KEY = 'pawbae-onboarding-theme';

export function normalizeOnboardingTheme(value: string | null): OnboardingTheme {
  return value === 'light' || value === 'dark' || value === 'system' ? value : 'system';
}
