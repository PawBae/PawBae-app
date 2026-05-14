<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { _ } from 'svelte-i18n';
  import { settingsStore } from '../../stores/settings.svelte';

  let { isWindows, open }: { isWindows: boolean; open: boolean } = $props();

  let enableClaudeCode = $state(true);
  let hookStatus = $state('');
  let enableCodex = $state(true);
  let codexHookStatus = $state('');
  let enableCursor = $state(true);
  let cursorHookStatus = $state('');

  let prevOpen = false;

  $effect(() => {
    const isOpen = open;
    if (isOpen && !prevOpen) {
      enableClaudeCode = settingsStore.enableClaudeCode;
      enableCodex = isWindows ? false : settingsStore.enableCodex;
      enableCursor = isWindows ? false : settingsStore.enableCursor;
    }
    prevOpen = isOpen;
  });

  async function toggleClaudeCode(val: boolean) {
    enableClaudeCode = val;
    if (val) {
      try {
        await invoke('install_claude_hooks');
        hookStatus = $_('settings.hookInstalled');
        await settingsStore.setEnableClaudeCode(val);
      } catch (e: unknown) {
        enableClaudeCode = false;
        hookStatus = `${$_('settings.hookFailed')} ${String(e)}`;
      }
    } else {
      await settingsStore.setEnableClaudeCode(val);
    }
  }

  async function toggleCodex(val: boolean) {
    enableCodex = val;
    if (val) {
      try {
        await invoke('install_claude_hooks');
        codexHookStatus = $_('settings.hookInstalled');
        await settingsStore.setEnableCodex(val);
      } catch (e: unknown) {
        enableCodex = false;
        codexHookStatus = `${$_('settings.hookFailed')} ${String(e)}`;
      }
    } else {
      await settingsStore.setEnableCodex(val);
    }
  }

  async function toggleCursor(val: boolean) {
    enableCursor = val;
    if (val) {
      try {
        await invoke('install_cursor_hooks');
        cursorHookStatus = $_('settings.hookInstalled');
        await settingsStore.setEnableCursor(val);
      } catch (e: unknown) {
        enableCursor = false;
        cursorHookStatus = `${$_('settings.hookFailed')} ${String(e)}`;
      }
    } else {
      await settingsStore.setEnableCursor(val);
    }
  }
</script>

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

<style>
  .hook-status {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.3);
    margin-top: 2px;
  }
</style>
