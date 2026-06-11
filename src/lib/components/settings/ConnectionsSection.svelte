<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { _ } from 'svelte-i18n';
  import { agentStore } from '../../stores/agents.svelte';
  import { settingsStore } from '../../stores/settings.svelte';
  import type { OcConnection } from '../../types';
  import { tryInvoke } from '../../utils/invoke';

  let { open }: { open: boolean } = $props();

  let connections = $state<OcConnection[]>([]);
  let testingIdx = $state<number | null>(null);
  let testResult = $state<{ idx: number; type: 'success' | 'error'; msg: string } | null>(null);

  let prevOpen = false;

  $effect(() => {
    const isOpen = open;
    if (isOpen && !prevOpen) {
      connections = [...settingsStore.ocConnections];
    }
    prevOpen = isOpen;
  });

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
      tryInvoke('close_ssh', { sshHost: conn.host, sshUser: conn.user });
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
        await tryInvoke('reset_ssh', { sshHost: conn.host, sshUser: conn.user });
        const raw: unknown = await invoke('get_agents', { mode: 'remote', sshHost: conn.host, sshUser: conn.user });
        const agents = Array.isArray(raw) ? raw : [];
        let keyInfo = '';
        try {
          const key: unknown = await invoke('get_ssh_key_info', { sshHost: conn.host, sshUser: conn.user });
          if (typeof key === 'string' && key) keyInfo = ` · ${$_('settings.key')} ${key}`;
        } catch {}
        testResult = { idx, type: 'success', msg: `${agents.length} ${$_('settings.agents')}${keyInfo}` };
      } else {
        const store = await settingsStore.getStore();
        const agentId = ((await store.get('tracked_agent')) as string) || 'main';
        const raw: unknown = await invoke('get_status', { gatewayUrl: 'http://localhost:4446', token: '', agentId });
        const sessions = (raw && typeof raw === 'object' && 'sessions' in raw && Array.isArray((raw as Record<string, unknown>).sessions))
          ? (raw as Record<string, unknown>).sessions as unknown[]
          : [];
        testResult = { idx, type: 'success', msg: `${sessions.length} ${$_('settings.sessions')}` };
      }
      setTimeout(() => { if (testResult?.idx === idx) testResult = null; }, 3000);
    } catch (e: unknown) {
      testResult = { idx, type: 'error', msg: String(e) };
    }
    testingIdx = null;
  }
</script>

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
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" role="img" aria-label="Delete">
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

<style>
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

  .empty-msg {
    text-align: center;
    color: rgba(255, 255, 255, 0.3);
    padding: 28px 16px;
    font-size: 13px;
  }
</style>
