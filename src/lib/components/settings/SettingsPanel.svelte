<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { settingsStore } from '../../stores/settings.svelte';
  import AboutSection from './AboutSection.svelte';
  import AppModeSection from './AppModeSection.svelte';
  import ConnectionsSection from './ConnectionsSection.svelte';
  import DisplaySection from './DisplaySection.svelte';
  import IntegrationToggles from './IntegrationToggles.svelte';
  import PetSettingsSection from './PetSettingsSection.svelte';
  import PrivacySection from './PrivacySection.svelte';
  import SoundSection from './SoundSection.svelte';

  let {
    open = false,
    onClose,
  }: {
    open?: boolean;
    onClose: () => void;
  } = $props();

  const isWindows = typeof navigator !== 'undefined' && navigator.userAgent.includes('Windows');
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
        <AppModeSection />
        <PetSettingsSection {isWindows} />

        {#if !isPetMode}
          <ConnectionsSection {open} />
          <IntegrationToggles {isWindows} {open} />
          <DisplaySection {isWindows} />
          <SoundSection {isWindows} />
        {/if}

        {#if !isWindows}
          <!-- Global input sensing is macOS-only (Phase 1); hide where it can't run. -->
          <PrivacySection {open} />
        {/if}

        <AboutSection {open} />
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

  /* Shared UI primitives — child components inherit these via :global */
  .settings-content :global(.section) {
    margin-bottom: 28px;
  }

  .settings-content :global(.section h2) {
    font-size: 15px;
    font-weight: 500;
    color: #fff;
    margin: 0 0 10px;
  }

  .settings-content :global(.section-header) {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }

  .settings-content :global(.section-header h2) {
    margin: 0;
  }

  .settings-content :global(.card) {
    background: #0f0f0f;
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 16px;
    overflow: hidden;
  }

  .settings-content :global(.setting-row) {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px;
    gap: 12px;
  }

  .settings-content :global(.border-bottom) {
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .settings-content :global(.border-top) {
    border-top: 1px solid rgba(255, 255, 255, 0.05);
  }

  .settings-content :global(.setting-info) {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    flex: 1;
  }

  .settings-content :global(.setting-label) {
    font-size: 13px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.9);
  }

  .settings-content :global(.setting-desc) {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.4);
  }

  .settings-content :global(.setting-value) {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.6);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .settings-content :global(.slider-row) {
    padding-bottom: 4px;
  }

  .settings-content :global(.slider-wrap) {
    padding: 0 16px 14px;
  }

  .settings-content :global(input[type="range"]) {
    width: 100%;
    height: 4px;
    accent-color: rgba(255, 255, 255, 0.6);
    cursor: pointer;
  }

  /* Toggle switch */
  .settings-content :global(.toggle) {
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
  .settings-content :global(.toggle.on) {
    background: #3b82f6;
  }
  .settings-content :global(.toggle-thumb) {
    display: block;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
    transition: transform 0.2s;
    transform: translateX(0);
  }
  .settings-content :global(.toggle.on .toggle-thumb) {
    transform: translateX(20px);
  }

  /* Segmented control */
  .settings-content :global(.segmented) {
    display: flex;
    background: rgba(0, 0, 0, 0.5);
    padding: 2px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.05);
    gap: 2px;
    flex-shrink: 0;
  }

  .settings-content :global(.seg-btn) {
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
  .settings-content :global(.seg-btn.active) {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }
  .settings-content :global(.seg-btn:not(.active):hover) {
    color: rgba(255, 255, 255, 0.6);
  }
  .settings-content :global(.seg-btn.disabled) {
    color: rgba(255, 255, 255, 0.15);
    cursor: not-allowed;
  }

  /* Buttons */
  .settings-content :global(.btn-small) {
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
  .settings-content :global(.btn-small:hover) {
    background: rgba(255, 255, 255, 0.1);
  }
  .settings-content :global(.btn-small:disabled) {
    opacity: 0.5;
    cursor: default;
  }

  .settings-content :global(.btn-primary) {
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
  .settings-content :global(.btn-primary:hover) {
    background: #2563eb;
  }
  .settings-content :global(.btn-primary:disabled) {
    opacity: 0.5;
    cursor: default;
  }

  /* Spinner */
  .settings-content :global(.spinner) {
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
