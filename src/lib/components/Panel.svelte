<script lang="ts">
  import { windowStore } from '../stores/window.svelte';
  import { settingsStore } from '../stores/settings.svelte';
  import { agentStore } from '../stores/agents.svelte';
  import { sessionStore } from '../stores/sessions.svelte';
  import { petStore } from '../stores/pet.svelte';

  let {
    class: className = '',
  }: {
    class?: string;
  } = $props();

  const maxHeight = $derived(settingsStore.panelMaxHeight);
</script>

{#if windowStore.expanded}
  <div
    class="panel {className}"
    style="max-height: {maxHeight}px;"
  >
    <div class="panel-content">
      {#if settingsStore.appMode === 'coding'}
        <div class="session-list">
          {#if sessionStore.claudeSessions.length === 0 && agentStore.agents.length === 0}
            <div class="empty">
              <p>No active sessions</p>
              <p class="hint">Start Claude Code, Codex, or Cursor to see sessions here</p>
            </div>
          {:else}
            {#each sessionStore.claudeSessions as session (session.id)}
              <button
                class="session-card"
                class:active={session.status === 'active'}
                onclick={() => sessionStore.selectSession(session.id)}
              >
                <div class="session-header">
                  <span class="session-source">{session.source || 'cc'}</span>
                  <span class="dot" class:active={session.status === 'active'}></span>
                </div>
                <div class="session-cwd">{session.cwd || session.id}</div>
                {#if session.model}
                  <div class="session-model">{session.model}</div>
                {/if}
              </button>
            {/each}

            {#each agentStore.agents as agent (agent.id)}
              <button
                class="session-card"
                class:active={agentStore.healthMap[agent.id]}
                onclick={() => agentStore.selectAgent(agent.id)}
              >
                <div class="session-header">
                  <span class="session-source">
                    {agent.identityEmoji || '🤖'} {agent.identityName || agent.id}
                  </span>
                  <span class="dot" class:active={agentStore.healthMap[agent.id]}></span>
                </div>
              </button>
            {/each}
          {/if}
        </div>
      {:else}
        <div class="pet-panel">
          <p class="pet-status">
            ❤️ {Math.round(petStore.petData.affection)} &nbsp;
            🍗 {Math.round(petStore.petData.hunger)} &nbsp;
            🪙 {petStore.petData.coins}
          </p>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .panel {
    background: rgba(26, 26, 32, 0.95);
    backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 12px;
    overflow-y: auto;
    overflow-x: hidden;
    width: 100%;
  }

  .panel-content {
    padding: 8px;
  }

  .session-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .empty {
    text-align: center;
    padding: 20px 12px;
  }

  .empty p {
    color: rgba(255, 255, 255, 0.5);
    font-size: 12px;
    margin: 0;
  }

  .empty .hint {
    color: rgba(255, 255, 255, 0.3);
    font-size: 11px;
    margin-top: 4px;
  }

  .session-card {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 8px;
    padding: 8px 10px;
    cursor: pointer;
    text-align: left;
    width: 100%;
    transition: all 0.15s;
    color: inherit;
    font: inherit;
  }

  .session-card:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  .session-card.active {
    border-color: rgba(100, 149, 237, 0.3);
  }

  .session-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .session-source {
    color: rgba(255, 255, 255, 0.8);
    font-size: 12px;
    font-weight: 500;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.2);
  }

  .dot.active {
    background: #27ae60;
  }

  .session-cwd {
    color: rgba(255, 255, 255, 0.4);
    font-size: 10px;
    margin-top: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-model {
    color: rgba(255, 255, 255, 0.3);
    font-size: 10px;
    margin-top: 1px;
  }

  .pet-panel {
    padding: 12px;
    text-align: center;
  }

  .pet-status {
    color: rgba(255, 255, 255, 0.7);
    font-size: 13px;
    margin: 0;
  }
</style>
