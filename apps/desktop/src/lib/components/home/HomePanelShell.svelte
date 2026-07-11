<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { _ } from 'svelte-i18n';
  import type { HomePanel } from '../../utils/social-home';

  let {
    panel,
    title,
    subtitle,
    onClose,
    children,
  }: {
    panel: Exclude<HomePanel, null>;
    title: string;
    subtitle?: string;
    onClose: () => void;
    children: Snippet;
  } = $props();

  let heading: HTMLHeadingElement;

  onMount(() => heading.focus());

  function handleKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    onClose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<aside
  id={`${panel}-panel`}
  class="home-panel"
  data-panel={panel}
  aria-labelledby={`${panel}-panel-title`}
>
  <header>
    <div>
      <h2
        id={`${panel}-panel-title`}
        data-panel-heading
        data-friends-heading={panel === 'friends' ? '' : undefined}
        tabindex="-1"
        bind:this={heading}
      >{title}</h2>
      {#if subtitle}
        <p data-album-privacy={panel === 'album' ? '' : undefined}>{subtitle}</p>
      {/if}
    </div>
    <button class="close-action" type="button" aria-label={$_('common.close')} onclick={onClose}>
      ×
    </button>
  </header>

  <div class="panel-scroll">
    {@render children()}
  </div>
</aside>

<style>
  .home-panel {
    position: absolute;
    top: 16px;
    right: 104px;
    bottom: 16px;
    z-index: 3;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    width: clamp(360px, calc(100% - 32px), 372px);
    overflow: hidden;
    border-radius: 22px;
    background: var(--home-surface);
    box-shadow: 0 8px 16px rgba(38, 30, 36, 0.14);
    animation: panel-in 220ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  header {
    display: flex;
    min-height: 74px;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 14px 10px 18px;
    border-bottom: 1px solid var(--home-border);
  }

  header > div {
    min-width: 0;
  }

  h2,
  p {
    margin: 0;
  }

  h2 {
    color: var(--home-text);
    font-family: 'M PLUS Rounded 1c', 'Noto Sans SC', 'Segoe UI', sans-serif;
    font-size: 19px;
    letter-spacing: -0.02em;
    line-height: 1.25;
    text-wrap: balance;
  }

  h2:focus-visible {
    border-radius: 4px;
    outline: 2px solid var(--home-focus);
    outline-offset: 3px;
  }

  header p {
    margin-top: 3px;
    color: var(--home-text-muted);
    font-size: 12px;
    line-height: 1.35;
    text-wrap: pretty;
  }

  .close-action {
    display: grid;
    width: 36px;
    min-height: 36px;
    flex: 0 0 auto;
    place-items: center;
    border: 0;
    border-radius: 50%;
    background: var(--home-subtle);
    color: var(--home-text);
    font: inherit;
    font-size: 20px;
    line-height: 1;
    cursor: pointer;
  }

  .close-action:hover {
    background: color-mix(in srgb, var(--home-action) 12%, var(--home-subtle));
  }

  .close-action:focus-visible {
    outline: 2px solid var(--home-focus);
    outline-offset: 2px;
  }

  .panel-scroll {
    min-height: 0;
    padding: 14px 18px 18px;
    overflow-y: auto;
  }

  @keyframes panel-in {
    from {
      opacity: 0;
      transform: translateX(14px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }

  @media (max-width: 520px) {
    .home-panel {
      left: 16px;
      width: auto;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .home-panel {
      animation: none;
    }
  }
</style>
