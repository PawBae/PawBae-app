<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { AgentId, AgentInstallStatus } from '../../utils/onboarding';

  let {
    id,
    selected,
    available,
    status,
    error,
    onToggle,
    onRetry,
  }: {
    id: AgentId;
    selected: boolean;
    available: boolean;
    status: AgentInstallStatus;
    error: string;
    onToggle: () => void;
    onRetry: () => void;
  } = $props();

  const names: Record<AgentId, string> = {
    claude: 'Claude Code',
    codex: 'Codex',
    cursor: 'Cursor',
  };
  const marks: Record<AgentId, string> = {
    claude: 'C',
    codex: '⌘',
    cursor: '↗',
  };
  const unavailable = $derived(!available || status === 'installing');
</script>

<div class="agent-row" class:selected class:failed={status === 'failed'} data-agent={id}>
  <span class="agent-mark" aria-hidden="true">{marks[id]}</span>
  <span class="agent-copy">
    <strong>{names[id]}</strong>
    <span>{$_(`onboarding.agents.${id}Desc`)}</span>
    {#if !available}
      <small>{$_('onboarding.agents.unsupportedWindows')}</small>
    {:else if status === 'installing'}
      <small>{$_('onboarding.agents.installing')}</small>
    {:else if status === 'connected'}
      <small class="success">✓ {$_('onboarding.agents.connected')}</small>
    {:else if status === 'failed'}
      <small class="error">{error || $_('onboarding.agents.failed')}</small>
    {/if}
  </span>
  {#if status === 'failed' && available}
    <button class="retry" type="button" onclick={onRetry}>{$_('common.retry')}</button>
  {/if}
  <button
    class="switch"
    class:on={selected}
    type="button"
    role="switch"
    aria-label={names[id]}
    aria-checked={selected}
    aria-disabled={unavailable}
    disabled={unavailable}
    onclick={onToggle}
  >
    <span></span>
  </button>
</div>

<style>
  .agent-row {
    display: grid;
    grid-template-columns: 42px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 14px;
    min-height: 72px;
    padding: 12px 14px;
    border: 1px solid var(--ob-border);
    border-radius: 14px;
    background: var(--ob-surface);
    transition: border-color 180ms ease-out, background 180ms ease-out;
  }
  .agent-row.selected { border-color: color-mix(in srgb, var(--ob-action) 55%, var(--ob-border)); }
  .agent-row.failed { border-color: var(--ob-danger); }
  .agent-mark {
    display: grid;
    width: 38px;
    height: 38px;
    place-items: center;
    border-radius: 11px;
    background: var(--ob-subtle);
    color: var(--ob-text);
    font-size: 15px;
    font-weight: 750;
  }
  .agent-copy { display: grid; gap: 3px; min-width: 0; }
  .agent-copy strong { color: var(--ob-text); font-size: 14px; }
  .agent-copy > span { color: var(--ob-text-muted); font-size: 12px; }
  small { color: var(--ob-text-muted); font-size: 11px; line-height: 1.35; }
  .success { color: var(--ob-success); }
  .error { color: var(--ob-danger); max-width: 42ch; }
  .retry {
    border: 0;
    padding: 6px 8px;
    border-radius: 8px;
    background: transparent;
    color: var(--ob-action);
    font: inherit;
    font-size: 12px;
    font-weight: 650;
    cursor: pointer;
  }
  .retry:focus-visible, .switch:focus-visible { outline: 2px solid var(--ob-focus); outline-offset: 2px; }
  .switch {
    position: relative;
    width: 40px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 999px;
    background: var(--ob-border-strong);
    cursor: pointer;
    transition: background 180ms ease-out;
  }
  .switch span {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgba(28, 23, 29, 0.2);
    transition: transform 180ms ease-out;
  }
  .switch.on { background: var(--ob-action); }
  .switch.on span { transform: translateX(16px); }
  .switch:disabled { cursor: not-allowed; opacity: 0.5; }
  @media (prefers-reduced-motion: reduce) {
    .agent-row, .switch, .switch span { transition: none; }
  }
</style>
