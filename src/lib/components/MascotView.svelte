<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { untrack } from 'svelte';
  import { agentStore } from '../stores/agents.svelte';
  import { petStore } from '../stores/pet.svelte';
  import { sessionStore } from '../stores/sessions.svelte';
  import { settingsStore } from '../stores/settings.svelte';
  import { windowStore } from '../stores/window.svelte';
  import type { UserInputEvent } from '../types';
  import { aggregateSessions, isOverloaded, mascotStateFor } from '../utils/agent-activity';
  import { dayPartFor } from '../utils/circadian';
  import type { CodexPet, CodexPetState } from '../utils/codex-pet';
  import { petStateToCodexState } from '../utils/codex-pet';
  import { STYLE_FROM_STAGE } from '../utils/evolution';
  import {
    availableIdleActions,
    IDLE_ACTION_MS,
    nextIdleDelayMs,
    pickIdleActionFor,
  } from '../utils/idle-actions';
  import { tryInvoke } from '../utils/invoke';
  import { keyboardMoveDelta } from '../utils/keyboard-control';
  import type { PhysicsState } from '../utils/pet-physics';
  import { createPhysicsLoop } from '../utils/pet-physics';
  import {
    endReaction,
    initialReactionState,
    reactionSpriteFor,
    requestReaction,
  } from '../utils/reaction-machine';
  import AgentBubble from './AgentBubble.svelte';
  import CelebrationBubble from './CelebrationBubble.svelte';
  import MiniPetMascot from './MiniPetMascot.svelte';
  import PetReplyBubble from './PetReplyBubble.svelte';
  import VoiceBubble from './VoiceBubble.svelte';

  interface MascotViewProps {
    pet: CodexPet | null;
    voiceRecording?: boolean;
    voiceText?: string;
    voiceError?: string;
    /** Pet's localized reply to the last final transcript (voice Phase C). */
    voiceReply?: string;
    /** CodexPetState emotion to play for the last intent, or null. */
    voiceEmotion?: string | null;
    /** Bumps on every final transcript so a repeated intent still replays. */
    voiceNonce?: number;
  }

  let {
    pet,
    voiceRecording = false,
    voiceText = '',
    voiceError = '',
    voiceReply = '',
    voiceEmotion = null,
    voiceNonce = 0,
  }: MascotViewProps = $props();

  const isWindows = typeof navigator !== 'undefined' && navigator.userAgent.includes('Windows');

  // Live agent workload (Phase 2): collapse the 2s-polled session statuses into the one
  // emotion the mascot, the activity bubble and the overload aura share. Pet mode has no
  // sessions, so it always reads idle.
  const activity = $derived(
    settingsStore.appMode === 'coding'
      ? aggregateSessions(sessionStore.claudeSessions)
      : { waiting: 0, compacting: 0, working: 0 }
  );

  // The mascot now mirrors waiting/compacting too, not just a binary working/idle —
  // pets map these via stateMap (yoonie hides when waiting; default pets play the
  // waiting row). anySessionActive keeps OpenClaw agents (no hook status) reading busy.
  const sourceState = $derived<'idle' | 'working' | 'compacting' | 'waiting'>(
    settingsStore.appMode === 'coding'
      ? mascotStateFor(activity, agentStore.anySessionActive)
      : 'idle'
  );

  // 3+ busy sessions in parallel: the pet shows nervous "overload" energy (roadmap 2.1).
  const overloaded = $derived(settingsStore.appMode === 'coding' && isOverloaded(activity));

  const physicsEnabled = $derived(!!pet?.physics?.enabled);

  let physicsSprite = $state<string | null>(null);
  let physicsState = $state<PhysicsState | null>(null); // null while physics disabled
  // $state so the pause effect below re-fires when a NEW loop is created (pet switch /
  // settings close) while the panel is already expanded.
  let physicsLoop = $state<ReturnType<typeof createPhysicsLoop> | null>(null);

  const spriteState = $derived<CodexPetState>(
    physicsSprite ?? petStateToCodexState(pet, sourceState)
  );

  // Idle micro-actions (Phase 5): wire up the long-dormant "Random Action Interval"
  // setting. While truly idle the pet occasionally plays a short personality row from
  // its OWN sheet (yoonie blinks/pounces; standard pets wave). Reaction always outranks
  // it, so typing never gets stepped on.
  let idleSprite = $state<CodexPetState | null>(null);
  const idleActions = $derived(availableIdleActions(pet?.animations));

  // One-shot input-reaction overlay (P1-B). The pure machine lives in reaction-machine.ts;
  // this component owns the listener, the busy-guard, and the revert timer.
  const REACTION_MS = 350; // beat duration ≈ one play of the reaction row (tune in acceptance)
  const KEYBOARD_MOVE_MODE_MS = 140;
  let reactionSprite = $state<CodexPetState | null>(null);
  const reaction = initialReactionState();
  let reactionTimer: ReturnType<typeof setTimeout> | null = null;
  let keyboardMoveTimer: ReturnType<typeof setTimeout> | null = null;

  // Voice emotion overlay (voice Phase C): a recognized intent plays a longer-lived
  // emotion (happy/sleep/eat/angry) on its own slot, kept separate from the 350ms
  // keyboard/mouse reaction machine so the two never clobber each other.
  const VOICE_EMOTION_MS = 2500;
  let voiceEmotionSprite = $state<CodexPetState | null>(null);
  let voiceEmotionTimer: ReturnType<typeof setTimeout> | null = null;

  // Overlay slot fed to MiniPetMascot: a live input reaction wins over a voice emotion,
  // which wins over an idle micro-action, which sits above the base/physics sprite.
  const overlaySprite = $derived<CodexPetState | null>(
    reactionSprite ?? voiceEmotionSprite ?? idleSprite
  );

  // The pet must not be mid-manipulation for a beat to steal its animation. Shared by the
  // keyboard/mouse reaction and the voice emotion. Read live at fire time, never reactively.
  function isBusyNow(): boolean {
    return (
      (physicsState !== null && physicsState !== 'on_floor') || // drag/throw/fall/bounce/wall
      windowStore.mascotHover || // hover-jump in flight
      petStore.currentAction === 'headpat' || // headpat beat
      windowStore.settingsOpen // settings panel open
    );
  }

  // Play the voice emotion when a new final transcript arrives. Only voiceNonce is tracked;
  // everything else is read inside untrack so store changes can't re-fire this and replay a
  // stale emotion. While busy we skip the animation but the reply bubble still shows.
  $effect(() => {
    voiceNonce; // tracked dependency
    untrack(() => {
      if (!voiceNonce || !voiceEmotion || isBusyNow()) return;
      voiceEmotionSprite = voiceEmotion as CodexPetState;
      if (voiceEmotionTimer) clearTimeout(voiceEmotionTimer);
      voiceEmotionTimer = setTimeout(() => {
        voiceEmotionSprite = null;
        voiceEmotionTimer = null;
      }, VOICE_EMOTION_MS);
    });
    return () => {
      if (voiceEmotionTimer) {
        clearTimeout(voiceEmotionTimer);
        voiceEmotionTimer = null;
      }
    };
  });

  const mascotSize = $derived(Math.round(60 * settingsStore.mascotScale));

  // Growth celebrations (Phase 6): play the queue head for a beat, then shift. The
  // effect re-arms per head change, so back-to-back unlocks show sequentially.
  const CELEBRATION_MS = 3200;
  const celebration = $derived(petStore.celebrations[0] ?? null);
  $effect(() => {
    if (!celebration) return;
    const timer = setTimeout(() => petStore.shiftCelebration(), CELEBRATION_MS);
    return () => clearTimeout(timer);
  });

  // The activity bubble yields to anything that owns the same space below the pet: a
  // growth celebration, voice transcript/recording, or the expanded panel.
  const agentBubbleSuppressed = $derived(
    celebration !== null || voiceRecording || !!voiceText || windowStore.expanded
  );

  // Evolution aura: a subtle glow from the branching stage up, tinted by work style.
  // Class-only here; the drop-shadow lives in CSS so the sprite itself stays untouched.
  const auraClass = $derived.by(() => {
    const evo = petStore.evolution;
    if (evo.stageIndex < STYLE_FROM_STAGE) return '';
    return `aura stage-${evo.stageIndex} style-${evo.style ?? 'companion'}`;
  });

  // Idle action gating, read live at fire time (not reactively): the pet must be calm —
  // idle agent state, no panel/settings/hover/move, no reaction or celebration or voice
  // in flight, and physics either off or standing on the floor.
  function idleAllowedNow(): boolean {
    return (
      sourceState === 'idle' &&
      !windowStore.expanded &&
      !windowStore.settingsOpen &&
      !windowStore.mascotHover &&
      !windowStore.moveMode &&
      reactionSprite === null &&
      celebration === null &&
      !voiceRecording &&
      !voiceText &&
      !voiceReply &&
      (physicsState === null || physicsState === 'on_floor')
    );
  }

  // Self-rescheduling idle loop. Re-armed only when the stable inputs change (pet's
  // available rows, the configured interval); the per-fire gate handles the volatile
  // conditions. nextIdleDelayMs returns null for a non-positive interval → feature off.
  $effect(() => {
    const actions = idleActions;
    const intervalMin = settingsStore.petIdleIntervalMin;
    if (actions.length === 0) return;

    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;
    let revert: ReturnType<typeof setTimeout> | null = null;

    function scheduleNext() {
      const delay = nextIdleDelayMs(intervalMin, Math.random());
      if (delay === null) return; // interval disabled — stop the loop
      timer = setTimeout(() => {
        if (cancelled) return;
        if (idleAllowedNow()) {
          // Time-of-day bias: calm rows at night, lively rows midday. Hour is read at
          // fire time so a long-running app drifts with the clock.
          const part = dayPartFor(new Date().getHours());
          const action = pickIdleActionFor(actions, part, Math.random());
          if (action) {
            idleSprite = action as CodexPetState;
            revert = setTimeout(() => {
              idleSprite = null;
            }, IDLE_ACTION_MS);
          }
        }
        scheduleNext();
      }, delay);
    }
    scheduleNext();

    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
      if (revert) clearTimeout(revert);
      idleSprite = null;
    };
  });

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

  // Physics loop — torn down while the settings panel is open
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
    physicsLoop = loop;
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
      physicsLoop = null;
      clearInterval(spriteInterval);
      physicsSprite = null;
      physicsState = null;
      for (const fn of listenerCleanups) fn();
      tryInvoke('set_stroll_mode', { enabled: false });
      tryInvoke('set_throw_tracking', { enabled: false });
    };
  });

  // While the panel is expanded the pet must stand still: the stroll/physics loop moves
  // the WHOLE mini window, so a strolling pet drags the open panel around the screen.
  // Pause (not teardown) keeps position and physics state, so collapsing the panel
  // resumes in place instead of re-dropping the pet; teardown stays reserved for the
  // settings flow, which resizes the window. Pinch/throw still update state while
  // paused — tick() simply stops moving the window.
  $effect(() => {
    physicsLoop?.setPaused(windowStore.expanded);
  });

  function handleUserInput(ev: UserInputEvent) {
    // Suppress a reaction while the pet is being manipulated or otherwise busy, so it can
    // never interrupt drag/throw/hover/headpat or the settings interaction.
    if (!requestReaction(reaction, ev, { busy: isBusyNow() })) return; // coalesced or guarded → drop
    reactionSprite = reactionSpriteFor(reaction) as CodexPetState;
    if (reactionTimer) clearTimeout(reactionTimer);
    reactionTimer = setTimeout(() => {
      endReaction(reaction);
      reactionSprite = null;
      reactionTimer = null;
    }, REACTION_MS);
  }

  function isEditableTarget(target: EventTarget | null) {
    if (!(target instanceof HTMLElement)) return false;
    const tag = target.tagName.toLowerCase();
    return target.isContentEditable || tag === 'input' || tag === 'textarea' || tag === 'select';
  }

  function handleKeyboardMove(e: KeyboardEvent) {
    if (windowStore.expanded || windowStore.settingsOpen) return;
    if (physicsState !== null && physicsState !== 'on_floor') return;
    if (isEditableTarget(e.target)) return;

    const delta = keyboardMoveDelta(e);
    if (!delta) return;

    e.preventDefault();
    e.stopPropagation();
    windowStore.setMoveMode(true);
    void windowStore.moveBy(delta.dx, delta.dy);

    if (keyboardMoveTimer) clearTimeout(keyboardMoveTimer);
    keyboardMoveTimer = setTimeout(() => {
      windowStore.setMoveMode(false);
      keyboardMoveTimer = null;
    }, KEYBOARD_MOVE_MODE_MS);
  }

  $effect(() => {
    window.addEventListener('keydown', handleKeyboardMove);
    return () => {
      window.removeEventListener('keydown', handleKeyboardMove);
      if (keyboardMoveTimer) {
        clearTimeout(keyboardMoveTimer);
        keyboardMoveTimer = null;
      }
      windowStore.setMoveMode(false);
    };
  });

  // One-shot pet reaction to batched global input (Tauri "user-input" from P1-A; macOS-only).
  // Capture is OFF by default in the backend (privacy) — opt in for this component's
  // lifetime, gated on the user-facing privacy toggle (Settings → Privacy): the effect
  // re-runs on toggle, so an explicit OFF tears capture down immediately. The returned
  // ListenerStatus is surfaced by PrivacySection, not here. Safe everywhere: non-macOS
  // gets a no-op listener and start/stop are idempotent.
  $effect(() => {
    if (!settingsStore.inputTrackingEnabled) {
      tryInvoke('set_input_tracking', { active: false });
      return;
    }
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
    <div class="aura-wrap {auraClass}" class:overload={overloaded}>
      <MiniPetMascot
        {pet}
        baseState={spriteState}
        size={mascotSize}
        enableHoverJump
        externalHover={windowStore.mascotHover}
        useExternalHover
        suppressHover={windowStore.moveMode}
        reactionSprite={overlaySprite}
      />
    </div>
  {/if}

  <CelebrationBubble
    {celebration}
    placement={settingsStore.appMode === 'pet' ? 'above' : 'below'}
  />

  {#if settingsStore.appMode === 'coding'}
    <AgentBubble {activity} suppressed={agentBubbleSuppressed} />
  {/if}

  <VoiceBubble
    visible={voiceRecording || !!voiceText}
    text={voiceText}
    recording={voiceRecording}
    error={voiceError}
    petMode={settingsStore.appMode === 'pet'}
  />

  <!-- Below the pet in both modes: the mini window is only 200px tall with the mascot near
       its top, so an 'above' bubble overflows the window's top edge and gets clipped. -->
  <PetReplyBubble text={voiceReply} placement="below" />
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

  /* Evolution auras: stage drives intensity, work style drives the tint. Filters sit on
     the wrapper so the sprite's own background-position animation is unaffected. */
  .aura {
    --aura-color: rgba(255, 200, 120, 0.55);
  }

  .aura.style-commander {
    --aura-color: rgba(100, 149, 237, 0.6);
  }

  .aura.style-zen {
    --aura-color: rgba(80, 200, 140, 0.6);
  }

  .aura.style-companion {
    --aura-color: rgba(255, 143, 179, 0.6);
  }

  .aura.stage-2 {
    filter: drop-shadow(0 0 3px var(--aura-color));
  }

  .aura.stage-3 {
    filter: drop-shadow(0 0 5px var(--aura-color)) drop-shadow(0 0 10px var(--aura-color));
  }

  .aura.stage-4 {
    animation: legendPulse 3s ease-in-out infinite;
  }

  @keyframes legendPulse {
    0%,
    100% {
      filter: drop-shadow(0 0 4px var(--aura-color)) drop-shadow(0 0 9px rgba(255, 215, 80, 0.5));
    }
    50% {
      filter: drop-shadow(0 0 7px var(--aura-color)) drop-shadow(0 0 14px rgba(255, 215, 80, 0.75));
    }
  }

  /* Overload: 3+ agents busy in parallel. A small nervous tremble reads as "stressed,
     juggling a lot" — transform-based so it composes with any evolution-aura filter
     instead of fighting it in the cascade. */
  .aura-wrap.overload {
    animation: overloadShake 0.45s ease-in-out infinite;
  }

  @keyframes overloadShake {
    0%,
    100% {
      transform: translate(0, 0) rotate(0deg);
    }
    25% {
      transform: translate(-0.7px, 0.4px) rotate(-1.1deg);
    }
    50% {
      transform: translate(0.7px, -0.3px) rotate(0.9deg);
    }
    75% {
      transform: translate(-0.4px, 0.5px) rotate(-0.6deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .aura-wrap.overload {
      animation: none;
    }
  }
</style>
