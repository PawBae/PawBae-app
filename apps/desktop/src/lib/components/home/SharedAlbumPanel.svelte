<script lang="ts">
  import { _, locale } from 'svelte-i18n';
  import type { HomePanel, SharedMemorySummary } from '../../utils/social-home';
  import HomePanelShell from './HomePanelShell.svelte';

  type AlbumPanel = Extract<HomePanel, 'album' | 'plaza'>;

  let {
    panel,
    memories,
    onClose,
    onOpenMemory,
  }: {
    panel: AlbumPanel;
    memories: SharedMemorySummary[];
    onClose: () => void;
    onOpenMemory?: (memoryId: string) => void;
  } = $props();

  function petMark(petId: string) {
    return petId.trim().slice(0, 1).toLocaleUpperCase();
  }

  function memoryTitle(memory: SharedMemorySummary) {
    return $_(`home.memory.templates.${memory.templateKey}`, {
      values: { photoCount: memory.templateKey === 'shared-photo' ? memory.params.photoCount : 0 },
    });
  }

  function memoryDate(memory: SharedMemorySummary) {
    return new Intl.DateTimeFormat($locale ?? 'en', {
      month: 'short',
      day: 'numeric',
      timeZone: 'UTC',
    }).format(memory.occurredAt);
  }
</script>

<HomePanelShell
  {panel}
  title={$_(`home.${panel}.title`)}
  subtitle={panel === 'album' ? $_('home.album.privacy') : undefined}
  {onClose}
>
  {#if panel === 'album'}
    {#if memories.length === 0}
      <div class="album-empty" data-album-empty>
        <span class="empty-mark" aria-hidden="true">⌁</span>
        <strong>{$_('home.album.emptyTitle')}</strong>
        <p>{$_('home.album.emptyBody')}</p>
      </div>
    {:else}
      <div class="memory-grid" data-album-grid>
        {#each memories as memory (memory.id)}
          {@const title = memoryTitle(memory)}
          {@const date = memoryDate(memory)}
          <button
            class="memory-card"
            type="button"
            data-memory-id={memory.id}
            disabled={!onOpenMemory}
            aria-label={$_('home.album.openMemory', {
              values: {
                title,
                date,
                pets: memory.petIds.join(', '),
              },
            })}
            onclick={() => onOpenMemory?.(memory.id)}
          >
            <span class="memory-art" aria-hidden="true">
              {title.trim().slice(0, 1).toLocaleUpperCase()}
            </span>
            <span class="memory-copy">
              <strong>{title}</strong>
              <span>{date}</span>
            </span>
            <span class="pet-marks" aria-label={$_('home.album.participants')}>
              {#each memory.petIds as petId (petId)}
                <span class="pet-mark" data-memory-pet={petId} title={petId}>
                  {petMark(petId)}
                </span>
              {/each}
            </span>
          </button>
        {/each}
      </div>
    {/if}
  {:else}
    <div class="plaza-future" data-plaza-future>
      <span class="plaza-mark" aria-hidden="true">⌁</span>
      <h3>
        <span>{$_('home.plaza.soon')}</span>
        {$_('home.plaza.comingLater')}
      </h3>
      <p>{$_('home.plaza.body')}</p>
    </div>
  {/if}
</HomePanelShell>

<style>
  .memory-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }

  .memory-card {
    display: grid;
    min-width: 0;
    gap: 9px;
    padding: 9px;
    border: 1px solid var(--home-border);
    border-radius: 14px;
    background: var(--home-canvas);
    color: var(--home-text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .memory-card:hover:not(:disabled) {
    border-color: var(--home-action);
    background: var(--home-subtle);
  }

  .memory-card:active:not(:disabled) {
    transform: translateY(1px);
  }

  .memory-card:focus-visible {
    outline: 2px solid var(--home-focus);
    outline-offset: 2px;
  }

  .memory-card:disabled {
    cursor: not-allowed;
    opacity: 0.66;
  }

  .memory-art {
    display: grid;
    min-height: 82px;
    place-items: center;
    border-radius: 10px;
    background: color-mix(in srgb, var(--home-action) 15%, var(--home-subtle));
    color: var(--home-action);
    font-family: 'M PLUS Rounded 1c', 'Noto Sans SC', 'Segoe UI', sans-serif;
    font-size: 24px;
    font-weight: 800;
  }

  .memory-card:nth-child(4n + 2) .memory-art {
    background: color-mix(in srgb, #d1766f 15%, var(--home-subtle));
    color: color-mix(in srgb, #a84e47 78%, var(--home-text));
  }

  .memory-card:nth-child(4n + 3) .memory-art {
    background: color-mix(in srgb, #6c997f 16%, var(--home-subtle));
    color: color-mix(in srgb, #47775e 78%, var(--home-text));
  }

  .memory-card:nth-child(4n + 4) .memory-art {
    background: color-mix(in srgb, #b27b46 16%, var(--home-subtle));
    color: color-mix(in srgb, #80532b 78%, var(--home-text));
  }

  .memory-copy {
    display: grid;
    min-width: 0;
    gap: 2px;
  }

  .memory-copy strong,
  .memory-copy span {
    overflow: hidden;
    font-size: 12px;
    line-height: 1.35;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .memory-copy strong {
    color: var(--home-text);
  }

  .memory-copy span {
    color: var(--home-text-muted);
  }

  .pet-marks {
    display: flex;
    min-height: 24px;
    align-items: center;
    padding-left: 3px;
  }

  .pet-mark {
    display: grid;
    width: 24px;
    height: 24px;
    place-items: center;
    margin-left: -3px;
    border: 2px solid var(--home-canvas);
    border-radius: 50%;
    background: var(--home-subtle);
    color: var(--home-action);
    font-size: 12px;
    font-weight: 800;
    line-height: 1;
  }

  .album-empty,
  .plaza-future {
    display: grid;
    min-height: 100%;
    place-content: center;
    justify-items: center;
    gap: 8px;
    text-align: center;
  }

  .empty-mark,
  .plaza-mark {
    display: grid;
    width: 68px;
    height: 54px;
    place-items: center;
    margin-bottom: 3px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--home-action) 12%, var(--home-subtle));
    color: var(--home-action);
    font-size: 30px;
    transform: rotate(-7deg);
  }

  .album-empty strong,
  .plaza-future h3 {
    margin: 0;
    color: var(--home-text);
    font-family: 'M PLUS Rounded 1c', 'Noto Sans SC', 'Segoe UI', sans-serif;
    font-size: 16px;
    line-height: 1.3;
    text-wrap: balance;
  }

  .album-empty p,
  .plaza-future p {
    max-width: 30ch;
    margin: 0;
    color: var(--home-text-muted);
    font-size: 12px;
    line-height: 1.5;
    text-wrap: pretty;
  }

  .plaza-future h3 {
    display: grid;
    gap: 2px;
  }

  .plaza-future h3 span {
    color: var(--home-action);
    font-size: 12px;
    font-weight: 750;
    line-height: 1.3;
  }

  @media (prefers-reduced-motion: reduce) {
    .memory-card:active:not(:disabled) {
      transform: none;
    }
  }
</style>
