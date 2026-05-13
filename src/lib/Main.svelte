<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import MascotView from './MascotView.svelte';
  import Panel from './Panel.svelte';
  import Onboarding from './Onboarding.svelte';
  import UpdateModal from './UpdateModal.svelte';
  import type { UpdateModalInfo } from './UpdateModal.svelte';
  import { loadCodexPets, loadDefaultCodexPet } from './codexPet';
  import type { CodexPet } from './codexPet';
  import type { AppMode } from './types';
  import { settingsStore } from './stores/settings.svelte';
  import { windowStore } from './stores/window.svelte';
  import { agentStore } from './stores/agents.svelte';
  import { sessionStore } from './stores/sessions.svelte';
  import { petStore } from './stores/pet.svelte';

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

    const cleanups: (() => void)[] = [];

    listen<{ recording: boolean; error?: string }>('voice-status', (e) => {
      voiceRecording = e.payload.recording;
      voiceError = e.payload.error || '';
      if (!e.payload.recording) {
        setTimeout(() => { voiceText = ''; }, 2000);
      }
    }).then((u) => cleanups.push(u));

    listen<{ text: string; is_final: boolean }>('voice-transcript', (e) => {
      voiceText = e.payload.text;
    }).then((u) => cleanups.push(u));

    listen<boolean>('stroll-mode-changed', (e) => {
      windowStore.setStrollActive(e.payload);
    }).then((u) => cleanups.push(u));

    return () => {
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
    } else {
      startModePolling();
    }
  }

  function startModePolling() {
    if (settingsStore.appMode === 'coding') {
      agentStore.startPolling();
      sessionStore.startPolling();
    }
  }

  async function handleModeSelect(mode: AppMode) {
    showOnboarding = false;
    await settingsStore.setAppMode(mode);
    startModePolling();
  }
</script>

<div class="root" data-tauri-drag-region>
  <MascotView {pet} {voiceRecording} {voiceText} {voiceError} />
  <Panel />

  <Onboarding open={showOnboarding} onSelect={handleModeSelect} />

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
