<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { settingsStore } from '../../stores/settings.svelte';

  let { isWindows }: { isWindows: boolean } = $props();

  const isPetMode = $derived(settingsStore.appMode === 'pet');
</script>

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
          role="switch" aria-label={$_('settings.petSfx')} aria-checked={settingsStore.petSfxEnabled}>
          <span class="toggle-thumb"></span>
        </button>
      </div>
    </div>
  </section>

  <section class="section">
    <h2>{$_('settings.petBehavior')}</h2>
    <div class="card">
      <div class="setting-row">
        <div class="setting-info">
          <span class="setting-label">{$_('settings.strollMode')}</span>
          <span class="setting-desc">{$_('settings.strollModeDesc')}</span>
        </div>
        <button class="toggle" class:on={settingsStore.strollEnabled}
          onclick={() => settingsStore.setStrollEnabled(!settingsStore.strollEnabled)}
          role="switch" aria-label={$_('settings.strollMode')} aria-checked={settingsStore.strollEnabled}>
          <span class="toggle-thumb"></span>
        </button>
      </div>
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
