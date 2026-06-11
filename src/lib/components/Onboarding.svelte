<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { AppMode } from '../types';
  import type { CodexPet } from '../utils/codex-pet';
  import { loadDefaultCodexPet } from '../utils/codex-pet';
  import SpritePet from './SpritePet.svelte';

  interface OnboardingProps {
    open?: boolean;
    onSelect: (mode: AppMode) => void;
  }

  let {
    open = false,
    onSelect,
  }: OnboardingProps = $props();

  let previewPet = $state<CodexPet | null>(null);

  $effect(() => {
    if (open && !previewPet) {
      loadDefaultCodexPet().then((p) => { previewPet = p; });
    }
  });
</script>

{#if open}
  <div class="overlay">
    <div class="modal">
      <h2>{$_('onboarding.chooseModeTitle')}</h2>
      <p class="subtitle">{$_('onboarding.chooseModeSubtitle')}</p>

      <div class="cards">
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="card coding" onclick={() => onSelect('coding')}>
          <div class="preview">
            {#if previewPet}
              <SpritePet pet={previewPet} state="running" size={64} />
            {/if}
          </div>
          <div class="badge">{$_('onboarding.recommended')}</div>
          <h3>{$_('settings.codingMode')}</h3>
          <p>{$_('onboarding.codingModeLongDesc')}</p>
        </div>

        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="card pet" onclick={() => onSelect('pet')}>
          <div class="preview">
            {#if previewPet}
              <SpritePet pet={previewPet} state="idle" size={64} />
            {/if}
          </div>
          <h3>{$_('settings.petMode')}</h3>
          <p>{$_('onboarding.petModeLongDesc')}</p>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 9999;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.6);
  }

  .modal {
    background: #1a1a20;
    border-radius: 20px;
    padding: 32px;
    text-align: center;
    border: 1px solid rgba(255, 255, 255, 0.08);
    max-width: 420px;
  }

  h2 {
    color: #fff;
    font-size: 20px;
    margin: 0 0 8px;
  }

  .subtitle {
    color: rgba(255, 255, 255, 0.5);
    font-size: 13px;
    margin: 0 0 24px;
  }

  .cards {
    display: flex;
    gap: 16px;
  }

  .card {
    flex: 1;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    padding: 20px 16px;
    cursor: pointer;
    transition: all 0.2s;
    position: relative;
  }

  .card:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.15);
    transform: translateY(-2px);
  }

  .card.coding {
    border-color: rgba(100, 149, 237, 0.3);
  }

  .card.coding:hover {
    border-color: rgba(100, 149, 237, 0.6);
  }

  .preview {
    height: 80px;
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 12px;
  }

  .badge {
    position: absolute;
    top: 8px;
    right: 8px;
    background: rgba(100, 149, 237, 0.2);
    color: #6495ED;
    font-size: 10px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 8px;
  }

  h3 {
    color: #fff;
    font-size: 14px;
    margin: 0 0 6px;
  }

  .card p {
    color: rgba(255, 255, 255, 0.5);
    font-size: 11px;
    margin: 0;
    line-height: 1.4;
  }
</style>
