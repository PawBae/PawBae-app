<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { agentStore } from '../stores/agents.svelte';
  import { petStore } from '../stores/pet.svelte';
  import { sessionStore } from '../stores/sessions.svelte';
  import { settingsStore } from '../stores/settings.svelte';
  import { windowStore } from '../stores/window.svelte';
  import type { AppMode, UpdateModalInfo } from '../types';
  import type { CodexPet } from '../utils/codex-pet';
  import { loadCodexPets, loadDefaultCodexPet } from '../utils/codex-pet';
  import MascotView from './MascotView.svelte';
  import Onboarding from './Onboarding.svelte';
  import Panel from './Panel.svelte';
  import SettingsPanel from './settings/SettingsPanel.svelte';
  import UpdateModal from './UpdateModal.svelte';

  let pet = $state<CodexPet | null>(null);
  let showOnboarding = $state(false);

  let voiceRecording = $state(false);
  let voiceText = $state('');
  let voiceError = $state('');

  let updateOpen = $state(false);
  let updatePhase = $state<'available' | 'downloading' | 'ready_to_restart'>('available');
  let updateInfo = $state<UpdateModalInfo | null>(null);
  let updateProgress = $state<number | null>(null);
  let updateProgressStage = $state('');

  $effect(() => {
    init();

    let disposed = false;
    const cleanups: (() => void)[] = [];

    function addListener<T>(event: string, handler: (e: { payload: T }) => void) {
      listen<T>(event, handler).then((u) => {
        if (disposed) u();
        else cleanups.push(u);
      });
    }

    addListener<{ recording: boolean; error?: string }>('voice-status', (e) => {
      voiceRecording = e.payload.recording;
      voiceError = e.payload.error || '';
      if (!e.payload.recording) {
        setTimeout(() => { voiceText = ''; }, 2000);
      }
    });

    addListener<{ text: string; is_final: boolean }>('voice-transcript', (e) => {
      voiceText = e.payload.text;
    });

    addListener<boolean>('stroll-mode-changed', (e) => {
      windowStore.setStrollActive(e.payload);
    });

    addListener('tray-open-settings', () => {
      openSettings();
    });

    // Reward model (P1-C): hydrate persisted coins/ledger, then listen for agent
    // completions and user input. App-wide on purpose — agent stops happen in coding
    // mode while the coin balance shows in pet mode.
    petStore.init().then((dispose) => {
      if (disposed) dispose();
      else cleanups.push(dispose);
    });

    return () => {
      disposed = true;
      for (const fn of cleanups) fn();
      agentStore.stopPolling();
      sessionStore.stopPolling();
    };
  });

  async function init() {
    try {
      await settingsStore.loadSettings();
    } catch (e) {
      console.warn('[init] settings load failed:', e);
    }

    try {
      const allPets = await loadCodexPets();
      const selectedId = settingsStore.miniPetId;
      pet = allPets.find((p) => p.id === selectedId) || (await loadDefaultCodexPet());
    } catch (e) {
      console.error('[init] pet loading failed:', e);
      pet = await loadDefaultCodexPet().catch(() => null);
    }

    if (!settingsStore.appMode) {
      showOnboarding = true;
    }
  }

  function startModePolling() {
    if (settingsStore.appMode === 'coding') {
      agentStore.startPolling();
      sessionStore.startPolling();
    } else {
      agentStore.stopPolling();
      sessionStore.stopPolling();
    }
  }

  async function handleModeSelect(mode: AppMode) {
    showOnboarding = false;
    await settingsStore.setAppMode(mode);
  }

  $effect(() => {
    if (settingsStore.appMode) startModePolling();
  });

  async function openSettings() {
    if (windowStore.settingsOpen) return;
    windowStore.setSettingsOpen(true);
    try {
      await invoke('set_mini_size', { restore: false });
    } catch (e) {
      console.warn('[settings] set_mini_size failed:', e);
    }
  }

  async function closeSettings() {
    if (!windowStore.settingsOpen) return;
    windowStore.setSettingsOpen(false);
    try {
      await invoke('set_mini_size', { restore: true, mascotScale: settingsStore.mascotScale });
    } catch (e) {
      console.warn('[settings] restore size failed:', e);
    }
  }
</script>

<div class="root" data-tauri-drag-region={windowStore.settingsOpen ? undefined : ''}>
  <MascotView {pet} {voiceRecording} {voiceText} {voiceError} />
  <Panel />

  <Onboarding open={showOnboarding} onSelect={handleModeSelect} />

  <SettingsPanel open={windowStore.settingsOpen} onClose={closeSettings} />

  <UpdateModal
    open={updateOpen}
    phase={updatePhase}
    info={updateInfo}
    progress={updateProgress}
    progressStage={updateProgressStage}
    onLater={() => { updateOpen = false; }}
    onSkipVersion={() => { updateOpen = false; }}
    onUpdateNow={() => { updatePhase = 'downloading'; }}
    onRestartNow={() => {}}
  />
</div>

<style>
  .root {
    width: 100vw;
    height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    background: transparent;
    overflow: hidden;
  }
</style>
