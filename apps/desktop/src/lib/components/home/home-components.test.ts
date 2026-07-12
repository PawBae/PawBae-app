import '../../i18n';
import { mount, tick, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CodexPet } from '../../utils/codex-pet';
import { ONBOARDING_THEME_STORAGE_KEY } from '../../utils/onboarding-theme';
import type { FriendSummary, SocialHomeModel } from '../../utils/social-home';
import friendsPanelSource from './FriendsPanel.svelte?raw';
import homeEventCardSource from './HomeEventCard.svelte?raw';
import homePanelShellSource from './HomePanelShell.svelte?raw';
import homePetArtworkSource from './HomePetArtwork.svelte?raw';
import socialDockSource from './SocialDock.svelte?raw';
import SocialHome from './SocialHome.svelte';
import socialHomeSource from './SocialHome.svelte?raw';

const base: SocialHomeModel = {
  localPet: { id: 'muru', name: 'Muru', officialPetId: 'muru' },
  presence: { kind: 'home', visitor: null },
  agentState: 'working',
  realtimeState: 'connected',
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

const momoFriend: FriendSummary = {
  id: 'friend-momo',
  displayName: 'Momo',
  handle: '@momo',
  pet: { id: 'solu', name: 'Solu', officialPetId: 'solu' },
  availability: 'available',
  publicAgentState: 'idle',
  visitDirection: 'visit-them',
};

let component: object | null = null;
let originalLocalStorage: PropertyDescriptor | undefined;

const legacyPet: CodexPet = {
  id: 'doro',
  displayName: 'Doro',
  description: 'A legacy pet',
  spritesheetUrl: 'codexpet://localhost/doro/spritesheet.png',
  atlas: { cellW: 192, cellH: 208, cols: 8, rows: 9 },
  animations: { idle: { row: 2, frames: 6 } },
  stateMap: { idle: 'idle', working: 'idle', compacting: 'idle', waiting: 'idle' },
  oneShot: new Set(),
  imageRendering: 'pixelated',
};

afterEach(async () => {
  if (component) await unmount(component);
  component = null;
  document.body.innerHTML = '';
  if (originalLocalStorage) {
    Object.defineProperty(globalThis, 'localStorage', originalLocalStorage);
    originalLocalStorage = undefined;
  }
});

function memoryStorage(): Storage {
  const entries = new Map<string, string>();
  return {
    get length() {
      return entries.size;
    },
    clear: () => entries.clear(),
    getItem: (key) => entries.get(key) ?? null,
    key: (index) => [...entries.keys()][index] ?? null,
    removeItem: (key) => entries.delete(key),
    setItem: (key, value) => entries.set(key, value),
  };
}

function mountHome(
  model: SocialHomeModel,
  callbacks: {
    onPetAction?: (
      action:
        | 'feed'
        | 'gift'
        | 'diary'
        | 'play'
        | 'snack'
        | 'photo'
        | 'end-visit'
        | 'view-visit'
        | 'recall',
    ) => void;
    onInviteFriend?: (id: string) => void;
    onVisitFriend?: (id: string) => void;
    onAcceptVisit?: (id: string) => void;
    onDelayVisit?: (id: string) => void;
    onOpenMemory?: (id: string) => void;
  } = {},
) {
  return mount(SocialHome, {
    target: document.body,
    props: {
      open: true,
      model,
      legacyPet: null,
      theme: 'light',
      onThemeChange: vi.fn(),
      onSendToDesktop: vi.fn(),
      onOpenSettings: vi.fn(),
      ...callbacks,
    },
  });
}

describe('SocialHome', () => {
  it('renders the pet-first shell and desktop transition', async () => {
    const onSendToDesktop = vi.fn();
    component = mount(SocialHome, {
      target: document.body,
      props: {
        open: true,
        model: base,
        legacyPet: null,
        theme: 'light',
        onThemeChange: vi.fn(),
        onSendToDesktop,
        onOpenSettings: vi.fn(),
      },
    });
    await tick();

    expect(document.querySelector('[data-home-shell]')).not.toBeNull();
    expect(document.querySelector('[data-pet-id="muru"]')).not.toBeNull();
    document.querySelector<HTMLButtonElement>('[data-action="send-to-desktop"]')?.click();
    expect(onSendToDesktop).toHaveBeenCalledOnce();
  });

  it('reserves the requested inline size for official and legacy artwork', () => {
    const artworkRule =
      homePetArtworkSource.match(/\.artwork\s*\{(?<body>[^}]*)\}/s)?.groups?.body ?? '';

    expect(artworkRule).toMatch(/width:\s*var\(--art-size\)/);
    expect(artworkRule).toMatch(/max-width:\s*100%/);
    expect(artworkRule).toMatch(/height:\s*calc\(var\(--art-size\) \* 1\.08\)/);
  });

  it('shows an empty nest instead of the local pet while away', async () => {
    component = mount(SocialHome, {
      target: document.body,
      props: {
        open: true,
        model: {
          ...base,
          presence: {
            kind: 'away',
            friendId: momoFriend.id,
            friendName: 'Momo',
            endsAt: '16:30',
            leaseMinutes: 30,
          },
        },
        legacyPet: null,
        theme: 'dark',
        onThemeChange: vi.fn(),
        onSendToDesktop: vi.fn(),
        onOpenSettings: vi.fn(),
      },
    });
    await tick();

    expect(document.querySelector('[data-away-state]')).not.toBeNull();
    expect(document.querySelector('[data-local-pet]')).toBeNull();
  });

  it('uses the legacy pet idle frame without substituting official artwork', async () => {
    component = mount(SocialHome, {
      target: document.body,
      props: {
        open: true,
        model: { ...base, localPet: { id: 'doro', name: 'Doro' } },
        legacyPet,
        theme: 'light',
        onThemeChange: vi.fn(),
        onSendToDesktop: vi.fn(),
        onOpenSettings: vi.fn(),
      },
    });
    await tick();

    const artwork = document.querySelector<HTMLElement>('[data-pet-id="doro"]');
    expect(artwork?.style.backgroundImage).toContain('doro/spritesheet.png');
    expect(document.querySelector('[data-pet-id="muru"]')).toBeNull();
  });

  it('shares appearance preference with onboarding', async () => {
    const onThemeChange = vi.fn();
    originalLocalStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
    Object.defineProperty(globalThis, 'localStorage', {
      configurable: true,
      value: memoryStorage(),
    });
    component = mount(SocialHome, {
      target: document.body,
      props: {
        open: true,
        model: base,
        legacyPet: null,
        theme: 'light',
        onThemeChange,
        onSendToDesktop: vi.fn(),
        onOpenSettings: vi.fn(),
      },
    });
    await tick();

    expect(document.querySelector('[data-theme="light"]')).not.toBeNull();
    document.querySelector<HTMLButtonElement>('[data-theme-choice="dark"]')?.click();
    expect(onThemeChange).toHaveBeenCalledWith('dark');
    expect(localStorage.getItem(ONBOARDING_THEME_STORAGE_KEY)).toBe('dark');
  });

  it('exposes the appearance choices as a named group', async () => {
    component = mount(SocialHome, {
      target: document.body,
      props: {
        open: true,
        model: base,
        legacyPet: null,
        theme: 'light',
        onThemeChange: vi.fn(),
        onSendToDesktop: vi.fn(),
        onOpenSettings: vi.fn(),
      },
    });
    await tick();

    const group = document.querySelector<HTMLElement>('.theme-control');
    expect(group?.getAttribute('role')).toBe('group');
    expect(group?.getAttribute('aria-label')).toBeTruthy();
  });

  it('uses the same 2px focus-token inset indicator for selected theme and dock controls', () => {
    expect(socialHomeSource).toMatch(
      /\.theme-control button\[aria-pressed='true'\]\s*\{[^}]*box-shadow:\s*inset 0 0 0 2px var\(--home-focus\)/s,
    );
    expect(socialDockSource).toMatch(
      /button\[aria-pressed='true'\]\s*\{[^}]*box-shadow:\s*inset 0 0 0 2px var\(--home-focus\)/s,
    );
  });

  it('disables Home panel, artwork, dock, and action motion for reduced-motion users', () => {
    const marker = '@media (prefers-reduced-motion: reduce)';
    const panelReduced = homePanelShellSource.split(marker)[1] ?? '';
    const artworkReduced = homePetArtworkSource.split(marker)[1] ?? '';
    const dockReduced = socialDockSource.split(marker)[1] ?? '';
    const homeReduced = socialHomeSource.split(marker)[1] ?? '';

    expect(panelReduced).toMatch(/\.home-panel\s*\{[^}]*animation:\s*none/s);
    expect(artworkReduced).toMatch(/\.artwork\s*\{[^}]*animation:\s*none/s);
    expect(dockReduced).toMatch(/button\s*\{[^}]*transition:\s*none/s);
    expect(dockReduced).toMatch(/button:active\s*\{[^}]*transform:\s*none/s);
    expect(homeReduced).toMatch(/\.home-shell\s*\{[^}]*animation:\s*none/s);
    expect(homeReduced).toMatch(/\.home-actions button\s*\{[^}]*transition:\s*none/s);
    expect(homeReduced).toMatch(
      /\.home-actions button:active:not\(:disabled\)\s*\{[^}]*transform:\s*none/s,
    );
  });

  it('keeps appearance and settings controls at least 12px', () => {
    const themeButtonRule =
      socialHomeSource.match(/\.theme-control button\s*\{(?<body>[^}]*)\}/s)?.groups?.body ?? '';

    expect(themeButtonRule).toMatch(/font-size:\s*12px/);
    expect(socialHomeSource).toMatch(/\.settings-action\s*\{[^}]*font-size:\s*12px/s);
  });

  it('keeps the essential Plaza Soon label at least 11px and on one line', () => {
    const soonRule = socialDockSource.match(/small\s*\{(?<body>[^}]*)\}/s)?.groups?.body ?? '';
    const fontSize = soonRule.match(/font-size:\s*(?<size>[\d.]+)px/)?.groups?.size;

    expect(Number(fontSize)).toBeGreaterThanOrEqual(11);
    expect(soonRule).toMatch(/white-space:\s*nowrap/);
  });

  it('accepts and delays a visit from the contextual event card', async () => {
    const onAcceptVisit = vi.fn();
    const onDelayVisit = vi.fn();
    const model = {
      ...base,
      friends: [momoFriend],
      pendingVisit: {
        id: 'visit-1',
        friendId: 'friend-momo',
        ownerName: 'Momo',
        pet: { id: 'solu', name: 'Solu', officialPetId: 'solu' as const },
      },
    } satisfies SocialHomeModel;
    component = mountHome(model, { onAcceptVisit, onDelayVisit });
    await tick();

    document.querySelector<HTMLButtonElement>('[data-action="accept-visit"]')?.click();
    document.querySelector<HTMLButtonElement>('[data-action="delay-visit"]')?.click();

    expect(onAcceptVisit).toHaveBeenCalledWith('visit-1');
    expect(onDelayVisit).toHaveBeenCalledWith('visit-1');
  });

  it('announces each visit request in a separate polite live region without wrapping controls', async () => {
    component = mountHome({
      ...base,
      friends: [momoFriend],
      pendingVisit: {
        id: 'visit-a11y',
        friendId: momoFriend.id,
        ownerName: momoFriend.displayName,
        pet: momoFriend.pet,
      },
    });
    await tick();

    const card = document.querySelector('[data-home-event="visit-request"]');
    const announcement = document.querySelector('[data-visit-announcement="visit-a11y"]');
    expect(announcement?.getAttribute('aria-live')).toBe('polite');
    expect(announcement?.textContent).toContain('Solu from Momo wants to visit');
    expect(card?.querySelector('[aria-live]')).toBeNull();
    expect(announcement?.querySelector('button')).toBeNull();
    expect(card?.querySelectorAll('button')).toHaveLength(2);
  });

  it('opens Friends without removing the visible stage', async () => {
    component = mountHome({ ...base, friends: [momoFriend] });
    const friendsTrigger = document.querySelector<HTMLButtonElement>('[data-dock="friends"]');
    friendsTrigger?.focus();
    friendsTrigger?.click();
    await tick();

    expect(document.querySelector('[data-panel="friends"]')).not.toBeNull();
    expect(document.querySelector('.living-room')).not.toBeNull();
    expect(document.activeElement).toBe(document.querySelector('[data-friends-heading]'));

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await tick();

    expect(document.querySelector('[data-panel="friends"]')).toBeNull();
    expect(document.activeElement).toBe(friendsTrigger);
  });

  it('keeps the desktop Friends panel in its approved geometry and leaves a stage lane', () => {
    const panelRule =
      homePanelShellSource.match(/\.home-panel\s*\{(?<body>[^}]*)\}/s)?.groups?.body ?? '';
    const shellRule =
      socialHomeSource.match(/\.home-shell\s*\{(?<body>[^}]*)\}/s)?.groups?.body ?? '';
    const panelMax = Number(
      panelRule.match(/width:\s*clamp\(360px,[^,]+,\s*(?<size>[\d.]+)px\)/)?.groups?.size,
    );
    const shellMax = Number(shellRule.match(/max-width:\s*(?<size>[\d.]+)px/)?.groups?.size);
    const panelRight = Number(panelRule.match(/right:\s*(?<size>[\d.]+)px/)?.groups?.size);
    const dockRule =
      socialDockSource.match(/\.social-dock\s*\{(?<body>[^}]*)\}/s)?.groups?.body ?? '';
    const dockWidth = Number(dockRule.match(/width:\s*(?<size>[\d.]+)px/)?.groups?.size);
    const dockPositionRule =
      socialHomeSource.match(/\.dock-position\s*\{(?<body>[^}]*)\}/s)?.groups?.body ?? '';
    const dockRight = Number(dockPositionRule.match(/right:\s*(?<size>[\d.]+)px/)?.groups?.size);

    expect(panelRule).toMatch(/top:\s*16px/);
    expect(panelRight - dockRight - dockWidth).toBe(16);
    expect(panelRule).toMatch(/bottom:\s*16px/);
    expect(panelRule).toMatch(/width:\s*clamp\(360px,\s*calc\(100% - 32px\),\s*372px\)/);
    expect(panelRule).toMatch(/border-radius:\s*22px/);
    expect(homePanelShellSource).toMatch(
      /@media\s*\(max-width:\s*520px\)\s*\{[\s\S]*?\.home-panel\s*\{[^}]*left:\s*16px[^}]*width:\s*auto/s,
    );
    expect(shellMax - panelMax - panelRight).toBeGreaterThanOrEqual(360);
  });

  it('keeps newly introduced Friends metadata and actions at least 12px', () => {
    const friendsSizes = [...friendsPanelSource.matchAll(/font-size:\s*(?<size>[\d.]+)px/g)].map(
      (match) => Number(match.groups?.size),
    );
    const eventButtonRule =
      homeEventCardSource.match(/button\s*\{(?<body>[^}]*)\}/s)?.groups?.body ?? '';
    const guestTagRule =
      socialHomeSource.match(/\.guest-meta\s*>\s*span\s*\{(?<body>[^}]*)\}/s)?.groups?.body ?? '';
    const guestTimeRule =
      socialHomeSource.match(/\.guest-meta small\s*\{(?<body>[^}]*)\}/s)?.groups?.body ?? '';

    expect(Math.min(...friendsSizes)).toBeGreaterThanOrEqual(12);
    expect(eventButtonRule).toMatch(/font-size:\s*12px/);
    expect(guestTagRule).toMatch(/font-size:\s*12px/);
    expect(guestTimeRule).toMatch(/font-size:\s*12px/);
  });

  it('keeps request acceptance in the request card and disables that friend row action', async () => {
    const onAcceptVisit = vi.fn();
    const onDelayVisit = vi.fn();
    const onInviteFriend = vi.fn();
    component = mountHome(
      {
        ...base,
        friends: [momoFriend],
        pendingVisit: {
          id: 'visit-1',
          friendId: momoFriend.id,
          ownerName: momoFriend.displayName,
          pet: momoFriend.pet,
        },
      },
      { onAcceptVisit, onDelayVisit, onInviteFriend },
    );
    document.querySelector<HTMLButtonElement>('[data-dock="friends"]')?.click();
    await tick();

    const request = document.querySelector<HTMLElement>('[data-visit-request="visit-1"]');
    const friend = document.querySelector<HTMLElement>('[data-friend="friend-momo"]');
    expect(request).not.toBeNull();
    expect(friend).not.toBeNull();
    expect(document.querySelector('.panel-scroll > section')).toBe(
      document.querySelector('.requests'),
    );
    expect(
      request && friend
        ? request.compareDocumentPosition(friend) & Node.DOCUMENT_POSITION_FOLLOWING
        : 0,
    ).toBeTruthy();
    expect(friend?.textContent).toContain('Agent idle');

    request?.querySelector<HTMLButtonElement>('[data-action="accept-visit"]')?.click();
    request?.querySelector<HTMLButtonElement>('[data-action="delay-visit"]')?.click();
    const requestedFriendAction = friend?.querySelector<HTMLButtonElement>(
      '[data-friend-action="visit"]',
    );
    requestedFriendAction?.click();

    expect(onAcceptVisit).toHaveBeenCalledWith('visit-1');
    expect(onDelayVisit).toHaveBeenCalledWith('visit-1');
    expect(requestedFriendAction?.disabled).toBe(true);
    expect(requestedFriendAction?.getAttribute('aria-describedby')).toBeTruthy();
    expect(friend?.querySelector('[data-friend-action-reason]')?.textContent).toContain(
      'Respond to the visit request above',
    );
    expect(onInviteFriend).not.toHaveBeenCalled();
  });

  it('renders contextual Visit, Invite, and Recall actions with honest callback disabling', async () => {
    const onVisitFriend = vi.fn();
    component = mountHome({ ...base, friends: [momoFriend] }, { onVisitFriend });
    document.querySelector<HTMLButtonElement>('[data-dock="friends"]')?.click();
    await tick();

    let action = document.querySelector<HTMLButtonElement>(
      '[data-friend="friend-momo"] [data-friend-action]',
    );
    expect(action?.dataset.friendAction).toBe('visit');
    expect(action?.textContent).toContain('Visit');
    expect(action?.disabled).toBe(false);
    action?.click();
    expect(onVisitFriend).toHaveBeenCalledWith('friend-momo');

    await unmount(component);
    component = mountHome({ ...base, friends: [momoFriend] });
    document.querySelector<HTMLButtonElement>('[data-dock="friends"]')?.click();
    await tick();
    action = document.querySelector('[data-friend="friend-momo"] [data-friend-action]');
    expect(action?.disabled).toBe(true);
    const reasonId = action?.getAttribute('aria-describedby');
    expect(reasonId).toBeTruthy();
    expect(document.getElementById(reasonId ?? '')?.textContent).toContain('not connected');

    await unmount(component);
    component = mountHome({
      ...base,
      friends: [momoFriend],
      presence: {
        kind: 'away',
        friendId: momoFriend.id,
        friendName: 'Renamed Momo',
        endsAt: '16:30',
        leaseMinutes: 30,
      },
    });
    document.querySelector<HTMLButtonElement>('[data-dock="friends"]')?.click();
    await tick();
    action = document.querySelector('[data-friend="friend-momo"] [data-friend-action]');
    expect(action?.dataset.friendAction).toBe('recall');
    expect(action?.textContent).toContain('Recall');
  });

  it('does not render or accept a request from a non-mutual friend id', async () => {
    const onAcceptVisit = vi.fn();
    component = mountHome(
      {
        ...base,
        friends: [momoFriend],
        pendingVisit: {
          id: 'visit-stranger',
          friendId: 'friend-stranger',
          ownerName: momoFriend.displayName,
          pet: { id: 'luma', name: 'Luma', officialPetId: 'luma' },
        },
      },
      { onAcceptVisit },
    );
    document.querySelector<HTMLButtonElement>('[data-dock="friends"]')?.click();
    await tick();

    document.querySelector<HTMLButtonElement>('[data-action="accept-visit"]')?.click();

    expect(document.querySelector('[data-home-event="visit-request"]')).toBeNull();
    expect(document.querySelector('[data-visit-request="visit-stranger"]')).toBeNull();
    expect(onAcceptVisit).not.toHaveBeenCalled();
  });

  it('shows honest disabled account affordances when the friend list is empty', async () => {
    component = mountHome(base);
    document.querySelector<HTMLButtonElement>('[data-dock="friends"]')?.click();
    await tick();

    expect(document.querySelector('[data-friends-empty]')).not.toBeNull();
    expect(document.querySelector<HTMLInputElement>('[data-friend-search]')?.disabled).toBe(true);
    expect(document.querySelector<HTMLButtonElement>('[data-invite-link]')?.disabled).toBe(true);
  });

  it('opens a private memory from the Album panel', async () => {
    const onOpenMemory = vi.fn();
    component = mountHome(
      {
        ...base,
        memories: [
          {
            id: 'memory-1',
            templateKey: 'played_together',
            params: { durationBucket: 'short', timeOfDay: 'morning', interactionCount: 4 },
            occurredAt: Date.UTC(2026, 6, 10),
            petIds: ['muru', 'solu'],
          },
        ],
      },
      { onOpenMemory },
    );
    document.querySelector<HTMLButtonElement>('[data-dock="album"]')?.click();
    await tick();

    document.querySelector<HTMLButtonElement>('[data-memory-id="memory-1"]')?.click();

    expect(onOpenMemory).toHaveBeenCalledWith('memory-1');
    const privacy = document.querySelector('[data-album-privacy]');
    expect(privacy?.textContent).toContain('you, your friend, and your pets');
    expect(privacy?.textContent).toContain('never published or shared automatically');
    expect(document.querySelectorAll('[data-memory-pet]')).toHaveLength(2);
  });

  it('renders allowlisted shared-memory templates at the localized UI boundary', async () => {
    // 模板文案来自 @pawbae/shared 契约表（不再走 svelte-i18n）——安全参数不含任何自由文本
    component = mountHome({
      ...base,
      memories: [
        {
          id: 'memory-safe',
          templateKey: 'worked_together',
          params: { durationBucket: 'full', timeOfDay: 'afternoon', interactionCount: 12 },
          occurredAt: Date.UTC(2026, 6, 10),
          petIds: ['muru', 'solu'],
        },
      ],
    });
    document.querySelector<HTMLButtonElement>('[data-dock="album"]')?.click();
    await tick();

    const memory = document.querySelector('[data-memory-id="memory-safe"]');
    expect(memory?.textContent).toContain('Side by side');
    expect(memory?.textContent).not.toMatch(/prompt|path|task/i);
  });

  it('names every participating pet in a memory card accessible label', async () => {
    component = mountHome({
      ...base,
      memories: [
        {
          id: 'memory-1',
          templateKey: 'played_together',
          params: { durationBucket: 'short', timeOfDay: 'morning', interactionCount: 4 },
          occurredAt: Date.UTC(2026, 6, 10),
          petIds: ['muru', 'solu'],
        },
      ],
    });
    document.querySelector<HTMLButtonElement>('[data-dock="album"]')?.click();
    await tick();

    const memoryLabel = document
      .querySelector<HTMLButtonElement>('[data-memory-id="memory-1"]')
      ?.getAttribute('aria-label');
    expect(memoryLabel).toContain('A visit together');
    expect(memoryLabel).toContain('Jul 10');
    expect(memoryLabel).toContain('muru');
    expect(memoryLabel).toContain('solu');
  });

  it('teaches the private Album empty state without reward pressure', async () => {
    component = mountHome(base);
    document.querySelector<HTMLButtonElement>('[data-dock="album"]')?.click();
    await tick();

    const emptyState = document.querySelector('[data-album-empty]');
    expect(emptyState?.textContent).toContain('No shared memories yet');
    expect(emptyState?.textContent).toContain('mutual-friend visit');
    expect(emptyState?.textContent).not.toMatch(/reward|streak/i);
  });

  it('marks Plaza as future work without fake profiles', async () => {
    component = mountHome(base);
    document.querySelector<HTMLButtonElement>('[data-dock="plaza"]')?.click();
    await tick();

    const plaza = document.querySelector('[data-panel="plaza"]');
    expect(plaza?.textContent).toContain('Soon');
    expect(plaza?.textContent).toContain('Coming later');
    expect(document.querySelector('[data-plaza-profile]')).toBeNull();
    expect(plaza?.querySelector('[data-friend]')).toBeNull();
    expect(plaza?.textContent).not.toMatch(/@\w+|online|followers?|\d+\s+(pets?|homes?)/i);
  });

  it('returns focus to the Album dock control after Escape', async () => {
    component = mountHome(base);
    const albumTrigger = document.querySelector<HTMLButtonElement>('[data-dock="album"]');
    albumTrigger?.focus();
    albumTrigger?.click();
    await tick();

    expect(document.activeElement).toBe(document.querySelector('[data-panel-heading]'));

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await tick();

    expect(document.querySelector('[data-panel="album"]')).toBeNull();
    expect(document.activeElement).toBe(albumTrigger);
  });

  it('focuses each new shared panel heading when switching destinations', async () => {
    component = mountHome(base);
    const albumTrigger = document.querySelector<HTMLButtonElement>('[data-dock="album"]');
    const plazaTrigger = document.querySelector<HTMLButtonElement>('[data-dock="plaza"]');

    albumTrigger?.focus();
    albumTrigger?.click();
    await tick();
    expect(document.querySelector('[data-panel="album"]')).not.toBeNull();
    expect(document.activeElement).toBe(document.querySelector('[data-panel="album"] h2'));

    plazaTrigger?.focus();
    plazaTrigger?.click();
    await tick();
    expect(document.querySelector('[data-panel="album"]')).toBeNull();
    expect(document.querySelector('[data-panel="plaza"]')).not.toBeNull();
    expect(document.activeElement).toBe(document.querySelector('[data-panel="plaza"] h2'));

    albumTrigger?.focus();
    albumTrigger?.click();
    await tick();
    expect(document.querySelector('[data-panel="plaza"]')).toBeNull();
    expect(document.querySelector('[data-panel="album"]')).not.toBeNull();
    expect(document.activeElement).toBe(document.querySelector('[data-panel="album"] h2'));

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await tick();
    expect(document.activeElement).toBe(albumTrigger);
  });

  it('labels one hosted pet with its owner and keeps hosting actions callback-driven', async () => {
    const onPetAction = vi.fn();
    const model = {
      ...base,
      presence: {
        kind: 'home',
        visitor: momoFriend.pet,
        visitorOwnerName: 'Momo',
        visitorAgentState: 'idle',
        endsAt: '16:30',
        leaseMinutes: 30,
      },
    } satisfies SocialHomeModel;
    component = mountHome(model, { onPetAction });
    await tick();

    expect(document.querySelectorAll('[data-guest-pet]')).toHaveLength(1);
    expect(document.querySelector('[data-guest-tag]')?.textContent).toContain('Momo');
    expect(document.querySelector('[data-visit-ends]')?.textContent).toContain('16:30');
    expect(document.querySelector('[data-visit-ends]')?.textContent).toContain('30');
    expect(
      [...document.querySelectorAll<HTMLButtonElement>('.home-actions [data-action]')].map(
        (button) => button.dataset.action,
      ),
    ).toEqual(['play', 'snack', 'photo', 'end-visit']);

    document.querySelector<HTMLButtonElement>('[data-action="end-visit"]')?.click();
    expect(onPetAction).toHaveBeenCalledWith('end-visit');
  });

  it('shows quiet visitor-offline and realtime recovery state without private activity text', async () => {
    component = mountHome({
      ...base,
      realtimeState: 'degraded',
      presence: {
        kind: 'home',
        visitor: momoFriend.pet,
        visitorOwnerName: 'Momo',
        visitorAgentState: 'offline',
        endsAt: '16:30',
        leaseMinutes: 30,
      },
    });
    await tick();

    expect(document.querySelector('[data-visitor-status]')?.textContent).toContain(
      "owner's agent is resting",
    );
    expect(document.querySelector('[data-visitor-resting-visual]')).not.toBeNull();
    expect(document.querySelector('[data-realtime-state="degraded"]')?.textContent).toContain(
      'Updates may be delayed',
    );
    expect(document.body.textContent).not.toMatch(/prompt|tool call|workspace|file path/i);
  });

  it('renders compact together-day and growth progress in the care footer', async () => {
    component = mountHome(base);
    await tick();

    const care = document.querySelector('[data-home-care]');
    const progress = care?.querySelector<HTMLProgressElement>('progress');
    expect(care?.textContent).toContain('23 days together');
    expect(care?.textContent).toContain('320 / 500');
    expect(progress?.value).toBe(320);
    expect(progress?.max).toBe(500);
    expect(progress?.getAttribute('aria-label')).toBe('Growth progress: 320 of 500');
  });
});
