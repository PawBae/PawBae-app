<script lang="ts">
  import { _, locale } from 'svelte-i18n';
  import { type HomeEvent, memoryCardCopy } from '../../utils/social-home';

  let {
    event,
    petName,
    onAcceptVisit,
    onDelayVisit,
    onInviteFriend,
    onOpenMemory,
  }: {
    event: HomeEvent | null;
    petName: string;
    onAcceptVisit?: (id: string) => void;
    onDelayVisit?: (id: string) => void;
    onInviteFriend?: (id: string) => void;
    onOpenMemory?: (id: string) => void;
  } = $props();

  // v1 契约在接受串门前不暴露对方宠物身份（pet.name 为空）：标题走未知宠物
  // 文案，角标回落主人名首字母。
  const visitTitleKey = $derived(
    event?.kind === 'visit-request' && event.request.pet.name === ''
      ? 'home.visit.cardTitleUnknownPet'
      : 'home.visit.cardTitle',
  );
</script>

{#if event?.kind === 'visit-request'}
  {#key event.request.id}
    <span
      class="visually-hidden"
      data-visit-announcement={event.request.id}
      aria-live="polite"
    >
      {$_(visitTitleKey, {
        values: { pet: event.request.pet.name, owner: event.request.ownerName },
      })}
    </span>
  {/key}
  <aside class="event-card has-actions" data-home-event="visit-request">
    <span class="event-mark" aria-hidden="true">
      {(event.request.pet.name || event.request.ownerName).slice(0, 1).toLocaleUpperCase()}
    </span>
    <span>
      <strong>
        {$_(visitTitleKey, {
          values: { pet: event.request.pet.name, owner: event.request.ownerName },
        })}
      </strong>
      <small>{$_('home.visit.cardBody')}</small>
    </span>
    <span class="event-actions">
      <button
        type="button"
        data-action="delay-visit"
        disabled={!onDelayVisit}
        onclick={() => onDelayVisit?.(event.request.id)}
      >{$_('home.visit.later')}</button>
      <button
        class="primary"
        type="button"
        data-action="accept-visit"
        disabled={!onAcceptVisit}
        onclick={() => onAcceptVisit?.(event.request.id)}
      >{$_('home.visit.accept')}</button>
    </span>
  </aside>
{:else if event?.kind === 'memory-ready'}
  {@const memoryTitle = memoryCardCopy(event.memory, $locale).title}
  {@const memoryDate = new Intl.DateTimeFormat($locale ?? 'en', {
    month: 'short',
    day: 'numeric',
    timeZone: 'UTC',
  }).format(event.memory.occurredAt)}
  <aside class="event-card has-actions" data-home-event="memory-ready">
    <span class="event-mark" aria-hidden="true">
      {memoryTitle.slice(0, 1).toLocaleUpperCase()}
    </span>
    <span>
      <strong>{$_('home.memory.readyTitle')}</strong>
      <small>
        {$_('home.memory.readyBody', {
          values: { title: memoryTitle, date: memoryDate },
        })}
      </small>
    </span>
    <span class="event-actions">
      <button
        class="primary"
        type="button"
        data-action="open-memory"
        disabled={!onOpenMemory}
        onclick={() => onOpenMemory?.(event.memory.id)}
      >{$_('home.memory.open')}</button>
    </span>
  </aside>
{:else if event?.kind === 'invite-friend'}
  <aside class="event-card" data-home-event="invite-friend">
    <span class="event-mark" aria-hidden="true">+</span>
    <span>
      <strong>{$_('home.empty.title', { values: { pet: petName } })}</strong>
      <small>{$_('home.empty.body')}</small>
    </span>
  </aside>
{/if}

<style>
  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
    border: 0;
  }
  .event-card {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr);
    align-items: center;
    gap: 10px;
    width: min(430px, calc(100% - 176px));
    min-height: 58px;
    margin: 0 auto 10px;
    padding: 9px 14px 9px 10px;
    border: 1px solid var(--home-border);
    border-radius: 20px;
    background: var(--home-surface);
    color: var(--home-text);
  }

  .event-card.has-actions {
    grid-template-columns: 34px minmax(0, 1fr) auto;
    width: min(580px, calc(100% - 176px));
  }

  .event-mark {
    display: grid;
    width: 34px;
    height: 34px;
    place-items: center;
    border-radius: 50%;
    background: var(--home-subtle);
    color: var(--home-action);
    font-size: 19px;
    font-weight: 500;
  }

  .event-card > span:last-child {
    display: grid;
    gap: 2px;
  }

  strong {
    font-size: 13px;
    line-height: 1.3;
  }

  small {
    color: var(--home-text-muted);
    font-size: 12px;
    line-height: 1.35;
  }

  .event-actions {
    display: flex;
    gap: 6px;
  }

  button {
    min-height: 34px;
    padding: 0 10px;
    border: 1px solid var(--home-border);
    border-radius: 10px;
    background: var(--home-canvas);
    color: var(--home-text);
    font: inherit;
    font-size: 12px;
    font-weight: 700;
    white-space: nowrap;
    cursor: pointer;
  }

  button:hover:not(:disabled) {
    border-color: var(--home-action);
    background: var(--home-subtle);
  }

  button.primary {
    border-color: transparent;
    background: var(--home-action);
    color: var(--home-action-text);
  }

  button:focus-visible {
    outline: 2px solid var(--home-focus);
    outline-offset: 2px;
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.48;
  }
</style>
