<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import MiniPetMascot from './MiniPetMascot.svelte';
  import VoiceBubble from './VoiceBubble.svelte';
  import type { CodexPet, CodexPetState } from './codexPet';
  import { petStateToCodexState } from './codexPet';
  import { createPhysicsLoop } from './petPhysics';
  import { windowStore } from './stores/window.svelte';
  import { petStore } from './stores/pet.svelte';
  import { agentStore } from './stores/agents.svelte';
  import { settingsStore } from './stores/settings.svelte';

  let {
    pet,
    voiceRecording = false,
    voiceText = '',
    voiceError = '',
  }: {
    pet: CodexPet | null;
    voiceRecording?: boolean;
    voiceText?: string;
    voiceError?: string;
  } = $props();

  const isWindows = typeof navigator !== 'undefined' && navigator.userAgent.includes('Windows');

  const sourceState = $derived<'idle' | 'working' | 'compacting' | 'waiting'>(
    settingsStore.appMode === 'coding' && agentStore.anySessionActive
      ? 'working'
      : 'idle'
  );

  const physicsEnabled = $derived(!!pet?.physics?.enabled);

  let physicsSprite = $state<string | null>(null);

  const spriteState = $derived<CodexPetState>(
    physicsSprite ?? petStateToCodexState(pet, sourceState)
  );

  const mascotSize = $derived(Math.round(60 * settingsStore.mascotScale));

  $effect(() => {
    if (!isWindows) {
      invoke('set_efficiency_hover_tracking', { active: true }).catch(() => {});
    }
    const unlisten = listen<boolean>('mini-mascot-hover', (e) => {
      windowStore.setMascotHover(e.payload);
    });
    return () => {
      unlisten.then((u) => u());
      if (!isWindows) {
        invoke('set_efficiency_hover_tracking', { active: false }).catch(() => {});
      }
    };
  });

  // Physics loop — paused when settings panel is open
  $effect(() => {
    const currentPet = pet;
    if (!currentPet?.physics?.enabled) return;
    if (windowStore.settingsOpen) return;

    invoke('set_stroll_mode', { enabled: true }).catch(() => {});
    invoke('set_throw_tracking', { enabled: true }).catch(() => {});

    const loop = createPhysicsLoop({
      pet: currentPet,
      enabled: true,
    });
    loop.start();

    const spriteInterval = setInterval(() => {
      physicsSprite = loop.spriteName;
    }, 30);

    const unlisteners: Promise<() => void>[] = [
      listen('mini-mascot-drag-start', () => {
        loop.setPinched(true);
      }),
      listen('mini-mascot-drag-end', () => {
        loop.setPinched(false);
      }),
      listen<{ vx: number; vy: number }>('mini-mascot-drag-throw', (e) => {
        loop.setPinched(false);
        loop.beginThrow(e.payload.vx, e.payload.vy);
      }),
    ];

    return () => {
      loop.stop();
      clearInterval(spriteInterval);
      physicsSprite = null;
      for (const p of unlisteners) p.then((u) => u());
      invoke('set_stroll_mode', { enabled: false }).catch(() => {});
      invoke('set_throw_tracking', { enabled: false }).catch(() => {});
    };
  });

  function handleClick() {
    if (settingsStore.appMode === 'pet') {
      petStore.applyHeadpat();
    } else {
      windowStore.toggle();
    }
  }

</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="mascot-view"
  data-tauri-drag-region={windowStore.settingsOpen ? undefined : ''}
  onclick={handleClick}
  oncontextmenu={(e) => e.preventDefault()}
  style="width: {mascotSize}px; height: {mascotSize}px;"
>
  {#if pet}
    <MiniPetMascot
      {pet}
      baseState={spriteState}
      size={mascotSize}
      enableHoverJump
      externalHover={windowStore.mascotHover}
      useExternalHover={!isWindows}
      suppressHover={windowStore.moveMode}
    />
  {/if}

  <VoiceBubble
    visible={voiceRecording || !!voiceText}
    text={voiceText}
    recording={voiceRecording}
    error={voiceError}
    petMode={settingsStore.appMode === 'pet'}
  />
</div>

<style>
  .mascot-view {
    cursor: grab;
    user-select: none;
    -webkit-user-select: none;
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
