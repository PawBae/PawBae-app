<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { UpdateModalInfo } from '../types';

  interface UpdateModalProps {
    open?: boolean;
    phase?: 'available' | 'downloading' | 'ready_to_restart';
    info?: UpdateModalInfo | null;
    progress?: number | null;
    progressStage?: string;
    errorMsg?: string;
    onLater: () => void;
    onSkipVersion: () => void;
    onUpdateNow: () => void;
    onRestartNow: () => void;
  }

  let {
    open = false,
    phase = 'available' as 'available' | 'downloading' | 'ready_to_restart',
    info = null as UpdateModalInfo | null,
    progress = null as number | null,
    progressStage = '',
    errorMsg = '',
    onLater,
    onSkipVersion,
    onUpdateNow,
    onRestartNow,
  }: UpdateModalProps = $props();

  const noteLines = $derived(
    info?.notes?.split('\n').filter((l) => l.trim()) || []
  );
</script>

{#if open}
  <div class="overlay">
    <div class="modal">
      <div class="header">
        <div class="logo">🐾</div>
        <div>
          <h3>{$_('updateModal.title')}</h3>
          {#if info}
            <span class="version">{info.current} → {info.latest}</span>
          {/if}
        </div>
      </div>

      {#if phase === 'available'}
        <p class="subtitle">{$_('updateModal.availableSubtitle')}</p>
        {#if noteLines.length > 0}
          <div class="notes">
            <div class="notes-title">{$_('updateModal.whatsNew')}</div>
            {#each noteLines as line}
              <div class="note-line">{line}</div>
            {/each}
          </div>
        {/if}
        {#if errorMsg}
          <p class="error">{$_('settings.updateFailed')}{errorMsg}</p>
        {/if}
        <div class="actions">
          <button class="btn secondary" onclick={onLater}>{$_('updateModal.later')}</button>
          <button class="btn secondary" onclick={onSkipVersion}>{$_('updateModal.skipVersion')}</button>
          <button class="btn primary" onclick={onUpdateNow}>{$_('updateModal.updateNow')}</button>
        </div>
      {:else if phase === 'downloading'}
        <div class="progress-wrap">
          <div class="progress-bar" style="width: {progress ?? 0}%"></div>
        </div>
        <p class="stage">{progressStage || $_('updateModal.progress.downloading')} {progress != null ? `${Math.round(progress)}%` : ''}</p>
        <p class="warn">{$_('updateModal.pleaseDontClose')}</p>
      {:else if phase === 'ready_to_restart'}
        <div class="done-icon">✓</div>
        <p class="subtitle">{$_('updateModal.completeDesc')}</p>
        <div class="actions">
          <button class="btn primary" onclick={onRestartNow}>{$_('updateModal.restartNow')}</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.5);
  }

  .modal {
    width: 420px;
    background: #1a1a20;
    border-radius: 16px;
    padding: 24px;
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .header {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 16px;
  }

  .logo { font-size: 28px; }

  h3 {
    color: #fff;
    font-size: 16px;
    margin: 0;
  }

  .version {
    color: rgba(255, 255, 255, 0.4);
    font-size: 12px;
  }

  .subtitle {
    color: rgba(255, 255, 255, 0.6);
    font-size: 13px;
    margin: 0 0 12px;
  }

  .notes {
    background: rgba(255, 255, 255, 0.04);
    border-radius: 8px;
    padding: 12px;
    margin-bottom: 16px;
    max-height: 120px;
    overflow-y: auto;
  }

  .notes-title {
    color: rgba(255, 255, 255, 0.45);
    font-size: 11px;
    margin-bottom: 4px;
  }

  .note-line {
    color: rgba(255, 255, 255, 0.7);
    font-size: 12px;
    line-height: 1.6;
  }

  .error {
    color: #f87171;
    font-size: 12px;
    margin: 0 0 12px;
    word-break: break-all;
  }

  .actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  .btn {
    border: none;
    border-radius: 8px;
    padding: 8px 16px;
    font-size: 13px;
    cursor: pointer;
    font-weight: 500;
  }

  .btn.primary {
    background: #6495ED;
    color: #fff;
  }

  .btn.secondary {
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.6);
  }

  .progress-wrap {
    height: 4px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 2px;
    margin-bottom: 8px;
    overflow: hidden;
  }

  .progress-bar {
    height: 100%;
    background: #6495ED;
    border-radius: 2px;
    transition: width 0.3s;
  }

  .stage {
    color: rgba(255, 255, 255, 0.6);
    font-size: 12px;
    margin: 0 0 4px;
    text-align: center;
  }

  .warn {
    color: rgba(255, 255, 255, 0.3);
    font-size: 11px;
    margin: 0;
    text-align: center;
  }

  .done-icon {
    width: 40px;
    height: 40px;
    background: #27ae60;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #fff;
    font-size: 20px;
    margin: 16px auto;
  }
</style>
