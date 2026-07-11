# PawBae Four-Pet Onboarding UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the first-run mode picker with a polished four-step desktop onboarding flow that saves local choices, installs selected Agent Hooks, presents the four official pets, and never pretends GitHub OAuth is available.

**Architecture:** Put deterministic flow rules and metadata in a pure TypeScript module, keep reusable visual rows/cards as focused Svelte components, and let `Onboarding.svelte` own transient draft state. `Main.svelte` remains the persistence and window-lifecycle boundary, receiving one typed completion payload and committing it through existing stores.

**Tech Stack:** Svelte 5 runes, TypeScript, svelte-i18n, Tauri invoke, Vitest, CSS custom properties

## Global Constraints

- Implement Screen 1 only: Welcome, GitHub, Coding Agents, Adopt.
- GitHub OAuth remains unavailable unless an `onGithubSignIn` callback is supplied; never fabricate login success.
- Telemetry is unchecked and disabled by default.
- Selected Agent Hooks use real Tauri commands and display real loading, success, failure, and unsupported states.
- Solu, Muru, Riffi, and Luma use the approved poster for onboarding presentation only; do not register poster art as a working desktop sprite.
- Persist the selected official starter id; the existing skin resolver may render Yoonie until Screen 2 supplies stage sprites.
- Support English and Simplified Chinese, light and dark themes, keyboard navigation, visible focus, and reduced motion.
- Do not redesign Settings, the desktop stage, Friends, visiting, or Memory Card surfaces.
- Never push directly to `main`; finish through a PR workflow.

---

### Task 1: Pure Onboarding Model and Tests

**Files:**
- Create: `src/lib/utils/onboarding.ts`
- Create: `src/lib/utils/onboarding.test.ts`

**Interfaces:**
- Produces: `OnboardingStep`, `OfficialPetId`, `AgentId`, `GithubProfile`, `OnboardingResult`, `ONBOARDING_STEPS`, `OFFICIAL_PETS`, `nextOnboardingStep`, `previousOnboardingStep`, `deriveOnboardingMode`, `hookCommandForAgent`, `agentAvailableOnPlatform`
- Consumed by: `Onboarding.svelte`, `PetAdoptionCard.svelte`, `AgentConnectionRow.svelte`, and `Main.svelte`

- [ ] **Step 1: Write the failing model tests**

Create `src/lib/utils/onboarding.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import {
  OFFICIAL_PETS,
  agentAvailableOnPlatform,
  deriveOnboardingMode,
  hookCommandForAgent,
  nextOnboardingStep,
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
```

- [ ] **Step 2: Run the focused test and verify failure**

Run: `pnpm vitest run src/lib/utils/onboarding.test.ts`

Expected: FAIL because `src/lib/utils/onboarding.ts` does not exist.

- [ ] **Step 3: Implement the pure model**

Create `src/lib/utils/onboarding.ts`:

```ts
import type { AppMode } from '../types';

export type OnboardingStep = 'welcome' | 'github' | 'agents' | 'adopt';
export type OfficialPetId = 'solu' | 'muru' | 'riffi' | 'luma';
export type AgentId = 'claude' | 'codex' | 'cursor';
export type AgentInstallStatus = 'idle' | 'installing' | 'connected' | 'failed';

export interface GithubProfile {
  login: string;
  displayName?: string;
  avatarUrl?: string;
}

export interface OnboardingResult {
  mode: AppMode;
  shareTelemetry: boolean;
  selectedAgents: AgentId[];
  starterPetId: OfficialPetId | null;
  githubProfile: GithubProfile | null;
}

export interface OfficialPet {
  id: OfficialPetId;
  posterIndex: 0 | 1 | 2 | 3;
  color: string;
  strongColor: string;
}

export const ONBOARDING_STEPS: readonly OnboardingStep[] = [
  'welcome',
  'github',
  'agents',
  'adopt',
] as const;

export const OFFICIAL_PETS: readonly OfficialPet[] = [
  { id: 'solu', posterIndex: 0, color: '#F58F5E', strongColor: '#9C472F' },
  { id: 'muru', posterIndex: 1, color: '#B3C7F0', strongColor: '#455A96' },
  { id: 'riffi', posterIndex: 2, color: '#A8E0C0', strongColor: '#2E6C58' },
  { id: 'luma', posterIndex: 3, color: '#F5AFC8', strongColor: '#7E4160' },
] as const;

export function nextOnboardingStep(step: OnboardingStep): OnboardingStep {
  const index = ONBOARDING_STEPS.indexOf(step);
  return ONBOARDING_STEPS[Math.min(index + 1, ONBOARDING_STEPS.length - 1)];
}

export function previousOnboardingStep(step: OnboardingStep): OnboardingStep {
  const index = ONBOARDING_STEPS.indexOf(step);
  return ONBOARDING_STEPS[Math.max(index - 1, 0)];
}

export function deriveOnboardingMode(selectedAgents: readonly AgentId[]): AppMode {
  return selectedAgents.length > 0 ? 'coding' : 'pet';
}

export function hookCommandForAgent(agent: AgentId): 'install_claude_hooks' | 'install_cursor_hooks' {
  return agent === 'cursor' ? 'install_cursor_hooks' : 'install_claude_hooks';
}

export function agentAvailableOnPlatform(agent: AgentId, isWindows: boolean): boolean {
  return !isWindows || agent === 'claude';
}
```

- [ ] **Step 4: Run the focused test and verify pass**

Run: `pnpm vitest run src/lib/utils/onboarding.test.ts`

Expected: one test file and five tests pass.

- [ ] **Step 5: Commit the model**

```bash
git add src/lib/utils/onboarding.ts src/lib/utils/onboarding.test.ts
git commit -m "feat: add onboarding flow model"
```

---

### Task 2: Poster Asset and Bilingual Copy

**Files:**
- Create: `public/assets/onboarding/pet-family-poster.png`
- Modify: `src/lib/i18n/en.json`
- Modify: `src/lib/i18n/zh.json`

**Interfaces:**
- Produces: `/assets/onboarding/pet-family-poster.png` and the complete `onboarding.*` translation tree
- Consumed by: all onboarding components

- [ ] **Step 1: Copy the approved poster without deleting the generated original**

```bash
mkdir -p public/assets/onboarding
cp /Users/user/.codex/generated_images/019f4dac-333a-7360-af4a-b343db65bb76/exec-736c7b36-7e2d-4453-ae84-cbb1ea934bdc.png \
  public/assets/onboarding/pet-family-poster.png
```

Expected: `file public/assets/onboarding/pet-family-poster.png` reports a PNG image, `1716 × 916`.

- [ ] **Step 2: Replace the English onboarding translation object**

Replace only the existing `onboarding` object in `src/lib/i18n/en.json` with keys grouped under `common`, `welcome`, `github`, `agents`, `adopt`, and `errors`. Required exact primary copy:

```json
{
  "common": {
    "setupLater": "Set up later",
    "back": "Back",
    "continue": "Continue",
    "aboutMinute": "About 1 minute"
  },
  "welcome": {
    "title": "Meet the gentler side of AI agents",
    "body": "Your pet reacts when Claude Code, Codex, or Cursor works.",
    "local": "Agent activity is processed locally.",
    "telemetry": "Share anonymous feature counts to help improve PawBae. Never includes prompts, file paths, or personal content."
  },
  "github": {
    "title": "Connect your identity—not your code",
    "body": "GitHub will power friends and shared memories without reading your repositories.",
    "action": "Continue with GitHub",
    "unavailable": "GitHub sign-in opens with the Friends beta.",
    "skip": "Skip for now"
  },
  "agents": {
    "title": "Which agent should your pet listen to?",
    "body": "Choose one or more. PawBae installs a small local Hook for each connection.",
    "claudeDesc": "Monitor local Claude Code sessions",
    "codexDesc": "Monitor local Codex sessions",
    "cursorDesc": "Monitor Cursor agent sessions",
    "installing": "Installing local Hook…",
    "connected": "Connected",
    "failed": "Connection failed",
    "unsupportedWindows": "Not available on Windows yet",
    "petOnly": "Just keep me company"
  },
  "adopt": {
    "title": "Choose your first desktop companion",
    "body": "Different personalities, one gentle way to stay close to your agents.",
    "soluName": "Solu",
    "soluTrait": "Warm & sunny",
    "muruName": "Muru",
    "muruTrait": "Shy & soothing",
    "riffiName": "Riffi",
    "riffiTrait": "Energetic & clumsy",
    "lumaName": "Luma",
    "lumaTrait": "Sleepy & dreamy",
    "changeLater": "You can change pets anytime",
    "spriteNotice": "Your choice will be saved; animated desktop forms arrive with the Stage update.",
    "action": "Adopt {{name}}"
  },
  "errors": {
    "complete": "Couldn’t save your setup. Try again.",
    "poster": "Pet preview unavailable"
  }
}
```

- [ ] **Step 3: Add the matching Chinese translation tree**

Use the same keys in `src/lib/i18n/zh.json` with the approved copy: `让 AI Agent 更亲近一点`, `连接身份，不读取你的代码`, `让宠物听见哪个 Agent？`, `选择第一位桌面伙伴`, `暂不登录`, `先让它陪陪我`, `以后可以随时更换`, and the exact four names `小煦`, `雾露`, `雷栗`, `星沫`.

- [ ] **Step 4: Verify both locale trees have matching onboarding keys**

Run a Node one-liner that recursively collects `onboarding` keys from both JSON files and exits non-zero when the sorted arrays differ.

Expected: exit 0 and print `onboarding locale keys match`.

- [ ] **Step 5: Commit asset and copy**

```bash
git add public/assets/onboarding/pet-family-poster.png src/lib/i18n/en.json src/lib/i18n/zh.json
git commit -m "feat: add four-pet onboarding content"
```

---

### Task 3: Reusable Progress, Agent, and Adoption Components

**Files:**
- Create: `src/lib/components/onboarding/OnboardingProgress.svelte`
- Create: `src/lib/components/onboarding/AgentConnectionRow.svelte`
- Create: `src/lib/components/onboarding/PetAdoptionCard.svelte`
- Create: `src/lib/components/onboarding/onboarding-components.test.ts`

**Interfaces:**
- `OnboardingProgress`: `{ step: OnboardingStep }`
- `AgentConnectionRow`: `{ id: AgentId; selected: boolean; available: boolean; status: AgentInstallStatus; error: string; onToggle: () => void; onRetry: () => void }`
- `PetAdoptionCard`: `{ pet: OfficialPet; selected: boolean; onSelect: () => void }`

- [ ] **Step 1: Implement `OnboardingProgress.svelte`**

Render one ordered list from `ONBOARDING_STEPS`. Use `aria-current="step"` on the current item, a check icon for completed steps, localized labels, and a visually-hidden live region announcing the current step. Use a horizontal desktop layout that wraps only below 640px.

- [ ] **Step 2: Implement `AgentConnectionRow.svelte`**

Use a real `<button role="switch">` with `aria-checked`, `aria-disabled`, and visible focus. Keep the product icon monochrome. Render exactly one status line: installing, connected, unsupported, or the returned error plus a Retry button. Disable only the active row while installation is pending.

- [ ] **Step 3: Implement `PetAdoptionCard.svelte`**

Use a real radio button pattern: the parent supplies `role="radiogroup"`, each card is a `<button role="radio">` with `aria-checked`. Show the correct poster quarter using `pet.posterIndex` and `background-position-x` values `0%`, `33.333%`, `66.667%`, `100%`. Apply `--pet-color` and `--pet-strong` from metadata, a 2px selected outline, a check icon, localized name, and localized personality.

- [ ] **Step 4: Run component type validation**

Create `src/lib/components/onboarding/onboarding-components.test.ts`:

```ts
import '../../i18n';
import { mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { OFFICIAL_PETS } from '../../utils/onboarding';
import AgentConnectionRow from './AgentConnectionRow.svelte';
import PetAdoptionCard from './PetAdoptionCard.svelte';

const mounted: object[] = [];

function target(): HTMLDivElement {
  const node = document.createElement('div');
  document.body.appendChild(node);
  return node;
}

afterEach(async () => {
  for (const component of mounted.splice(0)) await unmount(component);
  document.body.innerHTML = '';
});

describe('onboarding choice components', () => {
  it('exposes and triggers pet radio selection', () => {
    const onSelect = vi.fn();
    mounted.push(mount(PetAdoptionCard, {
      target: target(),
      props: { pet: OFFICIAL_PETS[0], selected: true, onSelect },
    }));
    const radio = document.querySelector<HTMLButtonElement>('[role="radio"]');
    expect(radio?.getAttribute('aria-checked')).toBe('true');
    radio?.click();
    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  it.each([
    { available: false, status: 'idle' as const },
    { available: true, status: 'installing' as const },
  ])('disables unavailable or installing agent switches', ({ available, status }) => {
    mounted.push(mount(AgentConnectionRow, {
      target: target(),
      props: {
        id: 'claude', selected: false, available, status, error: '',
        onToggle: vi.fn(), onRetry: vi.fn(),
      },
    }));
    expect(document.querySelector('[role="switch"]')?.getAttribute('aria-disabled')).toBe('true');
  });
});
```

Run: `pnpm vitest run src/lib/components/onboarding/onboarding-components.test.ts`

Expected: the focused component accessibility tests pass.

- [ ] **Step 5: Run component type validation**

Run: `pnpm check`

Expected: Svelte check exits 0 with no accessibility errors from the three new components.

- [ ] **Step 6: Commit the reusable components**

```bash
git add src/lib/components/onboarding
git commit -m "feat: add onboarding choice components"
```

---

### Task 4: Replace the Onboarding Flow

**Files:**
- Replace: `src/lib/components/Onboarding.svelte`

**Interfaces:**
- Consumes: pure onboarding model, three child components, `invoke`, svelte-i18n
- Produces props:

```ts
interface OnboardingProps {
  open?: boolean;
  isWindows: boolean;
  onComplete: (result: OnboardingResult) => Promise<void> | void;
  onGithubSignIn?: () => Promise<GithubProfile>;
}
```

- [ ] **Step 1: Replace transient state and actions**

Use Svelte 5 runes for `step`, `shareTelemetry`, `selectedAgents`, `starterPetId`, `githubProfile`, `agentStatuses`, `agentErrors`, `completionError`, and `saving`. Reset draft state only on the closed-to-open transition. Implement `toggleAgent()` with `hookCommandForAgent()` and real `invoke()`; remove failed selections. Implement `complete()` to emit one `OnboardingResult` and keep the flow open when `onComplete` rejects.

- [ ] **Step 2: Implement the four step templates**

Welcome shows trust copy, time estimate, poster, and unchecked telemetry. GitHub shows either the supplied sign-in action or the disabled Friends-beta state plus Skip. Agents uses three `AgentConnectionRow` components and a pet-only path. Adopt renders the four cards in a radiogroup and disables the CTA until a selection exists.

- [ ] **Step 3: Implement the desktop visual system**

Create a centered shell with `width: min(960px, calc(100vw - 32px))`, `height: min(600px, calc(100vh - 32px))`, fixed header/footer, scrollable content, light tokens by default, and dark tokens under `@media (prefers-color-scheme: dark)`. Use the exact colors, radii, typography, shadows, and 160–220ms motion from the approved spec. Add a `@media (prefers-reduced-motion: reduce)` block that removes transform motion.

- [ ] **Step 4: Add focus management and keyboard navigation**

Bind each visible step heading, focus it after step changes with `tick()`, and keep Tab order header → content → secondary footer → primary footer. On the adoption radiogroup, Left/Right moves selection through `OFFICIAL_PETS`, and Enter triggers completion only when a pet is selected.

- [ ] **Step 5: Validate the replacement component**

Run: `pnpm check`

Expected: exit 0 with no TypeScript, Svelte, or accessibility errors.

- [ ] **Step 6: Commit the flow UI**

```bash
git add src/lib/components/Onboarding.svelte
git commit -m "feat: build four-step onboarding flow"
```

---

### Task 5: Main-Store Persistence Integration

**Files:**
- Modify: `src/lib/components/Main.svelte`
- Test: `src/lib/utils/onboarding.test.ts`

**Interfaces:**
- Consumes: `OnboardingResult`
- Produces: persisted telemetry, integration toggles, app mode, starter id, restored mini window

- [ ] **Step 1: Extend the model test with result semantics**

Add assertions that an empty selected-agent list produces `pet` mode and any non-empty list produces `coding` mode. Keep the starter id independent of mode.

- [ ] **Step 2: Replace `handleModeSelect` with `handleOnboardingComplete`**

The function must keep onboarding visible until all persistence calls finish. Persist `telemetryEnabled`, each integration toggle, `miniPetId` when a starter exists, and `appMode`. Only then set `showOnboarding = false`, close settings state, restore the mini window, and allow the existing mode effect to start polling. A thrown persistence error propagates back to `Onboarding.svelte` so it can show its footer error.

- [ ] **Step 3: Update the component invocation**

Define the same platform test used by Settings inside `Main.svelte`:

```ts
const isWindows = typeof navigator !== 'undefined' && navigator.userAgent.includes('Windows');
```

Replace `onSelect` with `onComplete={handleOnboardingComplete}` and pass `{isWindows}`. Do not pass `onGithubSignIn` in this milestone, ensuring the unavailable state is honest.

- [ ] **Step 4: Run focused and full tests**

Run:

```bash
pnpm vitest run src/lib/utils/onboarding.test.ts
pnpm test:ci
pnpm check
```

Expected: onboarding tests pass, all existing Vitest files pass, and Svelte check exits 0.

- [ ] **Step 5: Commit integration**

```bash
git add src/lib/components/Main.svelte src/lib/utils/onboarding.test.ts
git commit -m "feat: persist onboarding setup"
```

---

### Task 6: Visual QA and Final Verification

**Files:**
- Modify only if verification finds a defect: onboarding components or locale JSON

**Interfaces:**
- Consumes: completed onboarding implementation
- Produces: verified Screen 1 at required desktop sizes and themes

- [ ] **Step 1: Start the web preview**

Run: `pnpm dev --host 127.0.0.1`

Expected: Vite serves the app on port 1420.

- [ ] **Step 2: Capture and inspect required states**

Use the in-app browser to inspect Welcome, GitHub unavailable, Agent install states, and Adopt at `960 × 600`. Repeat Adopt at `760 × 600`, verify the 2 × 2 fallback, then inspect light, dark, English, Chinese, keyboard focus, and reduced motion.

- [ ] **Step 3: Correct only verified defects**

Fix clipping, contrast, focus, overflow, poster crop, misleading state, or inconsistent copy found in Step 2. Do not expand into other screens.

- [ ] **Step 4: Run fresh final verification**

Run:

```bash
pnpm test:ci
pnpm check
git diff --check
git status --short
```

Expected: all tests pass, Svelte check exits 0, diff check exits 0, and the worktree contains no unrelated modifications.

- [ ] **Step 5: Commit any QA corrections**

If Step 3 changed files:

```bash
git add src/lib/components/Onboarding.svelte \
  src/lib/components/onboarding \
  src/lib/i18n/en.json \
  src/lib/i18n/zh.json
git commit -m "fix: polish onboarding desktop states"
```

Expected: the branch is ready for final review and PR creation; do not push directly to `main`.
