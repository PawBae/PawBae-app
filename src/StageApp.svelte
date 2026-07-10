<script lang="ts">
  // OBS 直播舞台 (spec: docs/superpowers/specs/2026-07-09-obs-stage-design.md).
  // A dumb mirror of the main window's mascot on a chroma-key backdrop: everything
  // rendered here rides the stage-state snapshot — no petStore, no persistence, no
  // interactivity. The only store it touches is the read-only skins load.
  import { emitTo } from '@tauri-apps/api/event';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { _, locale } from 'svelte-i18n';
  import AgentBubble from './lib/components/AgentBubble.svelte';
  import CelebrationBubble from './lib/components/CelebrationBubble.svelte';
  import MiniPetMascot from './lib/components/MiniPetMascot.svelte';
  import { skinsStore } from './lib/stores/skins.svelte';
  import type { CodexPetState } from './lib/utils/codex-pet';
  import { STAGE_BG_COLORS, type StageSnapshot } from './lib/utils/stage-bridge';

  let snap = $state<StageSnapshot | null>(null);
  let vw = $state(window.innerWidth);
  let vh = $state(window.innerHeight);

  $effect(() => {
    void skinsStore.ensureLoaded();
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    void (async () => {
      const stop = await getCurrentWebviewWindow().listen<StageSnapshot>('stage-state', (e) => {
        snap = e.payload;
      });
      if (cancelled) {
        stop();
        return;
      }
      unlisten = stop;
      // Handshake: this webview mounts after the main window's last emit, so ask
      // for a replay — but only once the listener registration above has landed,
      // or the reply races the subscription and the stage stays blank until the
      // next snapshot change (Codex review).
      void emitTo('main', 'stage-ready');
    })();
    const onResize = () => {
      vw = window.innerWidth;
      vh = window.innerHeight;
    };
    window.addEventListener('resize', onResize);
    return () => {
      cancelled = true;
      unlisten?.();
      window.removeEventListener('resize', onResize);
    };
  });

  // Each webview boots svelte-i18n from the navigator; the bubbles render their
  // text HERE, so follow the main window's in-app language choice instead.
  $effect(() => {
    if (snap?.locale) locale.set(snap.locale);
  });

  const pet = $derived.by(() => {
    if (!snap) return null;
    skinsStore.revision; // re-resolve when a re-imported skin upgrades in place
    return skinsStore.resolve(snap.petId);
  });
  // Half the short edge: leaves headroom for the celebration bubble above and
  // the agent bubble below at the default 16:9 window.
  const size = $derived(Math.round(Math.min(vw, vh) * 0.5));
  const bg = $derived(STAGE_BG_COLORS[snap?.bg ?? 'green']);
</script>

<div class="stage" data-tauri-drag-region style="background: {bg};">
  {#if snap?.away}
    <div class="away">
      <span style="font-size: {Math.round(size * 0.6)}px;">⛺</span>
      <span class="note">{$_('adventure.awayNote')}</span>
    </div>
  {:else if pet && snap}
    <div class="slot" style="width: {size}px; height: {size}px;">
      <MiniPetMascot
        {pet}
        baseState={snap.spriteState as CodexPetState}
        {size}
        enableHoverJump={false}
        suppressHover
        reactionSprite={(snap.overlaySprite as CodexPetState | null) ?? null}
      />
      <CelebrationBubble celebration={snap.celebration} placement="above" />
      <!-- Same yield rule as the desktop: the celebration owns the space. -->
      <AgentBubble activity={snap.activity} suppressed={snap.celebration !== null} />
    </div>
  {/if}
</div>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    padding: 0;
    overflow: hidden;
    width: 100%;
    height: 100%;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    color: #fff;
  }

  :global(*) {
    box-sizing: border-box;
  }

  .stage {
    width: 100vw;
    height: 100vh;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: grab;
    user-select: none;
    -webkit-user-select: none;
  }

  /* Children ignore the pointer so every press lands on the drag region — the
     stage is scenery, not a pet you can play with (that one is on the desktop). */
  .slot {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }

  .away {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    pointer-events: none;
  }

  .note {
    font-size: 13px;
    color: rgba(0, 0, 0, 0.7);
    background: rgba(255, 255, 255, 0.85);
    border-radius: 8px;
    padding: 2px 10px;
  }
</style>
