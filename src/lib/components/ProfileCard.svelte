<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { petStore } from '../stores/pet.svelte';
  import { settingsStore } from '../stores/settings.svelte';
  import type { CodexPet } from '../utils/codex-pet';
  import { loadCodexPets, loadDefaultCodexPet } from '../utils/codex-pet';
  import { effectiveName, NICKNAME_MAX } from '../utils/pet-name';
  import { track } from '../utils/telemetry';
  import SpritePet from './SpritePet.svelte';

  let { open = false, onclose }: { open?: boolean; onclose: () => void } = $props();

  let pet = $state<CodexPet | null>(null);
  let draft = $state('');

  // Self-contained like ShareCardModal: resolve the active pet on first open and
  // whenever the user has switched pets since.
  $effect(() => {
    if (!open) return;
    const wanted = settingsStore.miniPetId;
    if (pet && pet.id === wanted) return;
    void (async () => {
      const pets = await loadCodexPets();
      pet = pets.find((p) => p.id === wanted) ?? (await loadDefaultCodexPet());
      draft = pet ? (settingsStore.petNicknames[pet.id] ?? '') : '';
    })();
  });

  async function commitNickname() {
    if (!pet) return;
    const changed = await settingsStore.setPetNickname(pet.id, draft);
    draft = settingsStore.petNicknames[pet.id] ?? '';
    // Never carries the name itself — privacy over curiosity.
    if (changed) track('pet_renamed');
  }
</script>

{#if open}
  <div class="overlay" role="presentation" onclick={onclose}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <div class="portrait">
        {#if pet}
          <SpritePet {pet} state="idle" size={72} />
        {:else}
          <span class="portrait-fallback">🐾</span>
        {/if}
      </div>

      <h3 class="name">{effectiveName(pet ? settingsStore.petNicknames[pet.id] : '', pet?.displayName ?? 'PawBae')}</h3>
      <p class="species">{$_('lore.species')}</p>

      <label class="nickname">
        <span>{$_('profile.nicknameLabel')}</span>
        <input
          type="text"
          maxlength={NICKNAME_MAX}
          placeholder={pet?.displayName ?? 'PawBae'}
          bind:value={draft}
          onblur={commitNickname}
          onkeydown={(e) => {
            if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur();
          }}
        />
      </label>

      <p class="lore">{$_('lore.short')}</p>

      <p class="together">
        {$_('growth.daysTogether', { values: { days: petStore.daysTogether + 1 } })}
      </p>

      <button class="close" onclick={onclose}>✕</button>
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
    position: relative;
    background: #1a1a20;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    padding: 20px 22px 16px;
    width: 240px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    text-align: center;
  }

  .portrait {
    height: 76px;
    display: flex;
    align-items: flex-end;
    justify-content: center;
  }

  .portrait-fallback {
    font-size: 40px;
  }

  .name {
    margin: 0;
    color: #fff;
    font-size: 17px;
  }

  .species {
    margin: -4px 0 0;
    color: rgba(100, 149, 237, 0.9);
    font-size: 11px;
  }

  .nickname {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 4px;
  }

  .nickname span {
    color: rgba(255, 255, 255, 0.45);
    font-size: 11px;
    text-align: left;
  }

  .nickname input {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 8px;
    color: #fff;
    font-size: 13px;
    padding: 6px 10px;
    text-align: center;
  }

  .nickname input:focus {
    outline: none;
    border-color: rgba(100, 149, 237, 0.6);
  }

  .lore {
    margin: 6px 0 0;
    color: rgba(255, 255, 255, 0.65);
    font-size: 12px;
    line-height: 1.6;
  }

  .together {
    margin: 2px 0 0;
    color: rgba(255, 255, 255, 0.4);
    font-size: 11px;
  }

  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.4);
    font-size: 13px;
    cursor: pointer;
  }

  .close:hover {
    color: #fff;
  }
</style>
