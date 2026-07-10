<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { settingsStore } from '../../stores/settings.svelte';

  let { isWindows }: { isWindows: boolean } = $props();
</script>

<section class="section">
  <h2>{$_('settings.sound')}</h2>
  <div class="card">
    <div class="setting-row border-bottom">
      <div class="setting-info">
        <span class="setting-label">{$_('settings.completionSound')}</span>
        <span class="setting-desc">{$_('settings.completionSoundDesc')}</span>
      </div>
      <div class="segmented">
        {#each ['default', 'manbo'] as s}
          <button class="seg-btn" class:active={settingsStore.notifySound === s}
            onclick={() => settingsStore.setNotifySound(s as 'default' | 'manbo')}
          >
            {s === 'default' ? $_('settings.defaultSound') : $_('settings.manboSound')}
          </button>
        {/each}
      </div>
    </div>
    <div class="setting-row border-bottom">
      <div class="setting-info">
        <span class="setting-label">{$_('settings.ccSound')}</span>
        <span class="setting-desc">{$_('settings.ccSoundDesc')}</span>
      </div>
      <button class="toggle" class:on={settingsStore.soundEnabled}
        onclick={() => settingsStore.setSoundEnabled(!settingsStore.soundEnabled)}
        role="switch" aria-checked={settingsStore.soundEnabled}>
        <span class="toggle-thumb"></span>
      </button>
    </div>
    {#if !isWindows}
      <div class="setting-row border-bottom">
        <div class="setting-info">
          <span class="setting-label">{$_('settings.codexSound')}</span>
          <span class="setting-desc">{$_('settings.codexSoundDesc')}</span>
        </div>
        <button class="toggle" class:on={settingsStore.codexSoundEnabled}
          onclick={() => settingsStore.setCodexSoundEnabled(!settingsStore.codexSoundEnabled)}
          role="switch" aria-checked={settingsStore.codexSoundEnabled}>
          <span class="toggle-thumb"></span>
        </button>
      </div>
      <div class="setting-row border-bottom">
        <div class="setting-info">
          <span class="setting-label">{$_('settings.cursorSound')}</span>
          <span class="setting-desc">{$_('settings.cursorSoundDesc')}</span>
        </div>
        <button class="toggle" class:on={settingsStore.cursorSoundEnabled}
          onclick={() => settingsStore.setCursorSoundEnabled(!settingsStore.cursorSoundEnabled)}
          role="switch" aria-checked={settingsStore.cursorSoundEnabled}>
          <span class="toggle-thumb"></span>
        </button>
      </div>
    {/if}
    <div class="setting-row border-bottom">
      <div class="setting-info">
        <span class="setting-label">{$_('settings.waitingSound')}</span>
        <span class="setting-desc">{$_('settings.waitingSoundDesc')}</span>
      </div>
      <button class="toggle" class:on={settingsStore.waitingSound}
        onclick={() => settingsStore.setWaitingSound(!settingsStore.waitingSound)}
        role="switch" aria-checked={settingsStore.waitingSound}>
        <span class="toggle-thumb"></span>
      </button>
    </div>
    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-label">{$_('settings.autoCloseCompletion')}</span>
        <span class="setting-desc">{$_('settings.autoCloseCompletionDesc')}</span>
      </div>
      <button class="toggle" class:on={settingsStore.autoCloseCompletion}
        onclick={() => settingsStore.setAutoCloseCompletion(!settingsStore.autoCloseCompletion)}
        role="switch" aria-checked={settingsStore.autoCloseCompletion}>
        <span class="toggle-thumb"></span>
      </button>
    </div>
  </div>
</section>
