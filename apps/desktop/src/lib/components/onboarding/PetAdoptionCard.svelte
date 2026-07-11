<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { OfficialPet } from '../../utils/onboarding';

  let {
    pet,
    selected,
    tabIndex,
    onSelect,
  }: {
    pet: OfficialPet;
    selected: boolean;
    tabIndex: number;
    onSelect: () => void;
  } = $props();

  const positions = ['0%', '33.333%', '66.667%', '100%'] as const;
  const name = $derived($_(`onboarding.adopt.${pet.id}Name`));
</script>

<button
  class="pet-card"
  class:selected
  type="button"
  role="radio"
  data-pet-id={pet.id}
  tabindex={tabIndex}
  aria-checked={selected}
  aria-label={selected ? $_('onboarding.adopt.selected', { values: { name } }) : name}
  onclick={onSelect}
  style={`--pet-color:${pet.color};--pet-strong:${pet.strongColor};--poster-x:${positions[pet.posterIndex]}`}
>
  <span class="pet-art" aria-hidden="true"></span>
  {#if selected}<span class="check" aria-hidden="true">✓</span>{/if}
  <span class="pet-name">{name}</span>
  <span class="pet-trait">{$_(`onboarding.adopt.${pet.id}Trait`)}</span>
</button>

<style>
  .pet-card {
    position: relative;
    display: grid;
    grid-template-rows: minmax(180px, 1fr) auto auto;
    gap: 5px;
    min-width: 0;
    min-height: 268px;
    padding: 0 12px 16px;
    overflow: hidden;
    border: 1px solid var(--ob-border);
    border-radius: 16px;
    background: var(--ob-surface);
    color: var(--ob-text);
    text-align: center;
    cursor: pointer;
    transition: border-color 180ms ease-out, background 180ms ease-out, transform 180ms ease-out;
  }
  .pet-card:hover { transform: translateY(-2px); border-color: var(--ob-border-strong); }
  .pet-card:focus-visible { outline: 2px solid var(--ob-focus); outline-offset: 3px; }
  .pet-card.selected {
    border: 2px solid var(--pet-color);
    padding: 0 11px 15px;
    background: color-mix(in srgb, var(--pet-color) 8%, var(--ob-surface));
  }
  .pet-art {
    align-self: stretch;
    margin: 0 -12px 4px;
    background-image: url('/assets/onboarding/pet-family-poster.png');
    background-size: 400% auto;
    background-position: var(--poster-x) 36%;
    background-repeat: no-repeat;
  }
  .selected .pet-art { margin-inline: -11px; }
  .check {
    position: absolute;
    top: 10px;
    right: 10px;
    display: grid;
    width: 26px;
    height: 26px;
    place-items: center;
    border-radius: 50%;
    background: var(--pet-color);
    color: #fff;
    font-size: 14px;
    font-weight: 800;
  }
  .pet-name { color: var(--ob-text); font-size: 16px; font-weight: 750; }
  .pet-trait { color: var(--pet-strong); font-size: 12px; font-weight: 600; }
  @media (max-width: 820px) {
    .pet-card {
      grid-template-rows: minmax(160px, 1fr) auto auto;
      gap: 4px;
      min-height: 244px;
      padding: 0 8px 12px;
      border-radius: 14px;
    }
    .pet-card.selected { padding: 0 7px 11px; }
    .pet-art { margin: 0 -8px 4px; }
    .selected .pet-art { margin-inline: -7px; }
    .check { top: 8px; right: 8px; width: 22px; height: 22px; font-size: 12px; }
    .pet-name { font-size: 14px; }
    .pet-trait { font-size: 11px; }
  }
  @media (max-width: 640px) {
    .pet-card { grid-template-rows: minmax(148px, 1fr) auto auto; min-height: 230px; padding-inline: 6px; }
    .pet-card.selected { padding-inline: 5px; }
    .pet-art { margin-inline: -6px; }
    .selected .pet-art { margin-inline: -5px; }
  }
  @media (prefers-reduced-motion: reduce) {
    .pet-card { transition: none; }
    .pet-card:hover { transform: none; }
  }
</style>
