# PawBae Svelte 5 Frontend Refactoring Plan

## Context

PawBae is a Svelte 5 + Tauri 2 desktop pet app (pure SPA, NOT SvelteKit). The codebase already uses Svelte 5 runes (`$state`, `$props`, `$derived`, `$effect`) exclusively — no Svelte 4 legacy. However, there are anti-patterns (listener cleanup race conditions, store boilerplate), architecture issues (1232-line SettingsPanel, flat directory), and convention gaps (camelCase utility filenames). This refactoring aligns the codebase with Svelte 5 best practices and community conventions.

**Total frontend: ~3,200 LOC across 22 files.**

---

## Phase 1: Fix `$effect` listener cleanup race conditions [P0 Bug]

### Problem
`listen()` (Tauri event API) returns `Promise<UnlistenFn>`. Current code pushes unlisten functions via `.then()` — if the `$effect` cleanup runs before promises resolve, old listeners leak.

### Fix: `disposed` flag pattern

A boolean flag `disposed` is set to `true` in cleanup. Any `listen()` that resolves after disposal immediately calls its own unlisten function.

### Files to edit

**`src/lib/Main.svelte` (lines 32-62)**
- Replace 4x `listen(...).then(u => cleanups.push(u))` with helper function using `disposed` flag
- Keep the existing cleanup calls for `agentStore.stopPolling()` and `sessionStore.stopPolling()`

**`src/lib/MascotView.svelte` (lines 44-57)**
- First `$effect`: hover tracking listener — replace `unlisten.then((u) => u())` in cleanup
- Apply `disposed` flag pattern for single listener

**`src/lib/MascotView.svelte` (lines 60-98)**
- Physics `$effect`: replace `const unlisteners: Promise<() => void>[]` array + `p.then((u) => u())` cleanup
- Apply `disposed` flag + `addListener` helper for 3 drag/throw listeners

**`src/lib/SettingsPanel.svelte` (lines 126-136)**
- Update progress listener — same pattern

### Verification
`pnpm tauri dev` → open/close settings repeatedly, toggle physics on/off, verify no duplicate event handlers in console.

---

## Phase 2: Refactor stores to class pattern [P0 Boilerplate + P2 $state.raw]

### Problem
All 5 stores use: module-level `$state` variables → exported object with manual `get x() { return x }` wrappers → individual setter functions. This produces ~150 lines of boilerplate. Svelte 5 class fields with `$state` are directly reactive through property access.

### Transform pattern

```typescript
// BEFORE (settings.svelte.ts — 238 lines)
let appMode = $state<AppMode | null>(null);
// ... 22 more $state declarations
async function setAppMode(mode: AppMode) {
  appMode = mode;
  await saveSetting('app_mode', mode);
}
// ... 22 more setter functions
export const settingsStore = {
  get appMode() { return appMode; },
  // ... 22 more getters
  setAppMode,
  // ... 22 more setter refs
};

// AFTER (~100 lines)
class SettingsStore {
  appMode = $state<AppMode | null>(null);
  soundEnabled = $state(true);
  // ... fields directly on class

  private storeInstance: Awaited<ReturnType<typeof load>> | null = null;

  async getStore() { ... }
  private async saveSetting(key: string, value: unknown) { ... }

  async setAppMode(mode: AppMode) {
    this.appMode = mode;
    await this.saveSetting('app_mode', mode);
  }
  // ... setters as methods
}
export const settingsStore = new SettingsStore();
```

**Key**: exported name `settingsStore` stays the same. All consumers use `settingsStore.appMode` (reads) and `settingsStore.setAppMode(v)` (writes) — both work identically with class pattern. **Zero consumer changes required.**

### Files to edit

| File | Current lines | Est. after | Notes |
|------|-------------|-----------|-------|
| `stores/settings.svelte.ts` | 238 | ~100 | 23 fields + persistence. `getStore()` stays public (used by SettingsPanel line 217) |
| `stores/window.svelte.ts` | 121 | ~70 | 6 fields + Tauri invoke methods |
| `stores/agents.svelte.ts` | 155 | ~100 | `agentRealIdMap`, `agentConnMap` stay as plain Map (not `$state`). `pollIntervals`, `fetchBusy`, `healthBusy` stay as plain instance fields |
| `stores/sessions.svelte.ts` | 86 | ~55 | `pollInterval`, `pollBusy` stay as plain fields |
| `stores/pet.svelte.ts` | 174 | ~130 | Constants (`HUNGER_MAX` etc.) stay as module-level exports outside class |

### Apply `$state.raw` during this phase

For arrays/objects that are always replaced wholesale (never mutated in-place):

| Store | Field | Reason |
|-------|-------|--------|
| agents | `agents`, `healthMap`, `allSessions` | Replaced via `= newAgents` / `= newMap` every poll cycle |
| sessions | `claudeSessions`, `claudeConversation`, `sessionNicknames` | Replaced via `= sessions` / `= conv` / spread |
| settings | `ocConnections`, `petQueue` | Replaced wholesale |

**Do NOT** use `$state.raw` for `petData` in pet.svelte.ts — it's read by individual properties (`petStore.petData.affection`) in Panel.svelte and needs deep reactivity.

### Verification
After each store: `pnpm tauri dev` → verify the feature controlled by that store works (settings persistence, agent polling, session display, pet actions, window state).

---

## Phase 3: Reorganize directory structure [P1/P2]

### Target structure
```
src/
├── main.ts                              (unchanged)
├── App.svelte                           (update import path)
├── vite-env.d.ts                        (unchanged)
└── lib/
    ├── components/
    │   ├── Main.svelte                  (moved from lib/)
    │   ├── MascotView.svelte            (moved)
    │   ├── MiniPetMascot.svelte         (moved)
    │   ├── SpritePet.svelte             (moved)
    │   ├── VoiceBubble.svelte           (moved)
    │   ├── Panel.svelte                 (moved)
    │   ├── Onboarding.svelte            (moved)
    │   ├── UpdateModal.svelte           (moved)
    │   └── settings/                    (new dir, for Phase 4)
    │       └── SettingsPanel.svelte     (moved)
    ├── utils/                           (new dir)
    │   ├── codex-pet.ts                 (renamed from codexPet.ts)
    │   ├── pet-physics.ts               (renamed from petPhysics.ts)
    │   └── edge-detect.ts              (renamed from edgeDetect.ts)
    ├── stores/                          (unchanged location)
    ├── types.ts                         (unchanged)
    ├── i18n.ts                          (unchanged)
    └── i18n/                            (unchanged)
```

### Execution order

#### Step 3a: Create directories
```
mkdir -p src/lib/components/settings
mkdir -p src/lib/utils
```

#### Step 3b: Rename utility files to kebab-case + move to utils/
```
git mv src/lib/codexPet.ts    src/lib/utils/codex-pet.ts
git mv src/lib/petPhysics.ts  src/lib/utils/pet-physics.ts
git mv src/lib/edgeDetect.ts  src/lib/utils/edge-detect.ts
```

Update internal cross-references within utils/:
- `pet-physics.ts`: `'./codexPet'` → `'./codex-pet'`, `'./edgeDetect'` → `'./edge-detect'`
- `edge-detect.ts`: `'./codexPet'` → `'./codex-pet'`

#### Step 3c: Move all component .svelte files
```
git mv src/lib/Main.svelte           src/lib/components/Main.svelte
git mv src/lib/MascotView.svelte     src/lib/components/MascotView.svelte
git mv src/lib/MiniPetMascot.svelte  src/lib/components/MiniPetMascot.svelte
git mv src/lib/SpritePet.svelte      src/lib/components/SpritePet.svelte
git mv src/lib/VoiceBubble.svelte    src/lib/components/VoiceBubble.svelte
git mv src/lib/Panel.svelte          src/lib/components/Panel.svelte
git mv src/lib/Onboarding.svelte     src/lib/components/Onboarding.svelte
git mv src/lib/UpdateModal.svelte    src/lib/components/UpdateModal.svelte
git mv src/lib/SettingsPanel.svelte  src/lib/components/settings/SettingsPanel.svelte
```

#### Step 3d: Update all import paths

Complete import path change map (from perspective of the importing file):

**`src/App.svelte`:**
- `'./lib/Main.svelte'` → `'./lib/components/Main.svelte'`

**`src/lib/components/Main.svelte`:**
- `'./SettingsPanel.svelte'` → `'./settings/SettingsPanel.svelte'`
- `'./codexPet'` → `'../utils/codex-pet'`
- `'./types'` → `'../types'`
- `'./stores/settings.svelte'` → `'../stores/settings.svelte'` (same for all 5 stores)
- Component-to-component imports (`./MascotView.svelte` etc.) stay unchanged (same directory)

**`src/lib/components/MascotView.svelte`:**
- `'./codexPet'` → `'../utils/codex-pet'`
- `'./petPhysics'` → `'../utils/pet-physics'`
- `'./stores/*'` → `'../stores/*'`
- Component imports (`./MiniPetMascot.svelte`, `./VoiceBubble.svelte`) stay unchanged

**`src/lib/components/MiniPetMascot.svelte`:**
- `'./codexPet'` → `'../utils/codex-pet'`

**`src/lib/components/SpritePet.svelte`:**
- `'./codexPet'` → `'../utils/codex-pet'`

**`src/lib/components/Panel.svelte`:**
- `'./stores/*'` → `'../stores/*'`

**`src/lib/components/Onboarding.svelte`:**
- `'./codexPet'` → `'../utils/codex-pet'`
- `'./types'` → `'../types'`

**`src/lib/components/settings/SettingsPanel.svelte`:**
- `'./stores/settings.svelte'` → `'../../stores/settings.svelte'`
- `'./stores/agents.svelte'` → `'../../stores/agents.svelte'`
- `'./types'` → `'../../types'`

**`src/lib/stores/*.svelte.ts`:** All import `'../types'` — unchanged (stores don't move).

**`src/main.ts`:** Imports `'./lib/i18n'` and `'./App.svelte'` — unchanged.

### Verification
`pnpm tauri dev` — any broken import path causes an immediate Vite compilation error with exact file + line.

---

## Phase 4: Split SettingsPanel into sub-components [P1]

### Approach

SettingsPanel (1232 lines) splits into a shell (~150 lines) + 7 sub-components. Each sub-component reads directly from stores (no prop drilling) for simple cases, or receives local state via props when state is component-scoped.

### CSS Strategy

Shared UI primitive styles (`.section`, `.card`, `.setting-row`, `.toggle`, `.segmented`, `.btn-small`, `.slider-wrap`, etc.) stay in the parent SettingsPanel.svelte but are scoped with `:global()` under `.settings-content`:

```css
.settings-content :global(.section) { ... }
.settings-content :global(.card) { ... }
```

This lets child components use these classes without duplicating CSS. Section-specific styles (`.conn-row`, `.mode-btn`, `.exit-btn`, etc.) move into their respective sub-components.

### Sub-components to create

All created in `src/lib/components/settings/`:

| File | Source lines | Local state | Stores accessed | Key logic |
|------|-------------|-------------|-----------------|-----------|
| `AppModeSection.svelte` | 251-276 | None | settingsStore | Mode switch buttons |
| `PetSettingsSection.svelte` | 278-338 | None | settingsStore | Mascot scale slider, SFX toggle, idle interval |
| `ConnectionsSection.svelte` | 341-415 | `connections`, `testingIdx`, `testResult` | settingsStore, agentStore | Connection CRUD, SSH test. Moves `syncConnections`, `updateConnection`, `deleteConnection`, `addConnection`, `testConnection` functions from parent |
| `IntegrationToggles.svelte` | 417-473 | `enableClaudeCode/Codex/Cursor`, `hookStatus`, `codexHookStatus`, `cursorHookStatus` | settingsStore | Claude Code/Codex/Cursor toggles + hook install. Moves `toggleClaudeCode`, `toggleCodex`, `toggleCursor` |
| `DisplaySection.svelte` | 476-533 | None | settingsStore | Auto-expand, panel height, hover delay, mascot scale sliders |
| `SoundSection.svelte` | 535-612 | None | settingsStore | Notification sound, CC/Codex/Cursor/waiting sound toggles, auto-close |
| `AboutSection.svelte` | 615-696 | `updateInfo`, `updateChecking`, `updateCheckResult/Msg`, `updating`, `updateProgress/Msg`, `updateRunResult/Msg` | settingsStore | Update check/download + language picker + exit button. Moves `checkForUpdate`, `runUpdate`, `resolveProgressText`, `changeLanguage`. Moves `$effect` for update-progress listener (current lines 126-136) |

### Rewritten SettingsPanel.svelte shell (~120 lines)

```svelte
<script lang="ts">
  import { settingsStore } from '../../stores/settings.svelte';
  import AppModeSection from './AppModeSection.svelte';
  import PetSettingsSection from './PetSettingsSection.svelte';
  import ConnectionsSection from './ConnectionsSection.svelte';
  import IntegrationToggles from './IntegrationToggles.svelte';
  import DisplaySection from './DisplaySection.svelte';
  import SoundSection from './SoundSection.svelte';
  import AboutSection from './AboutSection.svelte';

  let { open = false, onClose }: { open?: boolean; onClose: () => void } = $props();

  const isWindows = typeof navigator !== 'undefined' && navigator.userAgent.includes('Windows');
  const isPetMode = $derived(settingsStore.appMode === 'pet');
</script>

{#if open}
<div class="settings-overlay" onclick={onClose}>
  <div class="settings-panel" onclick={(e) => e.stopPropagation()}>
    <div class="settings-header">...</div>
    <div class="settings-scroll">
      <div class="settings-content">
        <AppModeSection />
        {#if isPetMode}
          <PetSettingsSection {isWindows} />
        {/if}
        {#if !isPetMode}
          <ConnectionsSection {open} />
          <IntegrationToggles {isWindows} />
          <DisplaySection {isWindows} />
          <SoundSection {isWindows} />
        {/if}
        <AboutSection {open} />
      </div>
    </div>
  </div>
</div>
{/if}
```

The `open` prop is passed to `ConnectionsSection` (to sync connections from store on open) and `AboutSection` (to trigger initial update check on open).

### Props patterns for sub-components

All sub-components that accept props use Svelte 5 `$props()` with interface types:

```typescript
// Example: IntegrationToggles.svelte
interface IntegrationTogglesProps {
  isWindows: boolean;
}
let { isWindows }: IntegrationTogglesProps = $props();
```

Sub-components that need `$_()` for i18n import it directly:
```typescript
import { _ } from 'svelte-i18n';
```

### Verification
Open settings in coding mode → verify all 7 sections render. Switch to pet mode → verify pet-only sections. Test: connection CRUD, hook toggles, all sliders/toggles, update check, language switch, exit button.

---

## Phase 5: Extract inline prop types [P2]

### Files to edit

Extract inline type annotations from `$props()` into named interfaces at the top of each component's `<script>` block:

| Component | Current pattern | Interface name |
|-----------|----------------|----------------|
| `MascotView.svelte` | Inline type in destructuring (lines 14-24) | `MascotViewProps` |
| `MiniPetMascot.svelte` | Inline type in destructuring (lines 5-25) | `MiniPetMascotProps` |
| `VoiceBubble.svelte` | Inline type in destructuring (lines 2-14) | `VoiceBubbleProps` |
| `Onboarding.svelte` | Inline type in destructuring (lines 7-13) | `OnboardingProps` |
| `UpdateModal.svelte` | Inline type in destructuring (lines 10-30) | `UpdateModalProps` |
| `Main.svelte` | No props (root) | — |
| `Panel.svelte` | Just `class?: string` | Leave inline (too simple) |
| `SpritePet.svelte` | Already has extracted `Props` interface | Already done |

### Move `UpdateModalInfo` from UpdateModal.svelte to types.ts

Currently exported via `export interface` in the `.svelte` script block. Main.svelte imports it as `import type { UpdateModalInfo } from './UpdateModal.svelte'`. Move to `types.ts` and update both import sites.

### Verification
`pnpm tauri dev` — TypeScript errors surface immediately.

---

## Phase 6: Minor cleanup [P2]

### 6a. Onboarding `$effect` for data loading (optional)

Current `$effect` in Onboarding.svelte (lines 17-21) uses effect for one-time async load triggered by prop. This is acceptable since it genuinely reacts to `open` changing. Mark as optional — the current code works and the guard `!previewPet` prevents double-loading.

### 6b. MiniPetMascot `hovering` variable

Line 30: `let hovering = false` is a non-reactive variable mutated inside `$effect`. It works correctly but is a subtle pattern. No change needed — the variable intentionally avoids triggering re-renders.

---

## Execution Summary

| Phase | What | Files changed | Est. lines changed | Risk |
|-------|------|--------------|-------------------|------|
| 1 | `$effect` cleanup fix | 3 files (Main, MascotView, SettingsPanel) | ~60 | Low |
| 2 | Store class refactor + `$state.raw` | 5 store files | ~300 (net -150) | Medium |
| 3 | Directory reorg + rename | All files (import paths) | ~80 | Medium |
| 4 | SettingsPanel split | 1 file → 8 files | ~100 new, -1100 from parent | Medium |
| 5 | Prop type extraction | 5 component files + types.ts | ~40 | Low |
| 6 | Optional minor cleanup | 1-2 files | ~10 | Low |

**Phases 1 and 2 can be done in parallel** (different files). Phases 3-6 are sequential.

### Git commit strategy
- Phase 1: `fix: resolve $effect listener cleanup race conditions`
- Phase 2: `refactor: convert stores to Svelte 5 class pattern with $state.raw`
- Phase 3: `refactor: reorganize frontend directory structure and rename to kebab-case`
- Phase 4: `refactor: split SettingsPanel into focused sub-components`
- Phase 5: `refactor: extract inline prop types to named interfaces`
- Phase 6: (fold into Phase 5 commit if done)

### Final verification
After all phases: `pnpm tauri dev` → full manual test of all features:
- Open/close settings panel
- Switch app mode (coding ↔ pet)
- Agent connection CRUD + test
- Hook installation toggles
- All display/sound sliders and toggles
- Update check
- Language switch
- Pet physics (drag, throw, hover)
- Voice bubble
- Panel expand/collapse
