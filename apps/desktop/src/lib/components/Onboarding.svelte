<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { tick } from 'svelte';
  import { _ } from 'svelte-i18n';
  import {
    type AgentId,
    type AgentInstallStatus,
    agentAvailableOnPlatform,
    deriveOnboardingMode,
    type GithubProfile,
    hookCommandForAgent,
    nextOnboardingStep,
    OFFICIAL_PETS,
    type OfficialPetId,
    type OnboardingResult,
    type OnboardingStep,
    previousOnboardingStep,
  } from '../utils/onboarding';
  import {
    normalizeOnboardingTheme,
    ONBOARDING_THEME_STORAGE_KEY,
    type OnboardingTheme,
  } from '../utils/onboarding-theme';
  import AgentConnectionRow from './onboarding/AgentConnectionRow.svelte';
  import OnboardingProgress from './onboarding/OnboardingProgress.svelte';
  import PetAdoptionCard from './onboarding/PetAdoptionCard.svelte';

  interface OnboardingProps {
    open?: boolean;
    isWindows: boolean;
    onComplete: (result: OnboardingResult) => Promise<void> | void;
    onGithubSignIn?: () => Promise<GithubProfile>;
  }

  let {
    open = false,
    isWindows,
    onComplete,
    onGithubSignIn,
  }: OnboardingProps = $props();

  const agents: AgentId[] = ['claude', 'codex', 'cursor'];
  const themes: { id: OnboardingTheme; icon: string }[] = [
    { id: 'system', icon: '◐' },
    { id: 'light', icon: '☼' },
    { id: 'dark', icon: '☾' },
  ];

  function loadTheme(): OnboardingTheme {
    if (typeof localStorage === 'undefined') return 'system';
    try {
      return normalizeOnboardingTheme(localStorage.getItem(ONBOARDING_THEME_STORAGE_KEY));
    } catch {
      return 'system';
    }
  }

  let step = $state<OnboardingStep>('welcome');
  let theme = $state<OnboardingTheme>(loadTheme());
  let shareTelemetry = $state(false);
  let selectedAgents = $state<AgentId[]>([]);
  let starterPetId = $state<OfficialPetId | null>(null);
  let githubProfile = $state<GithubProfile | null>(null);
  let githubLoading = $state(false);
  let githubError = $state('');
  let completionError = $state('');
  let saving = $state(false);
  let headingEl = $state<HTMLHeadingElement | null>(null);
  let wasOpen = false;
  let agentInstallGeneration = 0;
  let agentStatuses = $state<Record<AgentId, AgentInstallStatus>>({
    claude: 'idle',
    codex: 'idle',
    cursor: 'idle',
  });
  let agentErrors = $state<Record<AgentId, string>>({ claude: '', codex: '', cursor: '' });

  const selectedPet = $derived(OFFICIAL_PETS.find((pet) => pet.id === starterPetId) ?? null);
  const connectedAgents = $derived(
    selectedAgents.filter((id) => agentStatuses[id] === 'connected'),
  );
  const hasInstallingAgents = $derived(
    agents.some((id) => agentStatuses[id] === 'installing'),
  );

  function clearAgentConnections() {
    agentInstallGeneration += 1;
    selectedAgents = [];
    agentStatuses = { claude: 'idle', codex: 'idle', cursor: 'idle' };
    agentErrors = { claude: '', codex: '', cursor: '' };
  }

  function resetDraft() {
    step = 'welcome';
    shareTelemetry = false;
    clearAgentConnections();
    starterPetId = null;
    githubProfile = null;
    githubLoading = false;
    githubError = '';
    completionError = '';
    saving = false;
  }

  $effect(() => {
    if (open && !wasOpen) resetDraft();
    wasOpen = open;
  });

  $effect(() => {
    step;
    if (!open) return;
    void tick().then(() => headingEl?.focus());
  });

  function goNext() {
    if (step === 'agents' && hasInstallingAgents) return;
    completionError = '';
    step = nextOnboardingStep(step);
  }

  function goBack() {
    completionError = '';
    step = previousOnboardingStep(step);
  }

  function setTheme(nextTheme: OnboardingTheme) {
    theme = nextTheme;
    try {
      localStorage.setItem(ONBOARDING_THEME_STORAGE_KEY, nextTheme);
    } catch {
      // The theme still applies for this session when storage is unavailable.
    }
  }

  async function handleGithubSignIn() {
    if (!onGithubSignIn || githubLoading) return;
    githubLoading = true;
    githubError = '';
    try {
      githubProfile = await onGithubSignIn();
      goNext();
    } catch (error) {
      githubError = String(error);
    } finally {
      githubLoading = false;
    }
  }

  async function toggleAgent(id: AgentId) {
    if (!agentAvailableOnPlatform(id, isWindows) || agentStatuses[id] === 'installing') return;
    if (selectedAgents.includes(id)) {
      selectedAgents = selectedAgents.filter((candidate) => candidate !== id);
      agentStatuses[id] = 'idle';
      agentErrors[id] = '';
      return;
    }

    selectedAgents = [...selectedAgents, id];
    agentStatuses[id] = 'installing';
    agentErrors[id] = '';
    const installGeneration = agentInstallGeneration;
    try {
      await invoke(hookCommandForAgent(id));
      if (installGeneration !== agentInstallGeneration) return;
      agentStatuses[id] = 'connected';
    } catch (error) {
      if (installGeneration !== agentInstallGeneration) return;
      console.warn('[onboarding] hook install failed:', id, error);
      selectedAgents = selectedAgents.filter((candidate) => candidate !== id);
      agentStatuses[id] = 'failed';
      agentErrors[id] = $_('onboarding.agents.failed');
    }
  }

  async function complete(result?: Partial<OnboardingResult>, allowInstalling = false) {
    if (saving || (hasInstallingAgents && !allowInstalling)) return;
    saving = true;
    completionError = '';
    try {
      await onComplete({
        mode: deriveOnboardingMode(connectedAgents),
        shareTelemetry,
        selectedAgents: connectedAgents,
        starterPetId,
        githubProfile,
        ...result,
      });
    } catch {
      completionError = $_('onboarding.errors.complete');
    } finally {
      saving = false;
    }
  }

  function setupLater() {
    clearAgentConnections();
    void complete(
      {
        mode: 'pet',
        shareTelemetry: false,
        selectedAgents: [],
        starterPetId: null,
        githubProfile: null,
      },
      true,
    );
  }

  function choosePetOnly() {
    clearAgentConnections();
    completionError = '';
    step = 'adopt';
  }

  function handlePetKeys(event: KeyboardEvent) {
    const group = event.currentTarget as HTMLElement;
    const target = event.target instanceof HTMLElement ? event.target.closest<HTMLElement>('[data-pet-id]') : null;
    if (event.key === 'Enter' && target?.dataset.petId === starterPetId) {
      event.preventDefault();
      void complete();
      return;
    }
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    const focusedId = target?.dataset.petId;
    const current = Math.max(
      0,
      OFFICIAL_PETS.findIndex((pet) => pet.id === (focusedId ?? starterPetId)),
    );
    const direction = event.key === 'ArrowRight' ? 1 : -1;
    const next = (current + direction + OFFICIAL_PETS.length) % OFFICIAL_PETS.length;
    starterPetId = OFFICIAL_PETS[next].id;
    void tick().then(() => {
      group.querySelector<HTMLElement>(`[data-pet-id="${starterPetId}"]`)?.focus();
    });
  }
</script>

{#if open}
  <div class="onboarding-overlay" data-theme={theme}>
    <section class="onboarding-shell" aria-label="PawBae onboarding" aria-busy={saving}>
      <header class="topbar">
        <div class="brand" aria-label="PawBae">
          <span class="brand-mark" aria-hidden="true">♥</span>
          <strong>PawBae</strong>
        </div>
        <OnboardingProgress {step} />
        <div class="topbar-actions">
          <div class="theme-control" role="group" aria-label={$_('onboarding.theme.label')}>
            {#each themes as option}
              <button
                class="theme-choice"
                class:active={theme === option.id}
                type="button"
                data-theme-choice={option.id}
                aria-pressed={theme === option.id}
                title={$_(`onboarding.theme.${option.id}`)}
                onclick={() => setTheme(option.id)}
              >
                <span aria-hidden="true">{option.icon}</span>
                <span class="theme-label">{$_(`onboarding.theme.${option.id}`)}</span>
              </button>
            {/each}
          </div>
          <button
            class="setup-later"
            type="button"
            onclick={setupLater}
            disabled={saving}
          >
            {$_('onboarding.common.setupLater')}
          </button>
        </div>
      </header>

      <main class="step-content">
        {#if step === 'welcome'}
          <div class="welcome-step" data-step="welcome">
            <div class="welcome-copy">
              <span class="time-chip">{$_('onboarding.common.aboutMinute')}</span>
              <h1 bind:this={headingEl} tabindex="-1">{$_('onboarding.welcome.title')}</h1>
              <p class="lead">{$_('onboarding.welcome.body')}</p>
              <div class="trust-note">
                <span aria-hidden="true">⌂</span>
                <span>{$_('onboarding.welcome.local')}</span>
              </div>
              <label class="telemetry-choice">
                <input type="checkbox" bind:checked={shareTelemetry} />
                <span>{$_('onboarding.welcome.telemetry')}</span>
              </label>
            </div>
            <div class="poster-wrap">
              <img src="/assets/onboarding/pet-family-poster.png" alt="Solu, Muru, Riffi and Luma" />
            </div>
          </div>
        {:else if step === 'github'}
          <div class="center-step" data-step="github">
            <div class="github-mark" aria-hidden="true">
              <svg viewBox="0 0 24 24" role="img" aria-label="GitHub">
                <path fill="currentColor" d="M12 .7a11.4 11.4 0 0 0-3.6 22.2c.6.1.8-.2.8-.6v-2.2c-3.3.7-4-1.4-4-1.4-.5-1.4-1.3-1.8-1.3-1.8-1.1-.7.1-.7.1-.7 1.2.1 1.8 1.2 1.8 1.2 1 1.8 2.8 1.3 3.4 1 .1-.8.4-1.3.8-1.6-2.7-.3-5.5-1.3-5.5-5.9 0-1.3.5-2.4 1.2-3.2-.1-.3-.5-1.5.1-3.2 0 0 1-.3 3.3 1.2a11.5 11.5 0 0 1 6 0C17 4.4 18 4.7 18 4.7c.6 1.7.2 2.9.1 3.2.8.8 1.2 1.9 1.2 3.2 0 4.6-2.8 5.6-5.5 5.9.4.4.8 1.1.8 2.2v3.2c0 .4.2.7.8.6A11.4 11.4 0 0 0 12 .7Z" />
              </svg>
            </div>
            <h1 bind:this={headingEl} tabindex="-1">{$_('onboarding.github.title')}</h1>
            <p class="lead">{$_('onboarding.github.body')}</p>
            {#if githubProfile}
              <div class="github-profile">
                {#if githubProfile.avatarUrl}<img src={githubProfile.avatarUrl} alt="" />{/if}
                <span>{githubProfile.displayName || githubProfile.login}</span>
              </div>
            {:else}
              <button
                class="primary github-action"
                type="button"
                data-action="github"
                disabled={!onGithubSignIn || githubLoading}
                onclick={handleGithubSignIn}
              >
                {githubLoading ? $_('common.loading') : $_('onboarding.github.action')}
              </button>
              {#if !onGithubSignIn}<p class="availability">{$_('onboarding.github.unavailable')}</p>{/if}
              {#if githubError}<p class="error-message">{githubError}</p>{/if}
            {/if}
            <button class="text-action" type="button" data-action="skip-github" onclick={goNext}>
              {$_('onboarding.github.skip')}
            </button>
          </div>
        {:else if step === 'agents'}
          <div class="agents-step" data-step="agents">
            <div class="step-heading">
              <h1 bind:this={headingEl} tabindex="-1">{$_('onboarding.agents.title')}</h1>
              <p class="lead">{$_('onboarding.agents.body')}</p>
            </div>
            <div class="agent-list">
              {#each agents as id}
                <AgentConnectionRow
                  {id}
                  selected={selectedAgents.includes(id)}
                  available={agentAvailableOnPlatform(id, isWindows)}
                  status={agentStatuses[id]}
                  error={agentErrors[id]}
                  onToggle={() => void toggleAgent(id)}
                  onRetry={() => void toggleAgent(id)}
                />
              {/each}
            </div>
          </div>
        {:else}
          <div class="adopt-step" data-step="adopt">
            <div class="step-heading adopt-heading">
              <h1 bind:this={headingEl} tabindex="-1">{$_('onboarding.adopt.title')}</h1>
              <p class="lead">{$_('onboarding.adopt.body')}</p>
            </div>
            <div class="pet-grid" role="radiogroup" tabindex="-1" aria-label={$_('onboarding.adopt.title')} onkeydown={handlePetKeys}>
              {#each OFFICIAL_PETS as pet, index}
                <PetAdoptionCard
                  {pet}
                  selected={starterPetId === pet.id}
                  tabIndex={starterPetId ? (starterPetId === pet.id ? 0 : -1) : (index === 0 ? 0 : -1)}
                  onSelect={() => { starterPetId = pet.id; }}
                />
              {/each}
            </div>
            <p class="sprite-notice">{$_('onboarding.adopt.spriteNotice')}</p>
          </div>
        {/if}
      </main>

      <footer class="footer">
        <div class="footer-start">
          {#if step !== 'welcome'}
            <button class="secondary" type="button" onclick={goBack} disabled={saving}>
              <span aria-hidden="true">←</span> {$_('onboarding.common.back')}
            </button>
          {/if}
        </div>
        <div class="footer-center">
          {#if step === 'agents'}
            <button class="text-action" type="button" data-action="pet-only" onclick={choosePetOnly}>{$_('onboarding.agents.petOnly')}</button>
          {:else if step === 'adopt'}
            <span>{$_('onboarding.adopt.changeLater')}</span>
          {/if}
          {#if completionError}<span class="error-message">{completionError}</span>{/if}
        </div>
        <div class="footer-end">
          {#if step === 'welcome'}
            <button class="primary" type="button" data-action="continue" onclick={goNext}>
              {$_('onboarding.common.continue')} <span aria-hidden="true">→</span>
            </button>
          {:else if step === 'agents'}
            <button
              class="primary"
              type="button"
              data-action="agents-continue"
              disabled={hasInstallingAgents}
              onclick={goNext}
            >
              {$_('onboarding.common.continue')} <span aria-hidden="true">→</span>
            </button>
          {:else if step === 'adopt'}
            <button
              class="primary adopt-action"
              type="button"
              data-action="complete-adoption"
              disabled={!selectedPet || saving || hasInstallingAgents}
              onclick={() => void complete()}
            >
              {selectedPet
                ? $_('onboarding.adopt.action', { values: { name: $_(`onboarding.adopt.${selectedPet.id}Name`) } })
                : $_('onboarding.adopt.title')}
            </button>
          {/if}
        </div>
      </footer>
    </section>
  </div>
{/if}

<style>
  .onboarding-overlay {
    --ob-canvas: #fbfaf8;
    --ob-surface: #ffffff;
    --ob-subtle: #f4f1f1;
    --ob-text: #2e2b31;
    --ob-text-muted: #635e67;
    --ob-border: #ded8dc;
    --ob-border-strong: #aaa3ad;
    --ob-action: #3d4e9e;
    --ob-action-hover: #334487;
    --ob-action-text: #ffffff;
    --ob-focus: #596bc0;
    --ob-ink: #393640;
    --ob-on-ink: #ffffff;
    --ob-success: #2e6c58;
    --ob-danger: #a94452;
    position: fixed;
    inset: 0;
    z-index: 2100;
    display: grid;
    place-items: center;
    padding: 12px;
    background: rgba(35, 31, 36, 0.34);
    color: var(--ob-text);
    font-family: Inter, 'Noto Sans SC', 'Segoe UI', 'PingFang SC', sans-serif;
  }
  .onboarding-shell {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    width: min(960px, calc(100vw - 24px));
    height: min(600px, calc(100vh - 24px));
    min-height: min(520px, calc(100vh - 24px));
    overflow: hidden;
    border-radius: 18px;
    background: var(--ob-canvas);
    box-shadow: 0 12px 36px rgba(27, 22, 28, 0.22);
  }
  .topbar {
    display: grid;
    grid-template-columns: 130px minmax(0, 1fr) 270px;
    align-items: center;
    gap: 20px;
    min-height: 76px;
    padding: 0 24px;
    border-bottom: 1px solid var(--ob-border);
    background: var(--ob-surface);
  }
  .brand { display: flex; align-items: center; gap: 10px; color: var(--ob-ink); }
  .brand strong { font-size: 17px; letter-spacing: -0.02em; }
  .brand-mark {
    display: grid;
    width: 30px;
    height: 30px;
    place-items: center;
    border-radius: 10px;
    background: #f5afc8;
    color: #3d4e9e;
    font-size: 15px;
  }
  .topbar-actions { display: flex; align-items: center; justify-content: flex-end; gap: 12px; }
  .theme-control {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 2px;
    border-radius: 10px;
    background: var(--ob-subtle);
  }
  .theme-choice {
    display: flex;
    align-items: center;
    gap: 4px;
    min-height: 30px;
    padding: 0 7px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--ob-text-muted);
    font-size: 11px;
    font-weight: 650;
    cursor: pointer;
  }
  .theme-choice:hover { color: var(--ob-text); }
  .theme-choice.active {
    background: var(--ob-surface);
    color: var(--ob-text);
    box-shadow: 0 1px 2px rgba(38, 30, 36, 0.14);
  }
  button { font: inherit; }
  .setup-later, .text-action {
    border: 0;
    background: transparent;
    color: var(--ob-text-muted);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }
  .setup-later { white-space: nowrap; }
  button:focus-visible { outline: 2px solid var(--ob-focus); outline-offset: 3px; }
  button:disabled { cursor: not-allowed; opacity: 0.48; }
  .step-content { min-height: 0; overflow: auto; padding: 28px 38px 22px; }
  h1 {
    margin: 0;
    color: var(--ob-text);
    font-family: 'M PLUS Rounded 1c', 'Noto Sans SC', 'Segoe UI', sans-serif;
    font-size: 26px;
    line-height: 1.22;
    letter-spacing: -0.025em;
    text-wrap: balance;
  }
  h1:focus { outline: none; }
  .lead { margin: 10px 0 0; color: var(--ob-text-muted); font-size: 14px; line-height: 1.55; text-wrap: pretty; }
  .welcome-step {
    display: grid;
    grid-template-columns: minmax(280px, 0.82fr) minmax(380px, 1.18fr);
    align-items: center;
    gap: 34px;
    height: 100%;
    max-width: 860px;
    margin: 0 auto;
  }
  .welcome-copy { display: grid; align-content: center; }
  .welcome-copy h1 { font-size: 32px; }
  .time-chip {
    justify-self: start;
    margin-bottom: 14px;
    padding: 5px 9px;
    border-radius: 999px;
    background: #e6e9fa;
    color: #455a96;
    font-size: 11px;
    font-weight: 700;
  }
  .trust-note {
    display: flex;
    align-items: center;
    gap: 9px;
    margin-top: 20px;
    color: var(--ob-success);
    font-size: 13px;
    font-weight: 650;
  }
  .trust-note > span:first-child {
    display: grid;
    width: 26px;
    height: 26px;
    place-items: center;
    border-radius: 8px;
    background: color-mix(in srgb, var(--ob-success) 12%, var(--ob-surface));
  }
  .telemetry-choice {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr);
    gap: 9px;
    align-items: start;
    margin-top: 18px;
    color: var(--ob-text-muted);
    font-size: 11px;
    line-height: 1.45;
    cursor: pointer;
  }
  .telemetry-choice input { margin: 1px 0 0; accent-color: var(--ob-action); }
  .poster-wrap {
    display: grid;
    place-items: center;
    align-self: stretch;
    min-height: 330px;
    overflow: hidden;
    border-radius: 16px;
    background: #e6e9fa;
  }
  .poster-wrap img { width: 100%; height: auto; max-height: 100%; object-fit: contain; display: block; }
  .center-step {
    display: grid;
    justify-items: center;
    align-content: center;
    min-height: 100%;
    max-width: 540px;
    margin: 0 auto;
    text-align: center;
  }
  .github-mark {
    display: grid;
    width: 68px;
    height: 68px;
    margin-bottom: 20px;
    place-items: center;
    border-radius: 16px;
    background: var(--ob-ink);
    color: var(--ob-on-ink);
  }
  .github-mark svg { width: 36px; height: 36px; }
  .github-action { margin-top: 24px; min-width: 220px; }
  .availability { margin: 12px 0 0; color: var(--ob-text-muted); font-size: 12px; }
  .center-step > .text-action { margin-top: 16px; color: var(--ob-action); }
  .github-profile { display: flex; align-items: center; gap: 10px; margin-top: 24px; }
  .github-profile img { width: 36px; height: 36px; border-radius: 50%; }
  .agents-step { max-width: 680px; margin: 0 auto; }
  .step-heading { text-align: center; }
  .agent-list { display: grid; gap: 10px; margin-top: 24px; }
  .adopt-step { max-width: 900px; margin: 0 auto; }
  .adopt-heading { margin-bottom: 20px; }
  .pet-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 14px; }
  .sprite-notice { margin: 10px 0 0; color: var(--ob-text-muted); font-size: 10px; text-align: center; }
  .footer {
    display: grid;
    grid-template-columns: minmax(160px, 1fr) minmax(240px, 1.5fr) minmax(160px, 1fr);
    align-items: center;
    gap: 16px;
    min-height: 74px;
    padding: 0 24px;
    border-top: 1px solid var(--ob-border);
    background: var(--ob-surface);
  }
  .footer-start { justify-self: start; }
  .footer-center { display: grid; justify-items: center; gap: 3px; color: var(--ob-text-muted); font-size: 11px; text-align: center; }
  .footer-end { justify-self: end; }
  .primary, .secondary {
    min-height: 40px;
    padding: 0 16px;
    border-radius: 10px;
    font-size: 13px;
    font-weight: 700;
    cursor: pointer;
  }
  .primary { border: 0; background: var(--ob-action); color: var(--ob-action-text); }
  .primary:hover:not(:disabled) { background: var(--ob-action-hover); }
  .secondary { border: 1px solid var(--ob-border-strong); background: var(--ob-surface); color: var(--ob-text); }
  .adopt-action { min-width: 150px; }
  .error-message { color: var(--ob-danger); font-size: 11px; line-height: 1.4; }
  .onboarding-overlay[data-theme='dark'] {
    --ob-canvas: #242226;
    --ob-surface: #2d2a2f;
    --ob-subtle: #36323a;
    --ob-text: #f7f3f5;
    --ob-text-muted: #c8c0c8;
    --ob-border: #514b55;
    --ob-border-strong: #746d77;
    --ob-action: #b3c7f0;
    --ob-action-hover: #c9d6f5;
    --ob-action-text: #242226;
    --ob-focus: #c9d6f5;
    --ob-ink: #f7f3f5;
    --ob-on-ink: #242226;
    --ob-success: #a8e0c0;
    --ob-danger: #ff9aa7;
  }
  @media (prefers-color-scheme: dark) {
    .onboarding-overlay[data-theme='system'] {
      --ob-canvas: #242226;
      --ob-surface: #2d2a2f;
      --ob-subtle: #36323a;
      --ob-text: #f7f3f5;
      --ob-text-muted: #c8c0c8;
      --ob-border: #514b55;
      --ob-border-strong: #746d77;
      --ob-action: #b3c7f0;
      --ob-action-hover: #c9d6f5;
      --ob-action-text: #242226;
      --ob-focus: #c9d6f5;
      --ob-ink: #f7f3f5;
      --ob-on-ink: #242226;
      --ob-success: #a8e0c0;
      --ob-danger: #ff9aa7;
    }
  }
  @media (max-width: 940px) {
    .topbar { grid-template-columns: 100px minmax(0, 1fr) 166px; padding-inline: 18px; gap: 12px; }
    .theme-label { display: none; }
    .theme-choice { width: 30px; justify-content: center; padding: 0; }
    .topbar-actions { gap: 8px; }
  }
  @media (max-width: 820px) {
    .step-content { padding-inline: 24px; }
    .welcome-step { grid-template-columns: 1fr 1fr; gap: 22px; }
    .adopt-heading { margin-bottom: 16px; }
    .pet-grid { gap: 8px; }
  }
  @media (max-width: 640px) {
    .topbar { grid-template-columns: auto minmax(0, 1fr) 166px; padding-inline: 14px; gap: 8px; }
    .brand strong { display: none; }
    .setup-later { max-width: 70px; }
    .step-content { padding-inline: 16px; }
    .welcome-step { grid-template-columns: 1fr; }
    .poster-wrap { min-height: 220px; }
    .pet-grid { gap: 6px; }
    .footer { grid-template-columns: 1fr auto; }
    .footer-center { display: none; }
  }
  @media (prefers-reduced-motion: reduce) {
    /* biome-ignore lint/complexity/noImportantStyles: Global accessibility override must win over component transitions. */
    *, *::before, *::after { scroll-behavior: auto !important; transition-duration: 0.01ms !important; animation-duration: 0.01ms !important; }
  }
</style>
