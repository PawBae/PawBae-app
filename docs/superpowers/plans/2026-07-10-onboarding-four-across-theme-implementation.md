# PawBae Four-Across Adoption and Theme Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep all four official pets in one adoption row across supported desktop widths and add a persisted System/Light/Dark appearance selector to onboarding.

**Architecture:** `Onboarding.svelte` continues to own onboarding-only UI state. A small pure utility defines the theme preference contract and local-storage normalization, while CSS custom properties apply explicit light/dark overrides and retain `prefers-color-scheme` for System mode. The existing four-card component stays intact; only its compact sizing and the parent grid breakpoint change.

**Tech Stack:** Svelte 5 runes, TypeScript, svelte-i18n, CSS custom properties, localStorage, Vitest

## Global Constraints

- Screen 1 only; do not alter Settings, desktop stage, Friends, visiting, or Memory Card surfaces.
- Supported appearance choices are exactly `system`, `light`, and `dark`; visible UI copy is bilingual through svelte-i18n.
- Store the preference locally under `pawbae-onboarding-theme`.
- At `960 × 600` and `760 × 600`, Solu, Muru, Riffi, and Luma remain in one horizontal row with no horizontal scrolling.
- Pet artwork colors do not change between light and dark themes.
- Keep keyboard radio semantics, focus rings, reduced motion, and current CTA behavior.
- Never push directly to `main`; finish through a PR workflow.

---

### Task 1: Theme Preference Contract

**Files:**
- Create: `src/lib/utils/onboarding-theme.ts`
- Create: `src/lib/utils/onboarding-theme.test.ts`

**Interfaces:**
- Produces: `OnboardingTheme`, `ONBOARDING_THEME_STORAGE_KEY`, `normalizeOnboardingTheme(value: string | null): OnboardingTheme`
- Consumed by: `src/lib/components/Onboarding.svelte`

- [ ] **Step 1: Write the failing utility tests**

```ts
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
```

- [ ] **Step 2: Run the focused test and verify failure**

Run: `pnpm vitest run src/lib/utils/onboarding-theme.test.ts`

Expected: FAIL because `src/lib/utils/onboarding-theme.ts` does not exist.

- [ ] **Step 3: Implement the minimal pure utility**

```ts
export type OnboardingTheme = 'system' | 'light' | 'dark';

export const ONBOARDING_THEME_STORAGE_KEY = 'pawbae-onboarding-theme';

export function normalizeOnboardingTheme(value: string | null): OnboardingTheme {
  return value === 'light' || value === 'dark' || value === 'system' ? value : 'system';
}
```

- [ ] **Step 4: Run the focused test and verify success**

Run: `pnpm vitest run src/lib/utils/onboarding-theme.test.ts`

Expected: PASS with 2 tests.

### Task 2: Theme Selector and Four-Across Layout

**Files:**
- Modify: `src/lib/components/Onboarding.svelte`
- Modify: `src/lib/components/Onboarding.test.ts`
- Modify: `src/lib/components/onboarding/PetAdoptionCard.svelte`
- Modify: `src/lib/i18n/en.json`
- Modify: `src/lib/i18n/zh.json`

**Interfaces:**
- Consumes: `OnboardingTheme`, `ONBOARDING_THEME_STORAGE_KEY`, and `normalizeOnboardingTheme` from Task 1.
- Produces: a top-right `[data-theme-choice]` segmented control and `.onboarding-overlay[data-theme]` state.

- [ ] **Step 1: Write failing component tests**

Add tests that seed `localStorage` with `dark`, mount onboarding, assert `data-theme="dark"`, click `[data-theme-choice="light"]`, and assert both the DOM state and stored value become `light`. Add a source regression assertion that the 820px media query no longer changes `.pet-grid` to two columns.

- [ ] **Step 2: Run the component test and verify failure**

Run: `pnpm vitest run src/lib/components/Onboarding.test.ts`

Expected: FAIL because no theme control or `data-theme` attribute exists and the 820px rule still sets two columns.

- [ ] **Step 3: Implement the theme state and selector**

Initialize the preference with `normalizeOnboardingTheme(localStorage.getItem(ONBOARDING_THEME_STORAGE_KEY))`, set `data-theme={theme}` on the overlay, and persist changes from three real buttons labeled with `onboarding.theme.system`, `onboarding.theme.light`, and `onboarding.theme.dark`. Use `aria-pressed` for the active choice.

- [ ] **Step 4: Implement explicit and system theme tokens**

Move dark values into `.onboarding-overlay[data-theme='dark']` and duplicate that token override inside `@media (prefers-color-scheme: dark)` for `[data-theme='system']`. Use a token for action foreground color so explicit Dark mode does not depend on the media query.

- [ ] **Step 5: Keep adoption cards four across at narrow desktop width**

Remove the two-column override from the 820px media query. At that breakpoint, reduce `.pet-grid` gap from 14px to 8px and use compact card rows, padding, artwork, and typography in `PetAdoptionCard.svelte` without changing the four-column grid.

- [ ] **Step 6: Run focused tests and checks**

Run: `pnpm vitest run src/lib/utils/onboarding-theme.test.ts src/lib/components/Onboarding.test.ts`

Expected: PASS.

Run: `pnpm check`

Expected: 0 errors; existing unrelated accessibility warnings may remain unchanged.

- [ ] **Step 7: Verify in the live preview**

At `960 × 600` and `760 × 600`, navigate to Step 4 and confirm four computed grid columns, no horizontal overflow, legible Chinese and English labels, immediate Light/Dark switching, System media fallback, keyboard focus, and selected-card visibility.
