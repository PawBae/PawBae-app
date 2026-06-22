<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { settingsStore } from '../../stores/settings.svelte';
  import { tryInvoke } from '../../utils/invoke';

  let { open, isWindows = false }: { open: boolean; isWindows?: boolean } = $props();

  type ListenerStatus = { keyboard: boolean; mouse: boolean; reason?: string };

  let status = $state<ListenerStatus | null>(null);

  let prevOpen = false;
  $effect(() => {
    const isOpen = open;
    if (isOpen && !prevOpen && !isWindows && settingsStore.inputTrackingEnabled) {
      tryInvoke<ListenerStatus>('get_input_tracking_status').then((s) => {
        status = s ?? null;
      });
    }
    prevOpen = isOpen;
  });

  async function toggle(v: boolean) {
    await settingsStore.setInputTrackingEnabled(v);
    // MascotView's lifecycle effect reacts to the setting as well; calling here too is
    // idempotent and hands us the fresh ListenerStatus to render (e.g. keyboard off
    // because macOS Accessibility access is denied).
    const s = await tryInvoke<ListenerStatus>('set_input_tracking', { active: v });
    status = s ?? null;
  }

  const showAccessibilityHint = $derived(
    settingsStore.inputTrackingEnabled && status !== null && !status.keyboard && status.mouse
  );
</script>

<section class="section">
  <h2>{$_('settings.privacy')}</h2>
  <div class="card">
    {#if !isWindows}
    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-label">{$_('settings.inputReactions')}</span>
        <span class="setting-desc">{$_('settings.inputReactionsDesc')}</span>
        {#if showAccessibilityHint}
          <span class="hint-warn">{$_('settings.inputKeyboardOff')}</span>
        {/if}
      </div>
      <button class="toggle" class:on={settingsStore.inputTrackingEnabled}
        onclick={() => toggle(!settingsStore.inputTrackingEnabled)}
        role="switch" aria-checked={settingsStore.inputTrackingEnabled}>
        <span class="toggle-thumb"></span>
      </button>
    </div>
    {/if}

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-label">{$_('settings.voiceInteraction')}</span>
        <span class="setting-desc">{$_('settings.voiceInteractionDesc')}</span>
        <span class="setting-desc">{$_('settings.voiceShortcutHint')}</span>
      </div>
      <button class="toggle" class:on={settingsStore.voiceEnabled}
        onclick={() => settingsStore.setVoiceEnabled(!settingsStore.voiceEnabled)}
        role="switch" aria-checked={settingsStore.voiceEnabled}
        aria-label={$_('settings.voiceInteraction')}>
        <span class="toggle-thumb"></span>
      </button>
    </div>

    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-label">{$_('settings.musicReaction')}</span>
        <span class="setting-desc">{$_('settings.musicReactionDesc')}</span>
      </div>
      <button class="toggle" class:on={settingsStore.musicReactionEnabled}
        onclick={() => settingsStore.setMusicReactionEnabled(!settingsStore.musicReactionEnabled)}
        role="switch" aria-checked={settingsStore.musicReactionEnabled}
        aria-label={$_('settings.musicReaction')}>
        <span class="toggle-thumb"></span>
      </button>
    </div>
  </div>
</section>

<style>
  .hint-warn {
    font-size: 11px;
    color: #fbbf24;
    margin-top: 2px;
  }
</style>
