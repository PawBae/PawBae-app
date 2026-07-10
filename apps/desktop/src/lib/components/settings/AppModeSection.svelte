<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { settingsStore } from '../../stores/settings.svelte';
</script>

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

<style>
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
</style>
