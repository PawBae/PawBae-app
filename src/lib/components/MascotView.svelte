<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { agentStore } from '../stores/agents.svelte';
  import { petStore } from '../stores/pet.svelte';
  import { settingsStore } from '../stores/settings.svelte';
  import { windowStore } from '../stores/window.svelte';
  import type { UserInputEvent } from '../types';
  import type { CodexPet, CodexPetState } from '../utils/codex-pet';
  import { petStateToCodexState } from '../utils/codex-pet';
  import { tryInvoke } from '../utils/invoke';
  import type { PhysicsState } from '../utils/pet-physics';
  import { createPhysicsLoop } from '../utils/pet-physics';
  import {
    endReaction,
    initialReactionState,
    reactionSpriteFor,
    requestReaction,
  } from '../utils/reaction-machine';
  import MiniPetMascot from './MiniPetMascot.svelte';
  import VoiceBubble from './VoiceBubble.svelte';

  interface MascotViewProps {
    pet: CodexPet | null;
    voiceRecording?: boolean;
    voiceText?: string;
    voiceError?: string;
  }

  let {
    pet,
    voiceRecording = false,
    voiceText = '',
    voiceError = '',
  }: MascotViewProps = $props();

  const isWindows = typeof navigator !== 'undefined' && navigator.userAgent.includes('Windows');

  const sourceState = $derived<'idle' | 'working' | 'compacting' | 'waiting'>(
    settingsStore.appMode === 'coding' && agentStore.anySessionActive
      ? 'working'
      : 'idle'
  );

  const physicsEnabled = $derived(!!pet?.physics?.enabled);

  let physicsSprite = $state<string | null>(null);
  let physicsState = $state<PhysicsState | null>(null); // null while physics disabled

  const spriteState = $derived<CodexPetState>(
    physicsSprite ?? petStateToCodexState(pet, sourceState)
  );

  // One-shot input-reaction overlay (P1-B). The pure machine lives in reaction-machine.ts;
  // this component owns the listener, the busy-guard, and the revert timer.
  const REACTION_MS = 350; // beat duration ≈ one play of the reaction row (tune in acceptance)
  let reactionSprite = $state<CodexPetState | null>(null);
  const reaction = initialReactionState();
  let reactionTimer: ReturnType<typeof setTimeout> | null = null;

  const mascotSize = $derived(Math.round(60 * settingsStore.mascotScale));

  $effect(() => {
    // macOS: the efficiency hover poll drives hover/drag in BOTH modes, so it
    // must stay active regardless of appMode. Windows: pet mode gets its poll
    // thread from set_pet_mode_window; hover tracking may only run in coding
    // mode because it forces the whole window interactive (no click-through).
    const needsHoverTracking = !isWindows || settingsStore.appMode === 'coding';
    if (needsHoverTracking) {
      tryInvoke('set_efficiency_hover_tracking', { active: true });
    }
    let disposed = false;
    let unlisten: (() => void) | null = null;
    listen<boolean>('mini-mascot-hover', (e) => {
      windowStore.setMascotHover(e.payload);
    }).then((u) => {
      if (disposed) u();
      else unlisten = u;
    });
    return () => {
      disposed = true;
      unlisten?.();
      if (needsHoverTracking) {
        tryInvoke('set_efficiency_hover_tracking', { active: false });
      }
    };
  });

  // Physics loop — paused when settings panel is open
  $effect(() => {
    const currentPet = pet;
    if (!currentPet?.physics?.enabled) return;
    if (windowStore.settingsOpen) return;

    tryInvoke('set_stroll_mode', { enabled: true });
    tryInvoke('set_throw_tracking', { enabled: true });

    const loop = createPhysicsLoop({
      pet: currentPet,
      enabled: true,
    });
    loop.start();
    // Sample the LIVE physics state up front so the reaction busy-guard isn't stale for the
    // first interval tick: the loop's snapshot initialises to 'on_floor' but the real start
    // state is 'falling'. getPhysicsState() returns the live state, not the lagging snapshot.
    physicsState = loop.getPhysicsState();

    const spriteInterval = setInterval(() => {
      physicsSprite = loop.spriteName;
      physicsState = loop.getPhysicsState();
    }, 30);

    let disposed = false;
    const listenerCleanups: (() => void)[] = [];

    function addListener<T>(event: string, handler: (e: { payload: T }) => void) {
      listen<T>(event, handler).then((u) => {
        if (disposed) u();
        else listenerCleanups.push(u);
      });
    }

    addListener('mini-mascot-drag-start', () => {
      loop.setPinched(true);
    });
    addListener('mini-mascot-drag-end', () => {
      loop.setPinched(false);
    });
    addListener<{ vx: number; vy: number }>('mini-mascot-drag-throw', (e) => {
      loop.setPinched(false);
      loop.beginThrow(e.payload.vx, e.payload.vy);
    });

    return () => {
      disposed = true;
      loop.stop();
      clearInterval(spriteInterval);
      physicsSprite = null;
      physicsState = null;
      for (const fn of listenerCleanups) fn();
      tryInvoke('set_stroll_mode', { enabled: false });
      tryInvoke('set_throw_tracking', { enabled: false });
    };
  });

  function handleUserInput(ev: UserInputEvent) {
    // Suppress a reaction while the pet is being manipulated or otherwise busy, so it can
    // never interrupt drag/throw/hover/headpat or the settings interaction. Guard on the
    // discrete physics STATE (physicsSprite is always non-null once physics runs).
    const busy =
      (physicsState !== null && physicsState !== 'on_floor') || // drag/throw/fall/bounce/wall
      windowStore.mascotHover || // hover-jump in flight
      petStore.currentAction === 'headpat' || // headpat beat
      windowStore.settingsOpen; // settings panel open
    if (!requestReaction(reaction, ev, { busy })) return; // coalesced or guarded → drop
    reactionSprite = reactionSpriteFor(reaction) as CodexPetState;
    if (reactionTimer) clearTimeout(reactionTimer);
    reactionTimer = setTimeout(() => {
      endReaction(reaction);
      reactionSprite = null;
      reactionTimer = null;
    }, REACTION_MS);
  }

  // One-shot pet reaction to batched global input (Tauri "user-input" from P1-A; macOS-only).
  // Capture is OFF by default in the backend (privacy) — opt in for this component's
  // lifetime, mirroring the set_efficiency_hover_tracking lifecycle above. Safe everywhere:
  // non-macOS gets a no-op listener and start/stop are idempotent.
  $effect(() => {
    // TODO(settings phase): surface the returned ListenerStatus (e.g. keyboard off when
    // macOS Accessibility is denied) in the settings UI instead of discarding it.
    tryInvoke('set_input_tracking', { active: true });
    let disposed = false;
    let unlisten: (() => void) | null = null;
    listen<UserInputEvent>('user-input', (e) => handleUserInput(e.payload)).then((u) => {
      if (disposed) u();
      else unlisten = u;
    });
    return () => {
      disposed = true;
      unlisten?.();
      tryInvoke('set_input_tracking', { active: false });
      if (reactionTimer) {
        clearTimeout(reactionTimer);
        reactionTimer = null;
      }
    };
  });

  // DEV-only synthetic emitter for manual testing without macOS input capture. Registered in
  // an effect so it is removed on unmount (no stale closure retained), and stripped from
  // production builds by the `import.meta.env.DEV` guard.
  $effect(() => {
    if (!import.meta.env.DEV) return;
    const w = window as unknown as { __pawbaeEmitInput?: (kind: 'keyboard' | 'mouse') => void };
    const emit = (kind: 'keyboard' | 'mouse') => handleUserInput({ kind, count: 1, at: Date.now() });
    w.__pawbaeEmitInput = emit;
    return () => {
      if (w.__pawbaeEmitInput === emit) w.__pawbaeEmitInput = undefined;
    };
  });

  function handleClick() {
    if (settingsStore.appMode === 'pet') {
      petStore.applyHeadpat();
    } else {
      windowStore.toggle();
    }
  }

  function handleContextMenu(e: MouseEvent) {
    e.preventDefault();
    // Pet mode reserves left-click for headpats, so the stats/feed panel opens on
    // right-click instead (the desktop-pet convention). Coding mode keeps
    // right-click inert; its panel already toggles on left-click.
    if (settingsStore.appMode === 'pet') {
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
  oncontextmenu={handleContextMenu}
  style="width: {mascotSize}px; height: {mascotSize}px;"
>
  {#if pet}
    <MiniPetMascot
      {pet}
      baseState={spriteState}
      size={mascotSize}
      enableHoverJump
      externalHover={windowStore.mascotHover}
      useExternalHover
      suppressHover={windowStore.moveMode}
      {reactionSprite}
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
