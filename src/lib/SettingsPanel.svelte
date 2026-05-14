<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { _, locale } from 'svelte-i18n';
  import { settingsStore } from './stores/settings.svelte';
  import { agentStore } from './stores/agents.svelte';
  import type { OcConnection } from './types';

  type UpdateProgressPayload = {
    stage: string;
    progress?: number | null;
    downloadedBytes?: number;
    totalBytes?: number | null;
    message?: string;
  };

  let {
    open = false,
    onClose,
  }: {
    open?: boolean;
    onClose: () => void;
  } = $props();

  const isWindows = typeof navigator !== 'undefined' && navigator.userAgent.includes('Windows');

  let connections = $state<OcConnection[]>([]);
  let enableClaudeCode = $state(true);
  let hookStatus = $state('');
  let enableCodex = $state(!isWindows);
  let codexHookStatus = $state('');
  let enableCursor = $state(!isWindows);
  let cursorHookStatus = $state('');

  let updateInfo = $state<{ current: string; latest: string; hasUpdate: boolean; url: string } | null>(null);
  let updateChecking = $state(false);
  let updateCheckResult = $state<'success' | 'error' | null>(null);
  let updateCheckMsg = $state('');
  let updating = $state(false);
  let updateProgress = $state<number | null>(null);
  let updateProgressMsg = $state('');
  let updateRunResult = $state<'success' | 'error' | null>(null);
  let updateRunMsg = $state('');

  // Connection test state per connection index
  let testingIdx = $state<number | null>(null);
  let testResult = $state<{ idx: number; type: 'success' | 'error'; msg: string } | null>(null);

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
      connections = [...settingsStore.ocConnections];
      enableClaudeCode = settingsStore.enableClaudeCode;
      enableCodex = isWindows ? false : settingsStore.enableCodex;
      enableCursor = isWindows ? false : settingsStore.enableCursor;
      checkForUpdate();
    }
    prevOpen = isOpen;
  });

  $effect(() => {
    if (!open) return;

    const unlisten = listen<UpdateProgressPayload>('update-progress', (event) => {
      const p = event.payload;
      updateProgress = typeof p.progress === 'number' ? p.progress : null;
      updateProgressMsg = resolveProgressText(p.stage, p.message);
    });

    return () => { unlisten.then((fn) => fn()); };
  });

  async function toggleClaudeCode(val: boolean) {
    enableClaudeCode = val;
    await settingsStore.setEnableClaudeCode(val);
    if (val) {
      try {
        await invoke('install_claude_hooks');
        hookStatus = $_('settings.hookInstalled');
      } catch (e: any) {
        hookStatus = `${$_('settings.hookFailed')} ${String(e)}`;
      }
    }
  }

  async function toggleCodex(val: boolean) {
    enableCodex = val;
    await settingsStore.setEnableCodex(val);
    if (val) {
      try {
        await invoke('install_claude_hooks');
        codexHookStatus = $_('settings.hookInstalled');
      } catch (e: any) {
        codexHookStatus = `${$_('settings.hookFailed')} ${String(e)}`;
      }
    }
  }

  async function toggleCursor(val: boolean) {
    enableCursor = val;
    await settingsStore.setEnableCursor(val);
    if (val) {
      try {
        await invoke('install_cursor_hooks');
        cursorHookStatus = $_('settings.hookInstalled');
      } catch (e: any) {
        cursorHookStatus = `${$_('settings.hookFailed')} ${String(e)}`;
      }
    }
  }

  function syncConnections(conns: OcConnection[]) {
    settingsStore.setOcConnections(conns);
    agentStore.setConnections(conns);
  }

  function updateConnection(idx: number, conn: OcConnection) {
    connections[idx] = conn;
    syncConnections([...connections]);
  }

  function deleteConnection(idx: number) {
    const conn = connections[idx];
    if (conn.type === 'remote' && conn.host && conn.user) {
      invoke('close_ssh', { sshHost: conn.host, sshUser: conn.user }).catch(() => {});
    }
    connections = connections.filter((_, i) => i !== idx);
    syncConnections([...connections]);
  }

  function addConnection() {
    const hasLocal = connections.some(c => c.type === 'local');
    connections = [...connections, { id: crypto.randomUUID(), type: hasLocal ? 'remote' : 'local' }];
    syncConnections([...connections]);
  }

  async function testConnection(idx: number) {
    const conn = connections[idx];
    testingIdx = idx;
    testResult = null;
    try {
      if (conn.type === 'remote') {
        await invoke('reset_ssh', { sshHost: conn.host, sshUser: conn.user }).catch(() => {});
        const result: any = await invoke('get_agents', { mode: 'remote', sshHost: conn.host, sshUser: conn.user });
        let keyInfo = '';
        try {
          const key = await invoke('get_ssh_key_info', { sshHost: conn.host, sshUser: conn.user }) as string | null;
          if (key) keyInfo = ` · ${$_('settings.key')} ${key}`;
        } catch {}
        testResult = { idx, type: 'success', msg: `${result.length} ${$_('settings.agents')}${keyInfo}` };
      } else {
        const store = await settingsStore.getStore();
        const agentId = ((await store.get('tracked_agent')) as string) || 'main';
        const result: any = await invoke('get_status', { gatewayUrl: 'http://localhost:4446', token: '', agentId });
        testResult = { idx, type: 'success', msg: `${result.sessions.length} ${$_('settings.sessions')}` };
      }
      setTimeout(() => { if (testResult?.idx === idx) testResult = null; }, 3000);
    } catch (e: any) {
      testResult = { idx, type: 'error', msg: String(e) };
    }
    testingIdx = null;
  }

  async function changeLanguage(lng: string) {
    locale.set(lng);
    await settingsStore.setLanguage(lng);
    invoke('update_tray_language', { lang: lng }).catch(() => {});
  }

  const isPetMode = $derived(settingsStore.appMode === 'pet');
</script>

{#if open}
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="settings-overlay" onclick={onClose}>
  <div class="settings-panel" onclick={(e) => e.stopPropagation()}>
    <div class="settings-header">
      <h1>{$_('mini.settings')}</h1>
      <button class="close-btn" onclick={onClose}>&times;</button>
    </div>

    <div class="settings-scroll">
      <div class="settings-content">

        <!-- App Mode Switch -->
        {#if settingsStore.appMode}
          <section class="section">
            <h2>{$_('settings.appMode')}</h2>
            <div class="card">
              <div class="mode-row">
                {#each [
                  { mode: 'coding' as const, label: $_('settings.codingMode'), icon: '\u{1F4BB}', desc: $_('settings.codingModeDesc') },
                  { mode: 'pet' as const, label: $_('settings.petMode'), icon: '\u{1F43E}', desc: $_('settings.petModeDesc') },
                ] as { mode, label, icon, desc }}
                  <button
                    class="mode-btn"
                    class:active={settingsStore.appMode === mode}
                    onclick={() => settingsStore.setAppMode(mode)}
                  >
                    <span class="mode-icon">{icon}</span>
                    <div class="mode-text">
                      <div class="mode-label">{label}</div>
                      <div class="mode-desc">{desc}</div>
                    </div>
                  </button>
                {/each}
              </div>
            </div>
          </section>
        {/if}

        <!-- Pet mode: mascot size -->
        {#if isPetMode && !isWindows}
          <section class="section">
            <h2>{$_('settings.display')}</h2>
            <div class="card">
              <div class="setting-row">
                <div class="setting-info">
                  <span class="setting-label">{$_('settings.largeMascotScale')}</span>
                  <span class="setting-desc">{$_('settings.largeMascotScaleDesc')}</span>
                </div>
                <span class="setting-value">{settingsStore.largeMascotScale.toFixed(1)}x</span>
              </div>
              <input type="range" min={4} max={6} step={0.1}
                value={settingsStore.largeMascotScale}
                oninput={(e) => settingsStore.setLargeMascotScale(Number(e.currentTarget.value))}
              />
            </div>
          </section>
        {/if}

        <!-- Pet mode: character SFX -->
        {#if isPetMode}
          <section class="section">
            <h2>{$_('settings.sound')}</h2>
            <div class="card">
              <div class="setting-row">
                <div class="setting-info">
                  <span class="setting-label">{$_('settings.petSfx')}</span>
                  <span class="setting-desc">{$_('settings.petSfxDesc')}</span>
                </div>
                <button class="toggle" class:on={settingsStore.petSfxEnabled}
                  onclick={() => settingsStore.setPetSfxEnabled(!settingsStore.petSfxEnabled)}
                  role="switch" aria-checked={settingsStore.petSfxEnabled}>
                  <span class="toggle-thumb"></span>
                </button>
              </div>
            </div>
          </section>
        {/if}

        <!-- Pet mode: idle interval -->
        {#if isPetMode}
          <section class="section">
            <h2>{$_('settings.petBehavior')}</h2>
            <div class="card">
              <div class="setting-row">
                <div class="setting-info">
                  <span class="setting-label">{$_('settings.petIdleInterval')}</span>
                  <span class="setting-desc">{$_('settings.petIdleIntervalDesc')}</span>
                </div>
                <span class="setting-value">
                  {settingsStore.petIdleIntervalMin.toFixed(1)} {$_('settings.minutesShort')}
                </span>
              </div>
              <input type="range" min={0.5} max={30} step={0.5}
                value={settingsStore.petIdleIntervalMin}
                oninput={(e) => settingsStore.setPetIdleIntervalMin(Number(e.currentTarget.value))}
              />
            </div>
          </section>
        {/if}

        {#if !isPetMode}
          <!-- OpenClaw Connections -->
          <section class="section">
            <div class="section-header">
              <h2>{$_('settings.ocConnections')}</h2>
              <button class="btn-small" onclick={addConnection}>+ {$_('common.add')}</button>
            </div>
            <div class="card">
              {#if connections.length === 0}
                <div class="empty-msg">{$_('settings.noConnections')}</div>
              {:else}
                {#each connections as conn, idx (conn.id)}
                  <div class="conn-row" class:border-top={idx > 0}>
                    <div class="conn-header">
                      <div class="conn-type-group">
                        <div class="segmented">
                          {#each ['local', 'remote'] as typ}
                            {@const disabled = typ === 'local' && connections.some((c, i) => i !== idx && c.type === 'local') && conn.type !== 'local'}
                            <button
                              class="seg-btn" class:active={conn.type === typ} class:disabled
                              onclick={() => !disabled && updateConnection(idx, { ...conn, type: typ as 'local' | 'remote' })}
                            >
                              {typ === 'local' ? $_('settings.local') : $_('settings.remote')}
                            </button>
                          {/each}
                        </div>
                        <span class="conn-summary">
                          {conn.type === 'local' ? '~/.openclaw' : conn.host ? `${conn.user || 'root'}@${conn.host}` : $_('settings.notConfigured')}
                        </span>
                      </div>
                      <button class="delete-btn" onclick={() => deleteConnection(idx)}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                        </svg>
                      </button>
                    </div>

                    {#if conn.type === 'remote'}
                      <div class="remote-fields">
                        <input type="text" value={conn.user || ''} placeholder={$_('settings.username')}
                          autocapitalize="off" autocorrect="off" spellcheck="false"
                          oninput={(e) => updateConnection(idx, { ...conn, user: e.currentTarget.value })}
                        />
                        <span class="at-sign">@</span>
                        <input type="text" value={conn.host || ''} placeholder={$_('settings.serverAddress')}
                          class="flex-1"
                          oninput={(e) => updateConnection(idx, { ...conn, host: e.currentTarget.value })}
                        />
                      </div>
                    {/if}

                    <div class="conn-actions">
                      <button class="btn-small"
                        disabled={testingIdx === idx || (conn.type === 'remote' && (!conn.host || !conn.user))}
                        onclick={() => testConnection(idx)}
                      >
                        {#if testingIdx === idx}
                          <span class="spinner"></span>
                        {/if}
                        {$_('common.test')}
                      </button>
                      {#if testResult?.idx === idx && testResult.type === 'success'}
                        <span class="test-success">{$_('common.success')} {testResult.msg ? `· ${testResult.msg}` : ''}</span>
                      {/if}
                      {#if testResult?.idx === idx && testResult.type === 'error'}
                        <div class="test-error">
                          <span>{$_('common.failed')}</span>
                          <pre>{testResult.msg}</pre>
                        </div>
                      {/if}
                    </div>
                  </div>
                {/each}
              {/if}
            </div>
          </section>

          <!-- Claude Code -->
          <section class="section">
            <h2>Claude Code</h2>
            <div class="card">
              <div class="setting-row">
                <div class="setting-info">
                  <span class="setting-label">{$_('settings.enableClaudeCode')}</span>
                  <span class="setting-desc">{$_('settings.enableClaudeCodeDesc')}</span>
                  {#if hookStatus}<span class="hook-status">{hookStatus}</span>{/if}
                </div>
                <button class="toggle" class:on={enableClaudeCode}
                  onclick={() => toggleClaudeCode(!enableClaudeCode)}
                  role="switch" aria-checked={enableClaudeCode}>
                  <span class="toggle-thumb"></span>
                </button>
              </div>
            </div>
          </section>

          {#if !isWindows}
            <!-- Codex -->
            <section class="section">
              <h2>{$_('settings.codex')}</h2>
              <div class="card">
                <div class="setting-row">
                  <div class="setting-info">
                    <span class="setting-label">{$_('settings.enableCodex')}</span>
                    <span class="setting-desc">{$_('settings.enableCodexDesc')}</span>
                    {#if codexHookStatus}<span class="hook-status">{codexHookStatus}</span>{/if}
                  </div>
                  <button class="toggle" class:on={enableCodex}
                    onclick={() => toggleCodex(!enableCodex)}
                    role="switch" aria-checked={enableCodex}>
                    <span class="toggle-thumb"></span>
                  </button>
                </div>
              </div>
            </section>

            <!-- Cursor -->
            <section class="section">
              <h2>Cursor</h2>
              <div class="card">
                <div class="setting-row">
                  <div class="setting-info">
                    <span class="setting-label">{$_('settings.enableCursor')}</span>
                    <span class="setting-desc">{$_('settings.enableCursorDesc')}</span>
                    {#if cursorHookStatus}<span class="hook-status">{cursorHookStatus}</span>{/if}
                  </div>
                  <button class="toggle" class:on={enableCursor}
                    onclick={() => toggleCursor(!enableCursor)}
                    role="switch" aria-checked={enableCursor}>
                    <span class="toggle-thumb"></span>
                  </button>
                </div>
              </div>
            </section>
          {/if}

          <!-- Display Settings -->
          <section class="section">
            <h2>{$_('settings.display')}</h2>
            <div class="card">
              <div class="setting-row border-bottom">
                <div class="setting-info">
                  <span class="setting-label">{$_('settings.autoExpandOnTask')}</span>
                  <span class="setting-desc">{$_('settings.autoExpandOnTaskDesc')}</span>
                </div>
                <button class="toggle" class:on={settingsStore.autoExpandOnTask}
                  onclick={() => settingsStore.setAutoExpandOnTask(!settingsStore.autoExpandOnTask)}
                  role="switch" aria-checked={settingsStore.autoExpandOnTask}>
                  <span class="toggle-thumb"></span>
                </button>
              </div>
              <div class="setting-row border-bottom slider-row">
                <div class="setting-info">
                  <span class="setting-label">{$_('settings.panelMaxHeight')}</span>
                  <span class="setting-desc">{$_('settings.panelMaxHeightDesc')}</span>
                </div>
                <span class="setting-value">{settingsStore.panelMaxHeight}px</span>
              </div>
              <div class="slider-wrap border-bottom">
                <input type="range" min={200} max={500} step={10}
                  value={settingsStore.panelMaxHeight}
                  oninput={(e) => settingsStore.setPanelMaxHeight(Number(e.currentTarget.value))}
                />
              </div>
              <div class="setting-row slider-row">
                <div class="setting-info">
                  <span class="setting-label">{$_('settings.hoverDelay')}</span>
                  <span class="setting-desc">{$_('settings.hoverDelayDesc')}</span>
                </div>
                <span class="setting-value">{settingsStore.hoverDelay.toFixed(1)}s</span>
              </div>
              <div class="slider-wrap border-bottom">
                <input type="range" min={0} max={2} step={0.1}
                  value={settingsStore.hoverDelay}
                  oninput={(e) => settingsStore.setHoverDelay(Number(e.currentTarget.value))}
                />
              </div>
              {#if !isWindows}
                <div class="setting-row slider-row">
                  <div class="setting-info">
                    <span class="setting-label">{$_('settings.largeMascotScale')}</span>
                    <span class="setting-desc">{$_('settings.largeMascotScaleDesc')}</span>
                  </div>
                  <span class="setting-value">{settingsStore.largeMascotScale.toFixed(1)}x</span>
                </div>
                <div class="slider-wrap">
                  <input type="range" min={1} max={6} step={0.1}
                    value={settingsStore.largeMascotScale}
                    oninput={(e) => settingsStore.setLargeMascotScale(Number(e.currentTarget.value))}
                  />
                </div>
              {/if}
            </div>
          </section>

          <!-- Sound Settings -->
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
        {/if}

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

      </div>
    </div>
  </div>
</div>
{/if}

<style>
  .settings-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    backdrop-filter: blur(4px);
    z-index: 1000;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    animation: fade-in 0.2s ease-out;
  }

  .settings-panel {
    width: 100%;
    height: 100%;
    background: rgba(26, 26, 32, 0.98);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: slide-up 0.25s ease-out;
  }

  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes slide-up {
    from { opacity: 0; transform: translateY(20px) scale(0.97); filter: blur(8px); }
    to { opacity: 1; transform: translateY(0) scale(1); filter: blur(0); }
  }

  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 20px 24px 12px;
    flex-shrink: 0;
  }

  .settings-header h1 {
    font-size: 20px;
    font-weight: 600;
    margin: 0;
    color: #fff;
  }

  .close-btn {
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.4);
    font-size: 24px;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 8px;
    transition: all 0.15s;
  }
  .close-btn:hover {
    color: rgba(255, 255, 255, 0.8);
    background: rgba(255, 255, 255, 0.08);
  }

  .settings-scroll {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .settings-content {
    max-width: 580px;
    margin: 0 auto;
    padding: 10px 24px 80px;
  }

  .section {
    margin-bottom: 28px;
  }

  .section h2 {
    font-size: 15px;
    font-weight: 500;
    color: #fff;
    margin: 0 0 10px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }

  .section-header h2 {
    margin: 0;
  }

  .card {
    background: #0f0f0f;
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 16px;
    overflow: hidden;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px;
    gap: 12px;
  }

  .border-bottom {
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .border-top {
    border-top: 1px solid rgba(255, 255, 255, 0.05);
  }

  .setting-info {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    flex: 1;
  }

  .setting-label {
    font-size: 13px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.9);
  }

  .setting-desc {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.4);
  }

  .setting-value {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.6);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .slider-row {
    padding-bottom: 4px;
  }

  .slider-wrap {
    padding: 0 16px 14px;
  }

  input[type="range"] {
    width: 100%;
    height: 4px;
    accent-color: rgba(255, 255, 255, 0.6);
    cursor: pointer;
  }

  /* Toggle switch */
  .toggle {
    position: relative;
    display: inline-flex;
    width: 44px;
    height: 24px;
    flex-shrink: 0;
    cursor: pointer;
    border-radius: 12px;
    border: 2px solid transparent;
    background: rgba(255, 255, 255, 0.1);
    transition: background 0.2s;
    padding: 0;
  }
  .toggle.on {
    background: #3b82f6;
  }
  .toggle-thumb {
    display: block;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
    transition: transform 0.2s;
    transform: translateX(0);
  }
  .toggle.on .toggle-thumb {
    transform: translateX(20px);
  }

  /* Segmented control */
  .segmented {
    display: flex;
    background: rgba(0, 0, 0, 0.5);
    padding: 2px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.05);
    gap: 2px;
    flex-shrink: 0;
  }

  .lang-segmented {
    flex-wrap: wrap;
  }

  .seg-btn {
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 500;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: rgba(255, 255, 255, 0.4);
    cursor: pointer;
    transition: all 0.15s;
    white-space: nowrap;
  }
  .seg-btn.active {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }
  .seg-btn:not(.active):hover {
    color: rgba(255, 255, 255, 0.6);
  }
  .seg-btn.disabled {
    color: rgba(255, 255, 255, 0.15);
    cursor: not-allowed;
  }

  /* Mode buttons */
  .mode-row {
    display: flex;
    gap: 10px;
    padding: 14px 16px;
  }

  .mode-btn {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: 12px;
    border: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(255, 255, 255, 0.02);
    cursor: pointer;
    transition: all 0.15s;
    text-align: left;
  }
  .mode-btn:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.1);
  }
  .mode-btn.active {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.2);
  }
  .mode-icon {
    font-size: 18px;
  }
  .mode-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .mode-label {
    font-size: 13px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.6);
  }
  .mode-btn.active .mode-label {
    color: #fff;
  }
  .mode-desc {
    font-size: 10px;
    color: rgba(255, 255, 255, 0.3);
  }

  /* Connection row */
  .conn-row {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .conn-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .conn-type-group {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .conn-summary {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.3);
  }

  .delete-btn {
    padding: 6px;
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.2);
    cursor: pointer;
    border-radius: 8px;
    transition: all 0.15s;
  }
  .delete-btn:hover {
    color: #f87171;
    background: rgba(248, 113, 113, 0.1);
  }

  .remote-fields {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .remote-fields input {
    background: rgba(0, 0, 0, 0.5);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 8px 12px;
    font-size: 13px;
    color: #fff;
    outline: none;
    transition: border-color 0.15s;
  }
  .remote-fields input:first-child {
    width: 90px;
  }
  .remote-fields input.flex-1 {
    flex: 1;
  }
  .remote-fields input::placeholder {
    color: rgba(255, 255, 255, 0.3);
  }
  .remote-fields input:focus {
    border-color: rgba(255, 255, 255, 0.3);
  }

  .at-sign {
    color: rgba(255, 255, 255, 0.3);
    font-size: 13px;
  }

  .conn-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .test-success {
    font-size: 11px;
    color: #34d399;
  }

  .test-error {
    font-size: 11px;
    color: #f87171;
    width: 100%;
  }
  .test-error pre {
    margin: 4px 0 0;
    padding: 8px;
    background: rgba(248, 113, 113, 0.1);
    border: 1px solid rgba(248, 113, 113, 0.2);
    border-radius: 8px;
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 100px;
    overflow-y: auto;
    font-family: monospace;
    font-size: 10px;
    line-height: 1.5;
  }

  .hook-status {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.3);
    margin-top: 2px;
  }

  .empty-msg {
    text-align: center;
    color: rgba(255, 255, 255, 0.3);
    padding: 28px 16px;
    font-size: 13px;
  }

  /* Buttons */
  .btn-small {
    padding: 6px 12px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    font-size: 11px;
    font-weight: 500;
    color: #fff;
    cursor: pointer;
    transition: all 0.15s;
    display: flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
  }
  .btn-small:hover {
    background: rgba(255, 255, 255, 0.1);
  }
  .btn-small:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .btn-primary {
    padding: 8px 16px;
    background: #3b82f6;
    border: none;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 500;
    color: #fff;
    cursor: pointer;
    transition: background 0.15s;
    white-space: nowrap;
  }
  .btn-primary:hover {
    background: #2563eb;
  }
  .btn-primary:disabled {
    opacity: 0.5;
    cursor: default;
  }

  /* Update section */
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

  /* Exit */
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

  /* Spinner */
  .spinner {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 2px solid rgba(255, 255, 255, 0.2);
    border-top-color: #fff;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
