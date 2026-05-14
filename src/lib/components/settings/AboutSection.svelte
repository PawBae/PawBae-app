<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { _, locale } from 'svelte-i18n';
  import { settingsStore } from '../../stores/settings.svelte';

  let { open }: { open: boolean } = $props();

  type UpdateProgressPayload = {
    stage: string;
    progress?: number | null;
    downloadedBytes?: number;
    totalBytes?: number | null;
    message?: string;
  };

  let updateInfo = $state<{ current: string; latest: string; hasUpdate: boolean; url: string } | null>(null);
  let updateChecking = $state(false);
  let updateCheckResult = $state<'success' | 'error' | null>(null);
  let updateCheckMsg = $state('');
  let updating = $state(false);
  let updateProgress = $state<number | null>(null);
  let updateProgressMsg = $state('');
  let updateRunResult = $state<'success' | 'error' | null>(null);
  let updateRunMsg = $state('');

  function resolveProgressText(stage?: string, fallbackMessage?: string): string {
    if (stage) {
      const key = `updateModal.progress.${stage}`;
      const localized = $_(key);
      if (localized !== key) return localized;
    }
    return fallbackMessage || '';
  }

  async function checkForUpdate(showFeedback = false) {
    updateChecking = true;
    if (showFeedback) {
      updateCheckResult = null;
      updateCheckMsg = '';
    }
    try {
      const info = await invoke('check_for_update', { lang: settingsStore.language }) as {
        current: string; latest: string; hasUpdate: boolean; url: string;
      };
      updateInfo = info;
      if (showFeedback) {
        updateCheckResult = 'success';
        updateCheckMsg = info.hasUpdate
          ? `${$_('settings.newVersionFound')} v${info.latest}`
          : $_('settings.alreadyLatest');
      }
    } catch (e: any) {
      if (showFeedback) {
        updateCheckResult = 'error';
        updateCheckMsg = `${$_('settings.checkFailed')}${String(e)}`;
      }
    } finally {
      updateChecking = false;
    }
  }

  async function runUpdate() {
    if (!updateInfo?.url) return;
    updating = true;
    updateProgress = 0;
    updateProgressMsg = resolveProgressText('preparing', $_('settings.preparingDownload'));
    updateRunResult = null;
    updateRunMsg = '';
    try {
      await invoke('run_update', { dmgUrl: updateInfo.url });
      updateRunResult = 'success';
      updateRunMsg = $_('settings.downloadComplete');
      setTimeout(() => {
        invoke('exit_app').catch((e: any) => {
          updating = false;
          updateRunResult = 'error';
          updateRunMsg = `${$_('settings.exitFailed')}${String(e)}`;
        });
      }, 600);
    } catch (e: any) {
      updateProgress = null;
      updateProgressMsg = '';
      updateRunResult = 'error';
      updateRunMsg = `${$_('settings.updateFailed')}${String(e)}`;
      updating = false;
    }
  }

  let prevOpen = false;

  $effect(() => {
    const isOpen = open;
    if (isOpen && !prevOpen) {
      checkForUpdate();
    }
    prevOpen = isOpen;
  });

  $effect(() => {
    if (!open) return;

    let disposed = false;
    let unlisten: (() => void) | null = null;
    listen<UpdateProgressPayload>('update-progress', (event) => {
      const p = event.payload;
      updateProgress = typeof p.progress === 'number' ? p.progress : null;
      updateProgressMsg = resolveProgressText(p.stage, p.message);
    }).then((u) => {
      if (disposed) u();
      else unlisten = u;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  async function changeLanguage(lng: string) {
    locale.set(lng);
    await settingsStore.setLanguage(lng);
    invoke('update_tray_language', { lang: lng }).catch(() => {});
  }
</script>

<!-- About -->
<section class="section">
  <h2>{$_('settings.about')}</h2>
  <div class="card">
    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-label">{$_('settings.currentVersion')}</span>
        <span class="setting-desc">
          {updateInfo ? `v${updateInfo.current}` : '...'}
          {#if updateInfo && !updateInfo.hasUpdate} ({$_('settings.latest')}){/if}
          {#if updateInfo?.hasUpdate}
            <span class="version-new">v{updateInfo.latest} {$_('settings.available')}</span>
          {/if}
        </span>
        {#if updateCheckResult === 'success' && updateCheckMsg}
          <span class="msg-success">{updateCheckMsg}</span>
        {/if}
        {#if updateCheckResult === 'error' && updateCheckMsg}
          <span class="msg-error">{updateCheckMsg}</span>
        {/if}
        {#if updateRunResult === 'success' && updateRunMsg}
          <span class="msg-success">{updateRunMsg}</span>
        {/if}
        {#if updateRunResult === 'error' && updateRunMsg}
          <span class="msg-error">{updateRunMsg}</span>
        {/if}
        {#if updating || updateProgressMsg}
          <div class="progress-info">
            <span class="progress-text">
              {updateProgressMsg}
              {#if typeof updateProgress === 'number' && updateProgress < 100} · {updateProgress}%{/if}
            </span>
            {#if typeof updateProgress === 'number'}
              <div class="progress-bar">
                <div class="progress-fill" style="width: {Math.max(updateProgress, 2)}%"></div>
              </div>
            {/if}
          </div>
        {/if}
      </div>
      <div class="update-buttons">
        {#if updateInfo?.hasUpdate}
          <button class="btn-primary" onclick={runUpdate} disabled={updating}>
            {updating ? $_('settings.updating') : $_('settings.updateNow')}
          </button>
        {/if}
        <button class="btn-small" onclick={() => checkForUpdate(true)} disabled={updateChecking}>
          {updateChecking ? $_('settings.checking') : $_('settings.checkUpdate')}
        </button>
      </div>
    </div>
  </div>
</section>

<!-- Language -->
<section class="section">
  <h2>{$_('settings.language')}</h2>
  <div class="card">
    <div class="setting-row">
      <div class="setting-info">
        <span class="setting-label">{$_('settings.language')}</span>
        <span class="setting-desc">{$_('settings.languageDesc')}</span>
      </div>
      <div class="segmented lang-segmented">
        {#each ['zh', 'en'] as lng}
          <button class="seg-btn" class:active={settingsStore.language === lng}
            onclick={() => changeLanguage(lng)}
          >
            {$_(`settings.lang${lng.charAt(0).toUpperCase() + lng.slice(1)}`)}
          </button>
        {/each}
      </div>
    </div>
  </div>
</section>

<!-- Exit -->
<section class="section exit-section">
  <button class="exit-btn" onclick={() => invoke('exit_app').catch(() => {})}>
    {$_('settings.exitApp')}
  </button>
</section>

<style>
  .update-buttons {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .version-new {
    margin-left: 8px;
    color: #34d399;
  }

  .msg-success {
    font-size: 11px;
    color: #34d399;
  }

  .msg-error {
    font-size: 11px;
    color: #f87171;
    word-break: break-all;
  }

  .progress-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 4px;
  }

  .progress-text {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.5);
  }

  .progress-bar {
    width: 100%;
    height: 6px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 3px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: #3b82f6;
    transition: width 0.2s;
    border-radius: 3px;
  }

  .lang-segmented {
    flex-wrap: wrap;
  }

  .exit-section {
    padding-top: 8px;
  }

  .exit-btn {
    width: 100%;
    padding: 12px;
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.2);
    border-radius: 12px;
    color: #f87171;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s;
  }
  .exit-btn:hover {
    background: rgba(239, 68, 68, 0.2);
  }
</style>
