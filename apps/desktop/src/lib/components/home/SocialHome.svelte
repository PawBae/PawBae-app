<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { FriendEntry, PublicProfile } from '../../platform/types';
  import type { CodexPet } from '../../utils/codex-pet';
  import {
    ONBOARDING_THEME_STORAGE_KEY,
    type OnboardingTheme,
  } from '../../utils/onboarding-theme';
  import {
    allowedHomeActions,
    authorizedVisitRequest,
    type HomeAction,
    type HomePanel,
    type SocialHomeModel,
    selectHomeEvent,
  } from '../../utils/social-home';
  import type { VisitInteraction } from '../../utils/visit-stage';
  import FriendsPanel from './FriendsPanel.svelte';
  import HomeEventCard from './HomeEventCard.svelte';
  import HomePetArtwork from './HomePetArtwork.svelte';
  import PetIdentityCapsule from './PetIdentityCapsule.svelte';
  import SharedAlbumPanel from './SharedAlbumPanel.svelte';
  import SocialDock from './SocialDock.svelte';

  let {
    open = false,
    model,
    legacyPet,
    theme,
    onThemeChange,
    onSendToDesktop,
    onOpenSettings,
    onPetAction,
    onInviteFriend,
    onVisitFriend,
    onAcceptVisit,
    onDelayVisit,
    onOpenMemory,
    relationships = [],
    inviteRedeemed = false,
    onFindFriend,
    onSendFriendRequest,
    onAcceptFriend,
    onRemoveFriend,
    onMuteFriend,
    onBlockFriend,
    onRedeemInvite,
    onDeclineVisit,
    visitInteraction = null,
  }: {
    open?: boolean;
    model: SocialHomeModel;
    legacyPet: CodexPet | null;
    theme: OnboardingTheme;
    onThemeChange: (theme: OnboardingTheme) => void;
    onSendToDesktop: () => void;
    onOpenSettings: () => void;
    onPetAction?: (action: Exclude<HomeAction, 'send-to-desktop'>) => void;
    onInviteFriend?: (id: string) => void;
    onVisitFriend?: (id: string) => void;
    onAcceptVisit?: (id: string) => void;
    onDelayVisit?: (id: string) => void;
    onOpenMemory?: (id: string) => void;
    relationships?: FriendEntry[];
    inviteRedeemed?: boolean;
    onFindFriend?: (handle: string) => Promise<PublicProfile | null>;
    onSendFriendRequest?: (id: string) => Promise<void>;
    onAcceptFriend?: (id: string) => Promise<void>;
    onRemoveFriend?: (id: string) => Promise<void>;
    onMuteFriend?: (id: string, muted: boolean) => Promise<void>;
    onBlockFriend?: (id: string) => Promise<void>;
    onRedeemInvite?: (code: string) => Promise<void>;
    onDeclineVisit?: (id: string) => void;
    visitInteraction?: VisitInteraction | null;
  } = $props();

  const actionKeys: Record<HomeAction, string> = {
    feed: 'feed',
    gift: 'gift',
    diary: 'diary',
    'send-to-desktop': 'sendDesktop',
    play: 'play',
    snack: 'snack',
    photo: 'photo',
    'end-visit': 'endVisit',
    'view-visit': 'viewVisit',
    recall: 'recall',
  };

  const themeChoices: OnboardingTheme[] = ['system', 'light', 'dark'];
  const petGlow: Record<string, string> = {
    solu: 'rgba(245, 143, 94, 0.18)',
    muru: 'rgba(179, 199, 240, 0.26)',
    riffi: 'rgba(168, 224, 192, 0.22)',
    luma: 'rgba(245, 175, 200, 0.22)',
  };

  let activePanel = $state<HomePanel>(null);
  let panelWasOpen = false;
  let panelReturnFocus: HTMLElement | null = null;
  let previousPanel: HomePanel = null;
  let systemTheme = $state<'light' | 'dark'>('light');
  const resolvedTheme = $derived(theme === 'system' ? systemTheme : theme);
  const currentPetGlow = $derived(
    petGlow[model.localPet.officialPetId ?? ''] ?? 'rgba(99, 94, 103, 0.13)',
  );
  const mutualVisitRequest = $derived(authorizedVisitRequest(model));

  $effect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return;
    const query = window.matchMedia('(prefers-color-scheme: dark)');
    const sync = () => {
      systemTheme = query.matches ? 'dark' : 'light';
    };
    sync();
    query.addEventListener('change', sync);
    return () => query.removeEventListener('change', sync);
  });

  $effect.pre(() => {
    if (activePanel) {
      if (activePanel !== previousPanel && document.activeElement instanceof HTMLElement) {
        panelReturnFocus = document.activeElement;
      }
      panelWasOpen = true;
      previousPanel = activePanel;
      return;
    }

    previousPanel = null;
    if (panelWasOpen) {
      panelWasOpen = false;
      const returnTarget = panelReturnFocus;
      panelReturnFocus = null;
      queueMicrotask(() => returnTarget?.focus());
    }
  });

  function setTheme(nextTheme: OnboardingTheme) {
    try {
      localStorage.setItem(ONBOARDING_THEME_STORAGE_KEY, nextTheme);
    } catch {
      // Keep the session callback working when storage is unavailable.
    }
    onThemeChange(nextTheme);
  }

  function runAction(action: HomeAction) {
    if (action === 'send-to-desktop') onSendToDesktop();
    else onPetAction?.(action);
  }
</script>

{#if open}
  <section
    class="home-overlay"
    data-theme={resolvedTheme}
    aria-label={$_('home.title')}
  >
    <div class="home-shell" data-home-shell style={`--home-pet-glow:${currentPetGlow}`}>
      <div class="top-region">
        <PetIdentityCapsule {model} />
        <div class="home-tools">
          <div class="theme-control" role="group" aria-label={$_('settings.display')}>
            {#each themeChoices as choice}
              <button
                type="button"
                class:active={theme === choice}
                data-theme-choice={choice}
                aria-pressed={theme === choice}
                onclick={() => setTheme(choice)}
              >{$_(`home.theme.${choice}`)}</button>
            {/each}
          </div>
          <button class="settings-action" type="button" onclick={onOpenSettings}>
            {$_('home.settings')}
          </button>
        </div>
      </div>

      {#if model.realtimeState !== 'connected'}
        <div class="realtime-note" data-realtime-state={model.realtimeState} role="status">
          {$_(`home.realtime.${model.realtimeState}`)}
        </div>
      {/if}

      <div class="dock-position"><SocialDock bind:activePanel /></div>

      <main class="living-room" data-visit-interaction={visitInteraction ?? undefined}>
        {#if model.presence.kind === 'away'}
          <div data-away-state class="empty-nest">
            <span aria-hidden="true">⌁</span>
            <p>
              {$_('home.away', {
                values: { pet: model.localPet.name, friend: model.presence.friendName },
              })}
            </p>
          </div>
        {:else}
          <div data-local-pet>
            <HomePetArtwork
              pet={model.localPet}
              {legacyPet}
              size={240}
              state={model.agentState}
            />
          </div>
          {#if model.presence.visitor}
            <div
              class="guest-slot"
              class:resting={model.presence.visitorAgentState === 'offline'}
              data-guest-pet
            >
              <HomePetArtwork
                pet={model.presence.visitor}
                legacyPet={null}
                size={190}
                guest
                state={model.presence.visitorAgentState}
              />
              {#if model.presence.visitorAgentState === 'offline'}
                <span class="resting-visual" data-visitor-resting-visual aria-hidden="true">zZ</span>
              {/if}
              <div class="guest-meta">
                <span data-guest-tag>
                  {$_('home.guestTag', { values: { owner: model.presence.visitorOwnerName } })}
                </span>
                <small data-visit-ends>
                  {$_('home.visit.remaining', {
                    values: {
                      minutes: model.presence.leaseMinutes,
                      time: model.presence.endsAt,
                    },
                  })}
                </small>
                <small data-visitor-status>
                  {model.presence.visitorAgentState === 'offline'
                    ? $_('home.visit.visitorOffline')
                    : $_(`home.friends.publicStatus.${model.presence.visitorAgentState}`)}
                </small>
              </div>
            </div>
          {/if}
        {/if}
      </main>

      <HomeEventCard
        event={selectHomeEvent(model)}
        petName={model.localPet.name}
        {onAcceptVisit}
        {onDelayVisit}
        {onInviteFriend}
        {onOpenMemory}
      />

      <footer class="home-actions">
        <div class="care-progress" data-home-care>
          <span>{$_('home.care.togetherDays', { values: { days: model.togetherDays } })}</span>
          <strong>{model.growthCurrent} / {model.growthTarget}</strong>
          <progress
            value={model.growthCurrent}
            max={Math.max(1, model.growthTarget)}
            aria-label={$_('home.care.growthProgress', {
              values: { current: model.growthCurrent, target: model.growthTarget },
            })}
          >
            {model.growthCurrent} / {model.growthTarget}
          </progress>
        </div>
        {#each allowedHomeActions(model) as action}
          <button
            type="button"
            class:primary={action === 'send-to-desktop'}
            data-action={action}
            disabled={action !== 'send-to-desktop' && !onPetAction}
            onclick={() => runAction(action)}
          >
            {$_(`home.actions.${actionKeys[action]}`, { values: { pet: model.localPet.name } })}
          </button>
        {/each}
      </footer>

      {#if activePanel === 'friends'}
        <FriendsPanel
          friends={model.friends}
          presence={model.presence}
          pendingVisit={mutualVisitRequest}
          onClose={() => (activePanel = null)}
          {onInviteFriend}
          {onVisitFriend}
          onRecallFriend={onPetAction ? (_id) => onPetAction?.('recall') : undefined}
          {onAcceptVisit}
          {onDelayVisit}
          {relationships}
          {inviteRedeemed}
          {onFindFriend}
          {onSendFriendRequest}
          {onAcceptFriend}
          {onRemoveFriend}
          {onMuteFriend}
          {onBlockFriend}
          {onRedeemInvite}
          {onDeclineVisit}
        />
      {:else if activePanel === 'album' || activePanel === 'plaza'}
        {#key activePanel}
          <SharedAlbumPanel
            panel={activePanel}
            memories={model.memories}
            onClose={() => (activePanel = null)}
            {onOpenMemory}
          />
        {/key}
      {/if}
    </div>
  </section>
{/if}

<style>
  .home-overlay {
    --home-canvas: #fbfaf8;
    --home-surface: #ffffff;
    --home-subtle: #f4f1f1;
    --home-text: #2e2b31;
    --home-text-muted: #635e67;
    --home-border: #ded8dc;
    --home-action: #3d4e9e;
    --home-action-hover: #334487;
    --home-action-text: #ffffff;
    --home-focus: #596bc0;
    position: fixed;
    inset: 0;
    z-index: 2100;
    display: grid;
    place-items: center;
    padding: 12px;
    background: rgba(35, 31, 36, 0.34);
    color: var(--home-text);
    font-family: Inter, 'Noto Sans SC', 'Segoe UI', 'PingFang SC', sans-serif;
  }

  .home-overlay[data-theme='dark'] {
    --home-canvas: #242226;
    --home-surface: #2d2a2f;
    --home-subtle: #36323a;
    --home-text: #f7f3f5;
    --home-text-muted: #c8c0c8;
    --home-border: #514b55;
    --home-action: #b3c7f0;
    --home-action-hover: #c9d6f5;
    --home-action-text: #2e2b31;
    --home-focus: #c9d6f5;
    background: rgba(19, 18, 20, 0.64);
  }

  .home-shell {
    position: relative;
    display: grid;
    grid-template-rows: 88px minmax(0, 1fr) auto 76px;
    width: min(960px, calc(100vw - 24px));
    max-width: 960px;
    height: min(600px, calc(100vh - 24px));
    overflow: hidden;
    border-radius: 18px;
    background: var(--home-canvas);
    box-shadow: 0 8px 24px rgba(38, 30, 36, 0.14);
    animation: home-in 200ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  .top-region {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    padding: 12px 96px 8px 20px;
  }

  .home-tools {
    display: flex;
    align-items: center;
    gap: 9px;
  }

  .theme-control {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 2px;
    border-radius: 12px;
    background: var(--home-subtle);
  }

  .theme-control button,
  .settings-action {
    border: 0;
    background: transparent;
    color: var(--home-text-muted);
    font: inherit;
    cursor: pointer;
  }

  .theme-control button {
    min-height: 30px;
    padding: 0 8px;
    border-radius: 10px;
    font-size: 12px;
    font-weight: 650;
    white-space: nowrap;
  }

  .theme-control button:hover,
  .theme-control button.active {
    background: var(--home-surface);
    color: var(--home-text);
  }

  .theme-control button[aria-pressed='true'] {
    box-shadow: inset 0 0 0 2px var(--home-focus);
  }

  .settings-action {
    min-height: 34px;
    padding: 0 10px;
    border: 1px solid var(--home-border);
    border-radius: 10px;
    background: var(--home-surface);
    color: var(--home-text);
    font-size: 12px;
    font-weight: 700;
    white-space: nowrap;
  }

  button:focus-visible {
    outline: 2px solid var(--home-focus);
    outline-offset: 3px;
  }

  .dock-position {
    position: absolute;
    top: 50%;
    right: 16px;
    z-index: 2;
    transform: translateY(-50%);
  }

  .realtime-note {
    position: absolute;
    top: 76px;
    left: 50%;
    z-index: 2;
    padding: 5px 10px;
    border: 1px solid var(--home-border);
    border-radius: 999px;
    background: var(--home-surface);
    color: var(--home-text-muted);
    font-size: 12px;
    font-weight: 650;
    transform: translateX(-50%);
  }

  .living-room {
    position: relative;
    display: flex;
    min-height: 0;
    align-items: end;
    justify-content: center;
    gap: 18px;
    padding: 2px 112px 2px 36px;
    overflow: hidden;
    isolation: isolate;
  }

  .living-room::before {
    position: absolute;
    inset: 10% 12% 0 4%;
    z-index: -1;
    background: radial-gradient(ellipse at 52% 72%, var(--home-pet-glow), transparent 62%);
    content: '';
    pointer-events: none;
  }

  [data-local-pet] {
    display: grid;
    min-width: 0;
    place-items: end center;
  }

  .guest-slot {
    position: relative;
    display: grid;
    min-width: 0;
    place-items: end center;
  }

  .living-room[data-visit-interaction='nose-touch'] .guest-slot {
    transform: translateX(-12px);
  }

  .living-room[data-visit-interaction='celebrate'] .guest-slot {
    animation: guest-celebrate 900ms ease-in-out infinite alternate;
  }

  .living-room[data-visit-interaction='rest'] .guest-slot {
    transform: translateY(5px);
  }

  @keyframes guest-celebrate {
    to {
      transform: translateY(-7px) rotate(2deg);
    }
  }

  .guest-slot.resting :global(.artwork) {
    opacity: 0.78;
    transform: translateY(3px) rotate(-2deg);
  }

  .resting-visual {
    position: absolute;
    top: 18px;
    right: 5px;
    z-index: 2;
    color: var(--home-text-muted);
    font-size: 13px;
    font-weight: 750;
    letter-spacing: 0.08em;
    transform: rotate(-8deg);
  }

  .guest-meta {
    z-index: 1;
    display: grid;
    justify-items: center;
    gap: 3px;
    margin-top: -10px;
  }

  .guest-meta > span {
    padding: 4px 9px;
    border-radius: 999px;
    background: var(--home-action);
    color: var(--home-action-text);
    font-size: 12px;
    font-weight: 750;
    line-height: 1.2;
    white-space: nowrap;
  }

  .guest-meta small {
    color: var(--home-text-muted);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    line-height: 1.3;
    white-space: nowrap;
  }

  .empty-nest {
    display: grid;
    max-width: 440px;
    place-items: center;
    align-self: center;
    gap: 10px;
    color: var(--home-text-muted);
    text-align: center;
  }

  .empty-nest > span {
    display: grid;
    width: 88px;
    height: 58px;
    place-items: center;
    border-radius: 50%;
    background: color-mix(in srgb, var(--home-pet-glow) 48%, var(--home-subtle));
    color: var(--home-action);
    font-size: 38px;
    transform: rotate(-8deg);
  }

  .empty-nest p {
    margin: 0;
    color: var(--home-text);
    font-size: 15px;
    font-weight: 700;
    line-height: 1.4;
    text-wrap: balance;
  }

  .home-actions {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-width: 0;
    padding: 10px 100px 12px 18px;
    border-top: 1px solid var(--home-border);
    background: var(--home-surface);
    overflow-x: auto;
  }

  .care-progress {
    display: grid;
    width: 168px;
    flex: 0 0 168px;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 3px 8px;
    margin-right: 8px;
    color: var(--home-text-muted);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .care-progress strong {
    color: var(--home-text);
    font-size: 12px;
  }

  .care-progress progress {
    width: 100%;
    height: 5px;
    grid-column: 1 / -1;
    overflow: hidden;
    border: 0;
    border-radius: 999px;
    background: var(--home-subtle);
    accent-color: var(--home-action);
  }

  .care-progress progress::-webkit-progress-bar {
    background: var(--home-subtle);
  }

  .care-progress progress::-webkit-progress-value {
    border-radius: 999px;
    background: var(--home-action);
  }

  .home-actions button {
    min-height: 38px;
    flex: 0 0 auto;
    padding: 0 14px;
    border: 1px solid var(--home-border);
    border-radius: 11px;
    background: var(--home-canvas);
    color: var(--home-text);
    font: inherit;
    font-size: 12px;
    font-weight: 700;
    white-space: nowrap;
    cursor: pointer;
    transition: background 180ms ease-out, border-color 180ms ease-out, transform 180ms ease-out;
  }

  .home-actions button:hover:not(:disabled) {
    border-color: var(--home-action);
    background: var(--home-subtle);
  }

  .home-actions button:active:not(:disabled) {
    transform: translateY(1px);
  }

  .home-actions button.primary {
    border-color: transparent;
    background: var(--home-action);
    color: var(--home-action-text);
  }

  .home-actions button.primary:hover {
    border-color: transparent;
    background: var(--home-action-hover);
  }

  .home-actions button:disabled {
    cursor: not-allowed;
    opacity: 0.48;
  }

  @keyframes home-in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @media (max-width: 760px) {
    .top-region {
      align-items: stretch;
      padding-right: 84px;
    }

    .home-tools {
      justify-content: flex-end;
    }

    .theme-control button {
      padding-inline: 5px;
    }

    .living-room {
      padding-left: 18px;
      padding-right: 94px;
    }

    .home-actions {
      justify-content: flex-start;
      padding-right: 86px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .living-room .guest-slot {
      animation: none;
      transform: none;
    }
    .home-shell {
      animation: none;
    }

    .home-actions button {
      transition: none;
    }

    .home-actions button:active:not(:disabled) {
      transform: none;
    }
  }
</style>
