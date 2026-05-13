<script lang="ts">
  import { onMount } from 'svelte';
  import { loadDefaultCodexPet, type CodexPet, type CodexPetState } from './lib/codexPet';
  import SpritePet from './lib/SpritePet.svelte';

  let pet = $state<CodexPet | null>(null);
  let petState = $state<CodexPetState>('idle');

  onMount(async () => {
    pet = await loadDefaultCodexPet();
  });

  function handleClick() {
    if (!pet) return;
    if (pet.animations['jumping']) {
      petState = 'jumping';
    }
  }

  function handleOneShotEnd() {
    petState = 'idle';
  }
</script>

<main data-tauri-drag-region>
  {#if pet}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="pet-container" onclick={handleClick}>
      <SpritePet 
        {pet} 
        state={petState} 
        size={200} 
        onOneShotEnd={handleOneShotEnd}
      />
    </div>
  {:else}
    <div class="loading">Loading...</div>
  {/if}
</main>

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    overflow: hidden;
    background: transparent;
    width: 100%;
    height: 100%;
  }

  main {
    width: 100vw;
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: grab;
  }

  main:active {
    cursor: grabbing;
  }

  .pet-container {
    cursor: pointer;
    user-select: none;
    -webkit-user-drag: none;
  }

  .loading {
    color: white;
    font-family: sans-serif;
  }
</style>
