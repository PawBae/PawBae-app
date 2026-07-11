<script lang="ts">
  import { onMount } from 'svelte';
  import { locale, waitLocale } from 'svelte-i18n';
  import SocialHome from './lib/components/home/SocialHome.svelte';
  import type { OfficialPetId } from './lib/utils/onboarding';
  import type { OnboardingTheme } from './lib/utils/onboarding-theme';
  import type {
    FriendSummary,
    HomeAction,
    HomePresence,
    PublicAgentState,
    SharedMemorySummary,
    SocialHomeModel,
  } from './lib/utils/social-home';

  const languages = ['en', 'zh'] as const;
  const themes = ['system', 'light', 'dark'] as const;
  const pets = ['solu', 'muru', 'riffi', 'luma'] as const;
  const scenarios = [
    'idle',
    'working',
    'waiting',
    'compacting',
    'request',
    'hosting',
    'away',
    'memory',
    'offline',
    'visitor-offline',
    'realtime-degraded',
    'realtime-reconnecting',
  ] as const;

  type PreviewLanguage = (typeof languages)[number];
  type PreviewScenario = (typeof scenarios)[number];

  const petNames: Record<PreviewLanguage, Record<OfficialPetId, string>> = {
    en: { solu: 'Solu', muru: 'Muru', riffi: 'Riffi', luma: 'Luma' },
    zh: { solu: '小煦', muru: '雾露', riffi: '雷栗', luma: '星沫' },
  };

  const scenarioLabels: Record<PreviewScenario, string> = {
    idle: 'Idle',
    working: 'Working',
    waiting: 'Waiting',
    compacting: 'Compacting',
    request: 'Visit request',
    hosting: 'Hosting',
    away: 'Away',
    memory: 'Memory ready',
    offline: 'Offline',
    'visitor-offline': 'Visitor offline',
    'realtime-degraded': 'Realtime delayed',
    'realtime-reconnecting': 'Realtime reconnecting',
  };

  function readChoice<const T extends readonly string[]>(
    params: URLSearchParams,
    key: string,
    choices: T,
    fallback: T[number],
  ): T[number] {
    const value = params.get(key);
    return value && choices.includes(value) ? (value as T[number]) : fallback;
  }

  function previewModel(
    petId: OfficialPetId,
    scenario: PreviewScenario,
    currentLanguage: PreviewLanguage,
  ): SocialHomeModel {
    const localPet = {
      id: `preview-local-${petId}`,
      name: petNames[currentLanguage][petId],
      officialPetId: petId,
    };
    const solu = {
      id: 'preview-momo-solu',
      name: petNames[currentLanguage].solu,
      officialPetId: 'solu' as const,
      ownerName: 'Momo',
    };
    const momo: FriendSummary = {
      id: 'preview-friend-momo',
      displayName: 'Momo',
      handle: '@momo',
      pet: solu,
      availability:
        scenario === 'hosting' ||
        scenario === 'visitor-offline' ||
        scenario === 'realtime-degraded' ||
        scenario === 'realtime-reconnecting' ||
        scenario === 'away'
          ? 'visiting'
          : 'available',
      publicAgentState: 'idle',
      visitDirection: 'visit-them',
    };
    const memory: SharedMemorySummary = {
      id: 'preview-memory-rainy-tea',
      templateKey: 'rainy-tea',
      params: {},
      occurredAt: Date.UTC(2026, 6, 10),
      petIds: [localPet.id, solu.id],
    };

    let presence: HomePresence = { kind: 'home', visitor: null };
    if (
      scenario === 'hosting' ||
      scenario === 'visitor-offline' ||
      scenario === 'realtime-degraded' ||
      scenario === 'realtime-reconnecting'
    ) {
      presence = {
        kind: 'home',
        visitor: solu,
        visitorOwnerName: 'Momo',
        visitorAgentState: scenario === 'visitor-offline' ? 'offline' : 'idle',
        endsAt: '16:30',
        leaseMinutes: 30,
      };
    } else if (scenario === 'away') {
      presence = {
        kind: 'away',
        friendId: momo.id,
        friendName: 'Momo',
        endsAt: '16:30',
        leaseMinutes: 30,
      };
    }

    const agentState: PublicAgentState =
      scenario === 'working'
      || scenario === 'waiting'
      || scenario === 'compacting'
      || scenario === 'offline'
        ? scenario
        : 'idle';

    return {
      localPet,
      presence,
      agentState,
      realtimeState:
        scenario === 'realtime-degraded'
          ? 'degraded'
          : scenario === 'realtime-reconnecting'
            ? 'reconnecting'
            : 'connected',
      affection: 86,
      coins: 140,
      togetherDays: 23,
      growthCurrent: 320,
      growthTarget: 500,
      friends: [momo],
      pendingVisit:
        scenario === 'request'
          ? {
              id: 'preview-visit-momo',
              friendId: momo.id,
              ownerName: 'Momo',
              pet: solu,
            }
          : null,
      latestMemory: scenario === 'memory' ? memory : null,
      memories: scenario === 'memory' ? [memory] : [],
    };
  }

  const params = new URLSearchParams(window.location.search);
  let language = $state<PreviewLanguage>(readChoice(params, 'lang', languages, 'en'));
  let theme = $state<OnboardingTheme>(readChoice(params, 'theme', themes, 'system'));
  let pet = $state<OfficialPetId>(readChoice(params, 'pet', pets, 'muru'));
  let scenario = $state<PreviewScenario>(readChoice(params, 'state', scenarios, 'idle'));
  let ready = $state(false);
  let notice = $state('');
  const model = $derived(previewModel(pet, scenario, language));

  onMount(async () => {
    locale.set(language);
    await waitLocale();
    ready = true;
  });

  function selectedValue(event: Event): string {
    return (event.currentTarget as HTMLSelectElement).value;
  }

  function updateQuery(key: string, value: string) {
    const url = new URL(window.location.href);
    url.searchParams.set(key, value);
    window.history.replaceState({}, '', url);
  }

  function setLanguage(next: PreviewLanguage) {
    language = next;
    locale.set(next);
    void waitLocale();
    updateQuery('lang', next);
  }

  function setTheme(next: OnboardingTheme) {
    theme = next;
    updateQuery('theme', next);
  }

  function setPet(next: OfficialPetId) {
    pet = next;
    updateQuery('pet', next);
  }

  function setScenario(next: PreviewScenario) {
    scenario = next;
    notice = '';
    updateQuery('state', next);
  }

  function runPetAction(action: Exclude<HomeAction, 'send-to-desktop'>) {
    if (action === 'end-visit' || action === 'recall') setScenario('idle');
    else notice = `${action} stays inside this preview.`;
  }
</script>

<main class="preview-page">
  <form
    class="preview-toolbar"
    data-preview-toolbar
    aria-label="Social Home preview controls"
    onsubmit={(event) => event.preventDefault()}
  >
    <strong>Social Home preview</strong>
    <label>
      <span>Language</span>
      <select
        data-preview-control="lang"
        name="lang"
        value={language}
        onchange={(event) => setLanguage(selectedValue(event) as PreviewLanguage)}
      >
        {#each languages as choice}
          <option value={choice}>{choice === 'en' ? 'English' : '中文'}</option>
        {/each}
      </select>
    </label>
    <label>
      <span>Theme</span>
      <select
        data-preview-control="theme"
        name="theme"
        value={theme}
        onchange={(event) => setTheme(selectedValue(event) as OnboardingTheme)}
      >
        {#each themes as choice}
          <option value={choice}>{choice[0].toUpperCase() + choice.slice(1)}</option>
        {/each}
      </select>
    </label>
    <label>
      <span>Pet</span>
      <select
        data-preview-control="pet"
        name="pet"
        value={pet}
        onchange={(event) => setPet(selectedValue(event) as OfficialPetId)}
      >
        {#each pets as choice}
          <option value={choice}>{petNames[language][choice]}</option>
        {/each}
      </select>
    </label>
    <label>
      <span>State</span>
      <select
        data-preview-control="state"
        name="state"
        value={scenario}
        onchange={(event) => setScenario(selectedValue(event) as PreviewScenario)}
      >
        {#each scenarios as choice}
          <option value={choice}>{scenarioLabels[choice]}</option>
        {/each}
      </select>
    </label>
    <span class="preview-notice" aria-live="polite">{notice}</span>
  </form>

  <div class="preview-stage" data-preview-stage>
    {#if ready}
      <SocialHome
        open
        {model}
        legacyPet={null}
        {theme}
        onThemeChange={setTheme}
        onSendToDesktop={() => (notice = 'Desktop transition is disabled in preview.')}
        onOpenSettings={() => (notice = 'Settings are outside this component preview.')}
        onPetAction={runPetAction}
        onVisitFriend={(id) => (notice = `Visit ${id} stays inside this preview.`)}
        onAcceptVisit={() => setScenario('hosting')}
        onDelayVisit={() => setScenario('idle')}
        onOpenMemory={(id) => (notice = `Opened local preview memory ${id}.`)}
      />
    {/if}
  </div>
</main>

<style>
  :global(html),
  :global(body) {
    min-width: 100%;
    min-height: 100%;
    margin: 0;
    background: #e8e8eb;
    color: #3f3d43;
    font-family: Inter, 'Noto Sans SC', 'Segoe UI', 'PingFang SC', sans-serif;
  }

  :global(*) {
    box-sizing: border-box;
  }

  .preview-page {
    display: flex;
    width: max-content;
    min-width: 100%;
    min-height: 100vh;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 12px;
  }

  .preview-toolbar {
    display: flex;
    width: 960px;
    min-height: 40px;
    align-items: center;
    gap: 12px;
    padding: 6px 8px;
    border: 1px solid #c9c8ce;
    border-radius: 10px;
    background: #f5f5f6;
    font-size: 12px;
  }

  .preview-toolbar > strong {
    margin-right: auto;
    color: #4a474f;
    font-size: 12px;
    white-space: nowrap;
  }

  .preview-toolbar label {
    display: flex;
    align-items: center;
    gap: 5px;
    color: #67636c;
    white-space: nowrap;
  }

  .preview-toolbar select {
    min-height: 28px;
    padding: 0 24px 0 7px;
    border: 1px solid #b9b7bf;
    border-radius: 7px;
    background: #ffffff;
    color: #343139;
    font: inherit;
  }

  .preview-toolbar select:focus-visible {
    outline: 2px solid #596bc0;
    outline-offset: 2px;
  }

  .preview-notice {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }

  .preview-stage {
    position: relative;
    width: 960px;
    height: 600px;
    flex: 0 0 auto;
    overflow: hidden;
  }

  .preview-stage :global(.home-overlay) {
    position: absolute;
    inset: 0;
    padding: 0;
    background: transparent;
  }

  .preview-stage :global(.home-shell) {
    width: 960px;
    max-width: none;
    height: 600px;
  }
</style>
