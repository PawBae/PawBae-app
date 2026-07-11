# Social Home UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the approved `960 × 600` pet-first Social Home, make it the post-onboarding destination, and provide an honest browser preview for idle, visit-request, hosting, away, and shared-memory states.

**Design Spec:** `docs/superpowers/specs/2026-07-10-social-home-design.md`

**Architecture:** Keep remote social networking out of this slice. A pure `social-home.ts` model selects safe UI state, Svelte components render that state through callback props, `HomePreview.svelte` supplies demo-only fixtures, and `Main.svelte` supplies an honest empty production model until the platform service exists. Reuse the existing `set_mini_size({ restore: false })` full-window mode and restore the transparent mini stage only when the user chooses `Send to desktop`.

**Tech Stack:** Svelte 5 runes, TypeScript 5.6, svelte-i18n, Vitest + jsdom, Vite 6, Tauri 2; no new runtime or UI dependencies.

## Global Constraints

- The Home content shell is `960 × 600` at desktop size and remains usable inside the existing 85%-of-monitor Tauri full-window mode.
- The current pet is the strongest visual element; agent monitoring is a compact companion status, not a dashboard card.
- v1 supports mutual-friend visits, one visitor at a time, a fixed 30-minute lease representation, and one shared memory per completed visit.
- Production UI must not fabricate friends, visits, memories, account state, or public discovery; demo records exist only behind `import.meta.env.DEV && ?home-preview`.
- Plaza is labeled `Soon / 即将开放` and has no live people or activity.
- Friend-visible agent state is limited to `idle | working | waiting | compacting | offline`; task text, code, paths, prompts, and approval content never enter the social model.
- A pet has one location. An away local pet renders an empty nest and never renders a duplicate body at Home.
- Chinese and English are alternate locales, not simultaneous labels.
- Reuse `pawbae-onboarding-theme` with `system | light | dark`; official pet colors remain stable across themes.
- Official pet art may use the approved poster crop. Legacy skins use the first idle spritesheet frame and are never silently replaced by an official pet.
- All actions are keyboard reachable, focus is restored after slide-over close, and reduced motion removes positional movement.
- Do not modify the Rust window command in this UI slice; use the existing `set_mini_size` behavior.
- Preview scenarios never play notification sounds; visit-request/completion audio waits for the real event adapter so reconnects cannot duplicate it.

---

## File Structure

### Create

- `src/lib/utils/social-home.ts` — serializable UI model, privacy-safe enums, event priority, allowed actions, official/legacy pet helpers.
- `src/lib/utils/social-home.test.ts` — pure state, privacy, event priority, one-location, and action tests.
- `src/lib/components/home/HomePetArtwork.svelte` — official poster crop or legacy idle-frame renderer.
- `src/lib/components/home/PetIdentityCapsule.svelte` — pet identity, safe companion state, affection, and coins.
- `src/lib/components/home/SocialDock.svelte` — Friends, Plaza, Album, and active/soon states.
- `src/lib/components/home/HomeEventCard.svelte` — one prioritized request, return, or memory event.
- `src/lib/components/home/FriendsPanel.svelte` — requests, mutual-friend rows, visit/invite actions, handle search, and invite-link affordance.
- `src/lib/components/home/SharedAlbumPanel.svelte` — two-column private shared-memory cards.
- `src/lib/components/home/SocialHome.svelte` — shell, stage, local/visitor/away rendering, slide-over ownership, and action bar.
- `src/lib/components/home/home-components.test.ts` — mounted component behavior and accessibility contracts.
- `src/HomePreview.svelte` — dev-only scenario/pet/theme/locale preview with fixtures that never enter production.

### Modify

- `src/lib/utils/runtime.ts` — resolve `home-preview` alongside the current onboarding preview.
- `src/lib/utils/runtime.test.ts` — preview routing precedence and production gating.
- `src/main.ts` — mount `HomePreview` only in development.
- `src/lib/i18n/en.json` — complete `home.*` English copy.
- `src/lib/i18n/zh.json` — complete `home.*` Chinese copy.
- `src/lib/stores/window.svelte.ts` — add `homeOpen` and setters without changing mini-window command ownership.
- `src/lib/components/Main.svelte` — open Home after onboarding, derive local safe agent state, switch Home/Settings correctly, and restore mini on desktop transition.
- `src/lib/components/Panel.svelte` — add a Home entry next to skin/settings so existing users can reopen it.
- `src/lib/components/Onboarding.test.ts` or a new `src/lib/components/Main.home.test.ts` — verify source-level post-onboarding wiring where Tauri invocation prevents a simple jsdom integration mount.

---

### Task 1: Pure Social Home Model and Privacy Boundary

**Files:**
- Create: `src/lib/utils/social-home.ts`
- Create: `src/lib/utils/social-home.test.ts`

**Interfaces:**
- Consumes: `OfficialPetId` from `src/lib/utils/onboarding.ts`, plus `AgentActivity` and `mascotStateFor` from `src/lib/utils/agent-activity.ts` so Home shares the mascot's waiting/compacting/working precedence.
- Produces: `PublicAgentState`, `HomePetIdentity`, `HomePresence`, `FriendSummary`, `VisitRequest`, `SharedMemorySummary`, `SocialHomeModel`, `HomeEvent`, `selectHomeEvent(model)`, `allowedHomeActions(model)`, `isOfficialPetId(value)`, and `deriveLocalAgentState(enabled, activity, anyHealthActive)`.

- [ ] **Step 1: Write failing model tests**

```ts
import { describe, expect, it } from 'vitest';
import {
  allowedHomeActions,
  deriveLocalAgentState,
  isOfficialPetId,
  selectHomeEvent,
  type SocialHomeModel,
} from './social-home';

const base: SocialHomeModel = {
  localPet: { id: 'muru', name: 'Muru', officialPetId: 'muru' },
  presence: { kind: 'home', visitor: null },
  agentState: 'idle',
  affection: 86,
  coins: 140,
  togetherDays: 23,
  growthCurrent: 320,
  growthTarget: 500,
  friends: [],
  pendingVisit: null,
  latestMemory: null,
  memories: [],
};

describe('social Home model', () => {
  it('prioritizes an incoming visit over a memory card', () => {
    const model = {
      ...base,
      pendingVisit: {
        id: 'visit-1', ownerName: 'Momo', pet: { id: 'solu', name: 'Solu', officialPetId: 'solu' },
      },
      latestMemory: { id: 'memory-1', title: 'Rainy tea', dateLabel: 'Jul 10', petIds: ['muru', 'solu'] },
    } satisfies SocialHomeModel;
    expect(selectHomeEvent(model)?.kind).toBe('visit-request');
  });

  it('never renders the local pet body while away', () => {
    const model = { ...base, presence: { kind: 'away', friendName: 'Momo', endsAt: '16:30' } } satisfies SocialHomeModel;
    expect(allowedHomeActions(model)).toEqual(['view-visit', 'recall']);
  });

  it('accepts only official ids and derives only safe agent enums', () => {
    expect(isOfficialPetId('muru')).toBe(true);
    expect(isOfficialPetId('yoonie')).toBe(false);
    const quiet = { waiting: 0, compacting: 0, working: 0 };
    expect(deriveLocalAgentState(false, quiet, false)).toBe('offline');
    expect(deriveLocalAgentState(true, { ...quiet, waiting: 1 }, true)).toBe('waiting');
    expect(deriveLocalAgentState(true, { ...quiet, compacting: 1 }, false)).toBe('compacting');
    expect(deriveLocalAgentState(true, quiet, true)).toBe('working');
    expect(deriveLocalAgentState(true, quiet, false)).toBe('idle');
  });
});
```

- [ ] **Step 2: Run the focused test and verify the red state**

Run: `npm test -- --run src/lib/utils/social-home.test.ts`

Expected: FAIL because `./social-home` does not exist.

- [ ] **Step 3: Implement the minimal typed model and selectors**

```ts
import { mascotStateFor, type AgentActivity } from './agent-activity';
import type { OfficialPetId } from './onboarding';

export type PublicAgentState = 'idle' | 'working' | 'waiting' | 'compacting' | 'offline';
export type HomePanel = 'friends' | 'plaza' | 'album' | null;
export type HomeAction =
  | 'feed' | 'gift' | 'diary' | 'send-to-desktop'
  | 'play' | 'snack' | 'photo' | 'end-visit'
  | 'view-visit' | 'recall';

export interface HomePetIdentity {
  id: string;
  name: string;
  officialPetId?: OfficialPetId;
  ownerName?: string;
}

export type HomePresence =
  | { kind: 'home'; visitor: HomePetIdentity | null; visitorOwnerName?: string; endsAt?: string }
  | { kind: 'away'; friendName: string; endsAt: string };

export interface FriendSummary {
  id: string;
  displayName: string;
  handle: string;
  pet: HomePetIdentity;
  availability: 'available' | 'visiting' | 'away' | 'offline';
  publicAgentState: PublicAgentState;
}

export interface VisitRequest { id: string; ownerName: string; pet: HomePetIdentity }
export interface SharedMemorySummary { id: string; title: string; dateLabel: string; petIds: string[] }

export interface SocialHomeModel {
  localPet: HomePetIdentity;
  presence: HomePresence;
  agentState: PublicAgentState;
  affection: number;
  coins: number;
  togetherDays: number;
  growthCurrent: number;
  growthTarget: number;
  friends: FriendSummary[];
  pendingVisit: VisitRequest | null;
  latestMemory: SharedMemorySummary | null;
  memories: SharedMemorySummary[];
}

export type HomeEvent =
  | { kind: 'visit-request'; request: VisitRequest }
  | { kind: 'memory-ready'; memory: SharedMemorySummary }
  | { kind: 'invite-friend' };

const OFFICIAL_IDS = new Set<OfficialPetId>(['solu', 'muru', 'riffi', 'luma']);

export function isOfficialPetId(value: string): value is OfficialPetId {
  return OFFICIAL_IDS.has(value as OfficialPetId);
}

export function deriveLocalAgentState(
  enabled: boolean,
  activity: AgentActivity,
  anyHealthActive: boolean,
): PublicAgentState {
  if (!enabled) return 'offline';
  return mascotStateFor(activity, anyHealthActive);
}

export function selectHomeEvent(model: SocialHomeModel): HomeEvent | null {
  if (model.pendingVisit) return { kind: 'visit-request', request: model.pendingVisit };
  if (model.latestMemory) return { kind: 'memory-ready', memory: model.latestMemory };
  if (model.friends.length === 0) return { kind: 'invite-friend' };
  return null;
}

export function allowedHomeActions(model: SocialHomeModel): HomeAction[] {
  if (model.presence.kind === 'away') return ['view-visit', 'recall'];
  if (model.presence.visitor) return ['play', 'snack', 'photo', 'end-visit'];
  return ['feed', 'gift', 'diary', 'send-to-desktop'];
}
```

- [ ] **Step 4: Run the focused test and type-check the model**

Run: `npm test -- --run src/lib/utils/social-home.test.ts && npm run check`

Expected: the focused test passes; `svelte-check` reports no new errors.

- [ ] **Step 5: Commit the model boundary**

```bash
git add src/lib/utils/social-home.ts src/lib/utils/social-home.test.ts
git commit -m "feat: add social home state model"
```

---

### Task 2: Home Shell, Pet Artwork, Theme, and Idle/Away Stage

**Files:**
- Create: `src/lib/components/home/HomePetArtwork.svelte`
- Create: `src/lib/components/home/PetIdentityCapsule.svelte`
- Create: `src/lib/components/home/SocialDock.svelte`
- Create: `src/lib/components/home/HomeEventCard.svelte`
- Create: `src/lib/components/home/SocialHome.svelte`
- Create: `src/lib/components/home/home-components.test.ts`
- Modify: `src/lib/i18n/en.json`
- Modify: `src/lib/i18n/zh.json`

**Interfaces:**
- Consumes: all Task 1 model types, `CodexPet`, `tileFrameStyle`, `OnboardingTheme`, and `ONBOARDING_THEME_STORAGE_KEY`.
- Produces: `SocialHome` props `{ open, model, legacyPet, theme, onThemeChange, onSendToDesktop, onOpenSettings, onPetAction?, onInviteFriend?, onAcceptVisit?, onDelayVisit?, onOpenMemory? }`.

- [ ] **Step 1: Add failing mounted tests for the shell and one-location invariant**

```ts
import '../../i18n';
import { mount, tick, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import SocialHome from './SocialHome.svelte';
import type { FriendSummary, HomeAction, SocialHomeModel } from '../../utils/social-home';

const base: SocialHomeModel = {
  localPet: { id: 'muru', name: 'Muru', officialPetId: 'muru' },
  presence: { kind: 'home', visitor: null }, agentState: 'working',
  affection: 86, coins: 140, togetherDays: 23, growthCurrent: 320, growthTarget: 500,
  friends: [], pendingVisit: null, latestMemory: null, memories: [],
};
let component: object | null = null;
afterEach(async () => { if (component) await unmount(component); component = null; document.body.innerHTML = ''; });

const momoFriend: FriendSummary = {
  id: 'momo', displayName: 'Momo', handle: '@momoco',
  pet: { id: 'solu', name: 'Solu', officialPetId: 'solu', ownerName: 'Momo' },
  availability: 'available', publicAgentState: 'working',
};

type HomeCallbackOverrides = {
  onAcceptVisit?: (id: string) => void;
  onDelayVisit?: (id: string) => void;
  onInviteFriend?: (id: string) => void;
  onOpenMemory?: (id: string) => void;
  onPetAction?: (action: Exclude<HomeAction, 'send-to-desktop'>) => void;
};

function mountHome(model: SocialHomeModel, overrides: HomeCallbackOverrides = {}) {
  return mount(SocialHome, { target: document.body, props: {
    open: true, model, legacyPet: null, theme: 'light', onThemeChange: vi.fn(),
    onSendToDesktop: vi.fn(), onOpenSettings: vi.fn(), ...overrides,
  }});
}

describe('SocialHome', () => {
  it('renders the pet-first shell and desktop transition', async () => {
    const onSendToDesktop = vi.fn();
    component = mount(SocialHome, { target: document.body, props: {
      open: true, model: base, legacyPet: null, theme: 'light', onThemeChange: vi.fn(),
      onSendToDesktop, onOpenSettings: vi.fn(),
    }});
    await tick();
    expect(document.querySelector('[data-home-shell]')).not.toBeNull();
    expect(document.querySelector('[data-pet-id="muru"]')).not.toBeNull();
    document.querySelector<HTMLButtonElement>('[data-action="send-to-desktop"]')?.click();
    expect(onSendToDesktop).toHaveBeenCalledOnce();
  });

  it('shows an empty nest instead of the local pet while away', async () => {
    component = mount(SocialHome, { target: document.body, props: {
      open: true, model: { ...base, presence: { kind: 'away', friendName: 'Momo', endsAt: '16:30' } },
      legacyPet: null, theme: 'dark', onThemeChange: vi.fn(), onSendToDesktop: vi.fn(),
      onOpenSettings: vi.fn(),
    }});
    await tick();
    expect(document.querySelector('[data-away-state]')).not.toBeNull();
    expect(document.querySelector('[data-local-pet]')).toBeNull();
  });
});
```

- [ ] **Step 2: Run the component test and verify the red state**

Run: `npm test -- --run src/lib/components/home/home-components.test.ts`

Expected: FAIL because the Home components do not exist.

- [ ] **Step 3: Add complete `home.*` locale trees**

Add the same key shape to both locale files:

```json
{
  "home": {
    "title": "PawBae Home",
    "nav": { "friends": "Friends", "plaza": "Plaza", "album": "Album", "soon": "Soon" },
    "status": { "idle": "Keeping you company", "working": "Keeping you company while you code", "waiting": "Waiting for your reply", "compacting": "Tidying memories", "offline": "Your agent is resting" },
    "actions": { "feed": "Feed", "gift": "Gift", "diary": "Diary", "sendDesktop": "Send {pet} to desktop", "play": "Play together", "snack": "Offer a snack", "photo": "Take a photo", "endVisit": "End visit", "recall": "Recall", "viewVisit": "View visit" },
    "away": "{pet} is visiting {friend}",
    "empty": { "title": "Invite a friend to meet {pet}", "body": "Friends beta opens after account connection." },
    "privacy": "Shared memories stay between the two of you.",
    "settings": "Settings",
    "theme": { "system": "System", "light": "Light", "dark": "Dark" }
  }
}
```

Use natural Chinese equivalents in `zh.json`, including `把{pet}放到桌面`, `邀请好友来认识{pet}`, and `共同记忆只属于你们两位`.

- [ ] **Step 4: Implement artwork and structural components**

`HomePetArtwork.svelte` must choose exactly one visual source:

```svelte
{#if pet.officialPetId}
  <div class="official-art" data-pet-id={pet.officialPetId} style={`--poster-x:${positions[pet.officialPetId]}`} />
{:else if legacyPet}
  <div class="legacy-art" data-pet-id={pet.id} style={tileFrameStyle(legacyPet, size)} />
{:else}
  <div class="missing-art" aria-hidden="true">✦</div>
{/if}
```

Use poster positions matching `PetAdoptionCard.svelte`; do not substitute Muru for a legacy pet.

`SocialHome.svelte` must:

```svelte
<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { allowedHomeActions, selectHomeEvent, type HomeAction } from '../../utils/social-home';

  const actionKeys: Record<HomeAction, string> = {
    feed: 'feed', gift: 'gift', diary: 'diary', 'send-to-desktop': 'sendDesktop',
    play: 'play', snack: 'snack', photo: 'photo', 'end-visit': 'endVisit',
    'view-visit': 'viewVisit', recall: 'recall',
  };

  function runAction(action: HomeAction) {
    if (action === 'send-to-desktop') onSendToDesktop();
    else onPetAction?.(action);
  }
</script>

{#if open}
  <section class="home-overlay" data-theme={resolvedTheme} aria-label={$_('home.title')}>
    <div class="home-shell" data-home-shell>
      <PetIdentityCapsule {model} />
      <SocialDock bind:activePanel />
      <main class="living-room">
        {#if model.presence.kind === 'away'}
          <div data-away-state class="empty-nest">
            <span aria-hidden="true">⌁</span>
            <p>{$_('home.away', { values: { pet: model.localPet.name, friend: model.presence.friendName } })}</p>
          </div>
        {:else}
          <div data-local-pet><HomePetArtwork pet={model.localPet} {legacyPet} size={240} /></div>
          {#if model.presence.visitor}<HomePetArtwork pet={model.presence.visitor} legacyPet={null} size={190} guest />{/if}
        {/if}
      </main>
      <HomeEventCard
        event={selectHomeEvent(model)}
        onAcceptVisit={onAcceptVisit}
        onDelayVisit={onDelayVisit}
        onOpenMemory={onOpenMemory}
      />
      <footer class="home-actions">
        {#each allowedHomeActions(model) as action}
          <button data-action={action} disabled={action !== 'send-to-desktop' && !onPetAction} onclick={() => runAction(action)}>
            {$_(`home.actions.${actionKeys[action]}`, { values: { pet: model.localPet.name } })}
          </button>
        {/each}
      </footer>
    </div>
  </section>
{/if}
```

Implement the approved neutral tokens as CSS custom properties, `max-width: 960px`, `height: min(600px, calc(100vh - 24px))`, stage-centered open canvas, right pill dock, 24px slide-over radius, pet ambient light, `prefers-reduced-motion`, and visible `:focus-visible` rings.

- [ ] **Step 5: Run the focused tests and Svelte check**

Run: `npm test -- --run src/lib/components/home/home-components.test.ts && npm run check`

Expected: focused tests pass and `svelte-check` reports no new errors.

- [ ] **Step 6: Commit the Home shell**

```bash
git add src/lib/components/home src/lib/i18n/en.json src/lib/i18n/zh.json
git commit -m "feat: build pet-first social home shell"
```

---

### Task 3: Friends Panel, Visit Requests, and Hosting State

**Files:**
- Create: `src/lib/components/home/FriendsPanel.svelte`
- Modify: `src/lib/components/home/SocialHome.svelte`
- Modify: `src/lib/components/home/HomeEventCard.svelte`
- Modify: `src/lib/components/home/home-components.test.ts`
- Modify: `src/lib/i18n/en.json`
- Modify: `src/lib/i18n/zh.json`

**Interfaces:**
- Consumes: `FriendSummary`, `VisitRequest`, `HomePresence`, and callbacks defined by Task 2.
- Produces: accessible Friends slide-over with `onInviteFriend(friendId)`, `onAcceptVisit(requestId)`, `onDelayVisit(requestId)`, and `onPetAction(action)` for hosting controls.

- [ ] **Step 1: Add failing request, panel, and guest-tag tests**

```ts
it('accepts a visit from the contextual event card', async () => {
  const onAcceptVisit = vi.fn();
  const model = { ...base, pendingVisit: {
    id: 'visit-1', ownerName: 'Momo', pet: { id: 'solu', name: 'Solu', officialPetId: 'solu' },
  }} satisfies SocialHomeModel;
  component = mountHome(model, { onAcceptVisit });
  await tick();
  document.querySelector<HTMLButtonElement>('[data-action="accept-visit"]')?.click();
  expect(onAcceptVisit).toHaveBeenCalledWith('visit-1');
});

it('opens Friends without removing the visible stage', async () => {
  component = mountHome({ ...base, friends: [momoFriend] });
  document.querySelector<HTMLButtonElement>('[data-dock="friends"]')?.click();
  await tick();
  expect(document.querySelector('[data-panel="friends"]')).not.toBeNull();
  expect(document.querySelector('.living-room')).not.toBeNull();
});

it('labels a hosted pet with its owner', async () => {
  const model = { ...base, presence: { kind: 'home', visitor: momoFriend.pet, visitorOwnerName: 'Momo', endsAt: '16:30' } } satisfies SocialHomeModel;
  component = mountHome(model);
  await tick();
  expect(document.querySelector('[data-guest-tag]')?.textContent).toContain('Momo');
});
```

- [ ] **Step 2: Run the focused test and verify it fails for missing behavior**

Run: `npm test -- --run src/lib/components/home/home-components.test.ts`

Expected: FAIL on request action, Friends panel, and guest tag assertions.

- [ ] **Step 3: Implement Friends and visit UI**

`FriendsPanel.svelte` must render:

- pending friend requests before the list;
- mutual-friend rows with display name, handle, pet identity, availability text, and one contextual action;
- an honest empty/account-beta state when `friends.length === 0`;
- handle search and invite-link affordances disabled with explanatory copy until account capability is provided;
- a close button with `aria-label`, Escape support, initial heading focus, and return-focus behavior owned by `SocialHome`.

Use callbacks only; do not mutate a social store or manufacture success. During hosting, render two pet slots, a guest tag, remaining-time label, and only `play`, `snack`, `photo`, and `end-visit` actions.

- [ ] **Step 4: Add both locale copies and run verification**

Add `home.friends.*`, `home.visit.*`, and `home.guestTag` to both locale files. Then run:

`npm test -- --run src/lib/components/home/home-components.test.ts && npm run check`

Expected: focused tests pass; no new Svelte errors.

- [ ] **Step 5: Commit the friend and visit slice**

```bash
git add src/lib/components/home src/lib/i18n/en.json src/lib/i18n/zh.json
git commit -m "feat: add social home visiting surfaces"
```

---

### Task 4: Shared Album and Honest Plaza Future State

**Files:**
- Create: `src/lib/components/home/SharedAlbumPanel.svelte`
- Modify: `src/lib/components/home/SocialHome.svelte`
- Modify: `src/lib/components/home/SocialDock.svelte`
- Modify: `src/lib/components/home/home-components.test.ts`
- Modify: `src/lib/i18n/en.json`
- Modify: `src/lib/i18n/zh.json`

**Interfaces:**
- Consumes: `SharedMemorySummary[]`, `onOpenMemory(memoryId)`, and `HomePanel`.
- Produces: private two-column memory grid and a non-fabricated Plaza `Soon` panel.

- [ ] **Step 1: Add failing Album and Plaza honesty tests**

```ts
it('opens a private memory from the Album panel', async () => {
  const onOpenMemory = vi.fn();
  component = mountHome({ ...base, memories: [{ id: 'memory-1', title: 'Rainy tea', dateLabel: 'Jul 10', petIds: ['muru', 'solu'] }] }, { onOpenMemory });
  document.querySelector<HTMLButtonElement>('[data-dock="album"]')?.click();
  await tick();
  document.querySelector<HTMLButtonElement>('[data-memory-id="memory-1"]')?.click();
  expect(onOpenMemory).toHaveBeenCalledWith('memory-1');
  expect(document.querySelector('[data-album-privacy]')).not.toBeNull();
});

it('marks Plaza as future work without fake profiles', async () => {
  component = mountHome(base);
  document.querySelector<HTMLButtonElement>('[data-dock="plaza"]')?.click();
  await tick();
  expect(document.querySelector('[data-panel="plaza"]')?.textContent).toContain('Soon');
  expect(document.querySelector('[data-plaza-profile]')).toBeNull();
});
```

- [ ] **Step 2: Run the focused test and verify the red state**

Run: `npm test -- --run src/lib/components/home/home-components.test.ts`

Expected: FAIL because Album and Plaza panel bodies are missing.

- [ ] **Step 3: Implement Album cards and Plaza state**

Use a two-column grid at `360–384px` panel width. Each memory button contains an art swatch, title, date, and participating pet marks. The panel subtitle is the localized privacy statement. When empty, show `No shared memories yet / 还没有共同记忆` and point toward a first mutual-friend visit without rewards or streak pressure.

The Plaza panel contains only a calm illustration mark, `Coming later / 稍后开放`, and one sentence explaining that safe discovery is still being designed. Do not render handles, avatars, online dots, counts, or cards.

- [ ] **Step 4: Add locale copy and verify**

Run: `npm test -- --run src/lib/components/home/home-components.test.ts && npm run check`

Expected: all Home component tests pass; no new Svelte errors.

- [ ] **Step 5: Commit Album and Plaza**

```bash
git add src/lib/components/home src/lib/i18n/en.json src/lib/i18n/zh.json
git commit -m "feat: add shared album and plaza future state"
```

---

### Task 5: Development Preview and High-Fidelity Scenario Controls

**Files:**
- Create: `src/HomePreview.svelte`
- Modify: `src/lib/utils/runtime.ts`
- Modify: `src/lib/utils/runtime.test.ts`
- Modify: `src/main.ts`

**Interfaces:**
- Consumes: `SocialHome`, Task 1 models, and query parameters.
- Produces: `resolveDevPreview(isDev, search): 'onboarding' | 'home' | null` and `?home-preview&lang=zh&theme=light&state=hosting&pet=muru`.

- [ ] **Step 1: Replace preview detection tests with a failing resolver contract**

```ts
import { resolveDevPreview } from './runtime';

it('resolves one explicit development preview', () => {
  expect(resolveDevPreview(true, '?onboarding-preview')).toBe('onboarding');
  expect(resolveDevPreview(true, '?home-preview')).toBe('home');
  expect(resolveDevPreview(true, '?home-preview&onboarding-preview')).toBe('home');
  expect(resolveDevPreview(false, '?home-preview')).toBeNull();
  expect(resolveDevPreview(true, '')).toBeNull();
});
```

- [ ] **Step 2: Run runtime tests and verify the red state**

Run: `npm test -- --run src/lib/utils/runtime.test.ts`

Expected: FAIL because `resolveDevPreview` is not exported.

- [ ] **Step 3: Implement resolver and mount routing**

```ts
export type DevPreview = 'onboarding' | 'home' | null;
export function resolveDevPreview(isDev: boolean, search: string): DevPreview {
  if (!isDev) return null;
  const params = new URLSearchParams(search);
  if (params.has('home-preview')) return 'home';
  if (params.has('onboarding-preview')) return 'onboarding';
  return null;
}
```

In `main.ts`, resolve once and mount `HomePreview`, `OnboardingPreview`, `StageApp`, or `App` in that order. Keep stage routing based on the Tauri window label and do not mount `Main` in preview mode.

- [ ] **Step 4: Implement demo-only scenario fixtures**

`HomePreview.svelte` reads and validates:

- `lang=zh|en`;
- `theme=system|light|dark`;
- `pet=solu|muru|riffi|luma`;
- `state=idle|working|waiting|compacting|request|hosting|away|memory|offline`.

It may provide a compact preview toolbar outside the `960 × 600` shell. Scenario callbacks update only local preview state. Use realistic Momo/Solu data for request/hosting, but guard the entire preview through `import.meta.env.DEV` routing so no fixture is imported by production `Main`.

- [ ] **Step 5: Verify preview routing and production exclusion**

Run: `npm test -- --run src/lib/utils/runtime.test.ts && npm run check && npm run build`

Expected: runtime tests pass; check and build exit `0`.

- [ ] **Step 6: Commit the preview**

```bash
git add src/HomePreview.svelte src/lib/utils/runtime.ts src/lib/utils/runtime.test.ts src/main.ts
git commit -m "feat: add social home browser preview"
```

---

### Task 6: Post-Onboarding and Mini-Window Integration

**Files:**
- Modify: `src/lib/stores/window.svelte.ts`
- Modify: `src/lib/components/Main.svelte`
- Modify: `src/lib/components/Panel.svelte`
- Create: `src/lib/components/Main.home.test.ts`

**Interfaces:**
- Consumes: `SocialHome`, current `settingsStore`, `skinsStore`, `petStore`, `agentStore`, and Tauri `set_mini_size`.
- Produces: `windowStore.homeOpen`, `setHomeOpen(value)`, `openHome()`, `sendPetToDesktop()`, and settings return-to-Home behavior.

- [ ] **Step 1: Add failing source-wiring tests for the Tauri integration seam**

```ts
import { describe, expect, it } from 'vitest';
import mainSource from './Main.svelte?raw';
import panelSource from './Panel.svelte?raw';
import windowSource from '../stores/window.svelte.ts?raw';

describe('Social Home window flow', () => {
  it('opens Home after onboarding without restoring mini immediately', () => {
    const completion = mainSource.slice(mainSource.indexOf('async function handleOnboardingComplete'), mainSource.indexOf('$effect(() =>', mainSource.indexOf('async function handleOnboardingComplete')));
    expect(completion).toContain('setHomeOpen(true)');
    expect(completion).not.toContain("restore: true");
  });

  it('restores mini only from the desktop transition', () => {
    expect(mainSource).toMatch(/async function sendPetToDesktop[\s\S]*restore: true/);
  });

  it('exposes Home from the expanded mini panel', () => {
    expect(panelSource).toContain('data-action="open-home"');
    expect(windowSource).toContain('homeOpen = $state(false)');
  });
});
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `npm test -- --run src/lib/components/Main.home.test.ts`

Expected: FAIL because Home window state and handlers are absent.

- [ ] **Step 3: Add Home window state and handlers**

In `window.svelte.ts` add:

```ts
homeOpen = $state(false);
setHomeOpen(v: boolean) { this.homeOpen = v; }
```

In `Main.svelte`:

- derive `SocialHomeModel` from the selected pet, local pet stats, and safe agent enum;
- use `friends: []`, `pendingVisit: null`, and `memories: []` in production until a real adapter exists;
- after `handleOnboardingComplete`, set onboarding false, settings false, Home true, and do not invoke `restore: true`;
- `openHome()` closes the expanded mini panel, sets Home true, and invokes `set_mini_size({ restore: false, keepOnTop: false })`;
- `sendPetToDesktop()` sets Home false and invokes `set_mini_size({ restore: true, mascotScale })`;
- opening Settings from Home records `returnToHomeAfterSettings = true`; closing Settings restores Home without resizing; Settings opened from mini retains the current restore behavior;
- hide or inert the mini `MascotView` and `Panel` while Home is open so duplicate interactive pets are not exposed to accessibility APIs.

Mount:

```svelte
<SocialHome
  open={windowStore.homeOpen}
  model={homeModel}
  legacyPet={pet}
  theme={homeTheme}
  onThemeChange={setHomeTheme}
  onSendToDesktop={sendPetToDesktop}
  onOpenSettings={openSettingsFromHome}
/>
```

Empty callbacks must leave controls disabled or show honest unavailable copy; never show a success state with no service behind it.

- [ ] **Step 4: Add the mini-panel Home entry**

Add a `⌂` button before skin/settings with `data-action="open-home"`, localized title, and an `openHome` function that collapses the mini panel, sets `homeOpen`, and requests full-window size. Prevent simultaneous Home and Settings surfaces.

- [ ] **Step 5: Run integration-focused and full frontend verification**

Run: `npm test -- --run src/lib/components/Main.home.test.ts src/lib/components/home/home-components.test.ts src/lib/utils/social-home.test.ts src/lib/utils/runtime.test.ts && npm run check && npm run build`

Expected: all focused tests pass; check and build exit `0` with no new warnings attributable to Social Home.

- [ ] **Step 6: Commit the integrated flow**

```bash
git add src/lib/stores/window.svelte.ts src/lib/components/Main.svelte src/lib/components/Panel.svelte src/lib/components/Main.home.test.ts
git commit -m "feat: open social home after onboarding"
```

---

### Task 7: Browser QA, Accessibility, and Final Regression Gate

**Files:**
- Modify: `src/lib/components/home/*.svelte`
- Modify: `src/HomePreview.svelte`
- Modify: `src/lib/i18n/en.json`
- Modify: `src/lib/i18n/zh.json`

**Interfaces:**
- Consumes: the completed preview and production integration.
- Produces: verified light/dark, bilingual, keyboard, reduced-motion, and state coverage without adding new product scope.

- [ ] **Step 1: Run the complete automated baseline**

Run: `npm run test:ci && npm run check && npm run build`

Expected: all Vitest files pass; `svelte-check` has no new errors; Vite build exits `0`.

- [ ] **Step 2: Inspect every approved preview state at desktop size**

Open and capture these exact URLs at a viewport large enough to show the `960 × 600` shell:

```text
http://127.0.0.1:1420/?home-preview&lang=zh&theme=light&state=idle&pet=muru
http://127.0.0.1:1420/?home-preview&lang=en&theme=dark&state=request&pet=solu
http://127.0.0.1:1420/?home-preview&lang=zh&theme=dark&state=hosting&pet=riffi
http://127.0.0.1:1420/?home-preview&lang=en&theme=light&state=away&pet=luma
http://127.0.0.1:1420/?home-preview&lang=zh&theme=light&state=memory&pet=muru
http://127.0.0.1:1420/?home-preview&lang=en&theme=dark&state=offline&pet=muru
```

For each, verify: no clipping; pet remains dominant; one contextual event maximum; dock does not cover actions; guest tag is clear; away state has no local pet; Plaza has no fake profiles; Chinese and English fit.

- [ ] **Step 3: Complete keyboard and reduced-motion checks**

Using only the keyboard: move through theme, settings, dock, event actions, and bottom actions; open and close Friends/Album/Plaza; confirm focus returns to the invoking dock item; confirm Escape closes a slide-over. With reduced motion enabled, confirm visitor and panel translations are removed and state feedback remains visible.

- [ ] **Step 4: Fix only observed issues and add a regression assertion for each**

For every issue, first add a failing Vitest assertion that names the missing contract, run it to confirm failure, make the smallest component/CSS/copy change, and rerun the focused file. Do not add new Home features in this task.

- [ ] **Step 5: Run the final fresh verification**

Run: `npm run test:ci && npm run check && npm run build && git diff --check`

Expected: all tests pass; no new Svelte errors; build exits `0`; `git diff --check` prints nothing.

- [ ] **Step 6: Commit verified polish**

```bash
git add src/HomePreview.svelte src/lib/components/home src/lib/i18n/en.json src/lib/i18n/zh.json
git commit -m "fix: polish social home states and accessibility"
```

---

## Deferred Follow-up Plans

These are intentionally separate because each crosses a different product or infrastructure boundary:

1. Account and mutual-friend backend: GitHub/PawBae identity, requests, invite links, mute, unfriend, and block.
2. Visit lease and Realtime adapter: `requested → accepted → traveling → visiting → returning → completed`, unique pet location, reconnect, expiry, and authorization.
3. Shared-memory persistence and Polaroid detail: idempotent settlement, two-pet ownership, local hide/delete, and safe template rendering.
4. Plaza discovery: separate privacy, moderation, ranking, and stranger-safety design before any live UI.
5. Social event audio: request/completion deduplication, global sound preference, reconnect suppression, and platform-specific playback.
