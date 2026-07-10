<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { settingsStore } from '../../stores/settings.svelte';
  import { STAGE_BG_COLORS, STAGE_BGS } from '../../utils/stage-bridge';
  import { track } from '../../utils/telemetry';

  let { isWindows }: { isWindows: boolean } = $props();

  // Telemetry stays at the UI layer (telemetry.ts imports the settings store —
  // the same no-cycle rule as pet_renamed).
  function toggleStage() {
    const next = !settingsStore.streamStageEnabled;
    void settingsStore.setStreamStageEnabled(next);
    track('stream_stage_toggled', { on: next ? 'on' : 'off' });
  }
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
      <div class="slider-wrap border-bottom">
        <input type="range" min={1} max={6} step={0.1}
          value={settingsStore.largeMascotScale}
          oninput={(e) => settingsStore.setLargeMascotScale(Number(e.currentTarget.value))}
        />
      </div>
    {/if}
    <div class="setting-row" class:border-bottom={settingsStore.streamStageEnabled}>
      <div class="setting-info">
        <span class="setting-label">{$_('settings.stageToggle')}</span>
        <span class="setting-desc">{$_('settings.stageToggleDesc')}</span>
      </div>
      <button class="toggle" class:on={settingsStore.streamStageEnabled}
        onclick={toggleStage}
        role="switch" aria-checked={settingsStore.streamStageEnabled}
        aria-label={$_('settings.stageToggle')}>
        <span class="toggle-thumb"></span>
      </button>
    </div>
    {#if settingsStore.streamStageEnabled}
      <div class="setting-row">
        <div class="setting-info">
          <span class="setting-label">{$_('settings.stageBg')}</span>
          <span class="setting-desc">{$_('settings.stageHint')}</span>
        </div>
        <div class="stage-bgs">
          {#each STAGE_BGS as bgOption (bgOption)}
            <button
              class="stage-swatch"
              class:active={settingsStore.streamStageBg === bgOption}
              style="background: {STAGE_BG_COLORS[bgOption]};"
              title={$_(`settings.stageBg_${bgOption}`)}
              aria-label={$_(`settings.stageBg_${bgOption}`)}
              onclick={() => settingsStore.setStreamStageBg(bgOption)}
            ></button>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</section>

<style>
  .stage-bgs {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  .stage-swatch {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    border: 2px solid transparent;
    padding: 0;
    cursor: pointer;
  }

  .stage-swatch.active {
    border-color: #fff;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.35);
  }
</style>
