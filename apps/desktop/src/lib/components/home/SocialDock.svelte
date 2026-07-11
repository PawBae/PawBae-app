<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { HomePanel } from '../../utils/social-home';

  let { activePanel = $bindable(null) }: { activePanel?: HomePanel } = $props();

  const destinations: { id: Exclude<HomePanel, null>; mark: string }[] = [
    { id: 'friends', mark: 'F' },
    { id: 'plaza', mark: 'P' },
    { id: 'album', mark: 'A' },
  ];

  function select(id: Exclude<HomePanel, null>) {
    activePanel = activePanel === id ? null : id;
  }
</script>

<nav class="social-dock" aria-label={$_('home.title')}>
  {#each destinations as destination}
    <button
      type="button"
      data-dock={destination.id}
      class:active={activePanel === destination.id}
      aria-pressed={activePanel === destination.id}
      aria-expanded={activePanel === destination.id}
      aria-controls={`${destination.id}-panel`}
      onclick={() => select(destination.id)}
    >
      <span class="dock-mark" aria-hidden="true">{destination.mark}</span>
      <span>{$_(`home.nav.${destination.id}`)}</span>
      {#if destination.id === 'plaza'}
        <small>{$_('home.nav.soon')}</small>
      {/if}
    </button>
  {/each}
</nav>

<style>
  .social-dock {
    display: grid;
    gap: 4px;
    width: 72px;
    padding: 5px;
    border: 1px solid var(--home-border);
    border-radius: 999px;
    background: var(--home-surface);
  }

  button {
    display: grid;
    min-height: 64px;
    place-items: center;
    align-content: center;
    gap: 3px;
    padding: 6px 3px;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: var(--home-text-muted);
    font: inherit;
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
    transition: background 180ms ease-out, color 180ms ease-out, transform 180ms ease-out;
  }

  button:hover,
  button.active {
    background: var(--home-subtle);
    color: var(--home-text);
  }

  button[aria-pressed='true'] {
    box-shadow: inset 0 0 0 2px var(--home-focus);
  }

  button:active {
    transform: scale(0.98);
  }

  button:focus-visible {
    outline: 2px solid var(--home-focus);
    outline-offset: 2px;
  }

  .dock-mark {
    display: grid;
    width: 24px;
    height: 24px;
    place-items: center;
    border-radius: 50%;
    background: color-mix(in srgb, var(--home-action) 12%, var(--home-surface));
    color: var(--home-action);
    font-size: 12px;
  }

  small {
    color: var(--home-text-muted);
    font-size: 12px;
    font-weight: 650;
    white-space: nowrap;
  }

  @media (prefers-reduced-motion: reduce) {
    button {
      transition: none;
    }

    button:active {
      transform: none;
    }
  }
</style>
