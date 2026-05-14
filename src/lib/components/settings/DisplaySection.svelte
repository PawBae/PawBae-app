<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { settingsStore } from '../../stores/settings.svelte';

  let { isWindows }: { isWindows: boolean } = $props();
</script>

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
