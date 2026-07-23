<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { FriendEntry, PublicProfile } from '../../platform/types';
  import {
    type FriendContextAction,
    type FriendSummary,
    type HomePresence,
    selectFriendAction,
    type VisitRequest,
  } from '../../utils/social-home';
  import HomePanelShell from './HomePanelShell.svelte';

  let {
    friends,
    presence,
    pendingVisit,
    onClose,
    onInviteFriend,
    onVisitFriend,
    onRecallFriend,
    onAcceptVisit,
    onDelayVisit,
    relationships = [],
    inviteRedeemed = false,
    onFindFriend,
    onSendFriendRequest,
    onAcceptFriend,
    onRemoveFriend,
    onMuteFriend,
    onBlockFriend,
    onRedeemInvite,
    onDeclineVisit,
  }: {
    friends: FriendSummary[];
    presence: HomePresence;
    pendingVisit: VisitRequest | null;
    onClose: () => void;
    onInviteFriend?: (id: string) => void;
    onVisitFriend?: (id: string) => void;
    onRecallFriend?: (id: string) => void;
    onAcceptVisit?: (id: string) => void;
    onDelayVisit?: (id: string) => void;
    relationships?: FriendEntry[];
    inviteRedeemed?: boolean;
    onFindFriend?: (handle: string) => Promise<PublicProfile | null>;
    onSendFriendRequest?: (id: string) => Promise<void>;
    onAcceptFriend?: (id: string) => Promise<void>;
    onRemoveFriend?: (id: string) => Promise<void>;
    onMuteFriend?: (id: string, muted: boolean) => Promise<void>;
    onBlockFriend?: (id: string) => Promise<void>;
    onRedeemInvite?: (code: string) => Promise<void>;
    onDeclineVisit?: (id: string) => void;
  } = $props();

  let handleQuery = $state('');
  let inviteCode = $state('');
  let searchResult = $state<PublicProfile | null>(null);
  let actionError = $state('');
  let busyKey = $state('');
  const incoming = $derived(relationships.filter((entry) => entry.relation === 'pending_in'));
  const outgoing = $derived(relationships.filter((entry) => entry.relation === 'pending_out'));

  async function run(key: string, action: () => Promise<void>) {
    if (busyKey) return;
    busyKey = key;
    actionError = '';
    try {
      await action();
    } catch (error) {
      actionError = error instanceof Error ? error.message : String(error);
    } finally {
      busyKey = '';
    }
  }

  async function findFriend(event: SubmitEvent) {
    event.preventDefault();
    if (!onFindFriend || !handleQuery.trim()) return;
    await run('search', async () => {
      searchResult = await onFindFriend(handleQuery);
      if (!searchResult) actionError = $_('home.friends.searchNotFound');
    });
  }

  function confirmBlock(entry: FriendEntry) {
    if (!onBlockFriend) return;
    if (!globalThis.confirm($_('home.friends.confirmBlock', { values: { handle: entry.handle } }))) {
      return;
    }
    void run(`block:${entry.userId}`, () => onBlockFriend(entry.userId));
  }

  function callbackAvailable(action: FriendContextAction) {
    if (action.kind === 'visit') return Boolean(onVisitFriend);
    if (action.kind === 'invite') return Boolean(onInviteFriend);
    return Boolean(onRecallFriend);
  }

  function runFriendAction(action: FriendContextAction, friendId: string) {
    if (action.disabledReason) return;
    if (action.kind === 'visit') onVisitFriend?.(friendId);
    else if (action.kind === 'invite') onInviteFriend?.(friendId);
    else onRecallFriend?.(friendId);
  }

</script>

<HomePanelShell
  panel="friends"
  title={$_('home.friends.title')}
  subtitle={$_('home.friends.subtitle')}
  {onClose}
>
    {#if pendingVisit}
      <section class="requests" aria-labelledby="visit-requests-title">
        <h3 id="visit-requests-title">{$_('home.visit.requestsTitle')}</h3>
        <article class="visit-request" data-visit-request={pendingVisit.id}>
          <span class="pet-initial" aria-hidden="true">
            {pendingVisit.pet.name.slice(0, 1).toLocaleUpperCase()}
          </span>
          <div class="request-copy">
            <strong>{pendingVisit.pet.name}</strong>
            <span>
              {$_('home.visit.requestedBy', { values: { owner: pendingVisit.ownerName } })}
            </span>
          </div>
          <div class="request-actions">
            <button
              type="button"
              data-action="delay-visit"
              disabled={!onDelayVisit}
              onclick={() => onDelayVisit?.(pendingVisit.id)}
            >{$_('home.visit.later')}</button>
            <button
              type="button"
              data-action="decline-visit"
              disabled={!onDeclineVisit}
              onclick={() => onDeclineVisit?.(pendingVisit.id)}
            >{$_('home.visit.decline')}</button>
            <button
              class="primary"
              type="button"
              data-action="accept-visit"
              disabled={!onAcceptVisit}
              onclick={() => onAcceptVisit?.(pendingVisit.id)}
            >{$_('home.visit.accept')}</button>
          </div>
        </article>
      </section>
    {/if}

    {#if incoming.length > 0}
      <section class="relationship-queue" aria-labelledby="incoming-friends-title">
        <h3 id="incoming-friends-title">{$_('home.friends.incomingTitle')}</h3>
        <ul>
          {#each incoming as entry (entry.userId)}
            <li data-relationship="pending-in">
              <span class="pet-initial" aria-hidden="true">@</span>
              <div class="friend-copy">
                <strong>{entry.displayName ?? entry.handle}</strong>
                <span>@{entry.handle}</span>
              </div>
              <div class="request-actions">
                <button
                  type="button"
                  disabled={!onAcceptFriend || Boolean(busyKey)}
                  onclick={() =>
                    void run(`accept:${entry.userId}`, () => onAcceptFriend?.(entry.userId) ?? Promise.resolve())}
                >{$_('home.friends.accept')}</button>
                <button type="button" disabled={!onBlockFriend || Boolean(busyKey)} onclick={() => confirmBlock(entry)}>
                  {$_('home.friends.block')}
                </button>
              </div>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if outgoing.length > 0}
      <section class="relationship-queue" aria-labelledby="outgoing-friends-title">
        <h3 id="outgoing-friends-title">{$_('home.friends.outgoingTitle')}</h3>
        <ul>
          {#each outgoing as entry (entry.userId)}
            <li data-relationship="pending-out">
              <span class="pet-initial" aria-hidden="true">→</span>
              <div class="friend-copy">
                <strong>{entry.displayName ?? entry.handle}</strong>
                <span>@{entry.handle}</span>
              </div>
              <button
                type="button"
                disabled={!onRemoveFriend || Boolean(busyKey)}
                onclick={() =>
                  void run(`remove:${entry.userId}`, () => onRemoveFriend?.(entry.userId) ?? Promise.resolve())}
              >{$_('home.friends.cancelRequest')}</button>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <section class="mutual-friends" aria-labelledby="mutual-friends-title">
      <h3 id="mutual-friends-title">{$_('home.friends.mutualTitle')}</h3>
      {#if friends.length === 0}
        <div class="friends-empty" data-friends-empty>
          <strong>{$_('home.friends.emptyTitle')}</strong>
          <p>{$_('home.friends.emptyBody')}</p>
        </div>
      {:else}
        <ul>
          {#each friends as friend (friend.id)}
            {@const action = selectFriendAction(presence, friend, pendingVisit)}
            {@const reasonKey = action.disabledReason ?? (!callbackAvailable(action) ? 'unavailable' : null)}
            {@const reasonId = `friend-action-reason-${friend.id}`}
            {@const relationship = relationships.find((entry) => entry.userId === friend.id)}
            <li data-friend={friend.id}>
              <span class="pet-initial" aria-hidden="true">
                {friend.pet.name.slice(0, 1).toLocaleUpperCase()}
              </span>
              <div class="friend-copy">
                <strong>{friend.displayName}</strong>
                <span>{friend.handle} · {friend.pet.name}</span>
                <small>
                  {$_(`home.friends.availability.${friend.availability}`)} ·
                  {$_(`home.friends.publicStatus.${friend.publicAgentState}`)}
                </small>
              </div>
              <div class="friend-action">
                <button
                  type="button"
                  data-friend-action={action.kind}
                  disabled={Boolean(reasonKey)}
                  aria-describedby={reasonKey ? reasonId : undefined}
                  onclick={() => runFriendAction(action, friend.id)}
                >{$_(`home.friends.actions.${action.kind}`)}</button>
                {#if reasonKey}
                  <small id={reasonId} data-friend-action-reason>
                    {$_(`home.friends.disabled.${reasonKey}`)}
                  </small>
                {/if}
                {#if relationship}
                  <div class="manage-actions">
                    <button
                      type="button"
                      disabled={!onMuteFriend || Boolean(busyKey)}
                      onclick={() =>
                        void run(`mute:${relationship.userId}`, () =>
                          onMuteFriend?.(relationship.userId, !relationship.muted) ?? Promise.resolve())}
                    >
                      {relationship.muted ? $_('home.friends.unmute') : $_('home.friends.mute')}
                    </button>
                    <button
                      type="button"
                      disabled={!onRemoveFriend || Boolean(busyKey)}
                      onclick={() =>
                        void run(`remove:${relationship.userId}`, () =>
                          onRemoveFriend?.(relationship.userId) ?? Promise.resolve())}
                    >{$_('home.friends.remove')}</button>
                    <button type="button" disabled={!onBlockFriend || Boolean(busyKey)} onclick={() => confirmBlock(relationship)}>
                      {$_('home.friends.block')}
                    </button>
                  </div>
                {/if}
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section class="account-tools" aria-labelledby="friends-find-title">
      <h3 id="friends-find-title">{$_('home.friends.findTitle')}</h3>
      <form onsubmit={findFriend}>
        <label for="friend-handle">{$_('home.friends.searchLabel')}</label>
        <div class="search-row">
        <input
          id="friend-handle"
          data-friend-search
          type="text"
          placeholder={$_('home.friends.searchPlaceholder')}
          bind:value={handleQuery}
          disabled={!inviteRedeemed || !onFindFriend || Boolean(busyKey)}
        />
          <button type="submit" disabled={!inviteRedeemed || !onFindFriend || !handleQuery.trim() || Boolean(busyKey)}>
            {$_('home.friends.searchAction')}
          </button>
        </div>
      </form>
      {#if searchResult}
        <article class="search-result">
          <div>
            <strong>{searchResult.displayName ?? searchResult.handle}</strong>
            <span>@{searchResult.handle}</span>
          </div>
          <button
            type="button"
            disabled={!onSendFriendRequest || Boolean(busyKey)}
            onclick={() =>
              void run(`send:${searchResult?.userId}`, async () => {
                if (!searchResult || !onSendFriendRequest) return;
                await onSendFriendRequest(searchResult.userId);
                searchResult = null;
                handleQuery = '';
              })}
          >{$_('home.friends.sendRequest')}</button>
        </article>
      {/if}
      {#if !inviteRedeemed}
        <label for="invite-code">{$_('home.friends.inviteCodeLabel')}</label>
        <div class="search-row">
          <input
            id="invite-code"
            type="text"
            autocomplete="one-time-code"
            placeholder="PAW-XXXXXXXXXX"
            bind:value={inviteCode}
            disabled={!onRedeemInvite || Boolean(busyKey)}
          />
          <button
            type="button"
            disabled={!onRedeemInvite || !inviteCode.trim() || Boolean(busyKey)}
            onclick={() =>
              void run('redeem', async () => {
                if (!onRedeemInvite) return;
                await onRedeemInvite(inviteCode);
                inviteCode = '';
              })}
          >{$_('home.friends.redeemInvite')}</button>
        </div>
      {/if}
      {#if actionError}<p class="action-error" role="alert">{actionError}</p>{/if}
      <p class="beta-note">{inviteRedeemed ? $_('home.friends.betaReady') : $_('home.friends.betaNote')}</p>
    </section>
</HomePanelShell>

<style>
  h3,
  p {
    margin: 0;
  }

  button,
  input {
    font: inherit;
  }

  button {
    min-height: 34px;
    border: 1px solid var(--home-border);
    border-radius: 10px;
    background: var(--home-canvas);
    color: var(--home-text);
    font-size: 12px;
    font-weight: 700;
    white-space: nowrap;
    cursor: pointer;
  }

  button:hover:not(:disabled) {
    border-color: var(--home-action);
    background: var(--home-subtle);
  }

  button:focus-visible,
  input:focus-visible {
    outline: 2px solid var(--home-focus);
    outline-offset: 2px;
  }

  button:disabled,
  input:disabled {
    cursor: not-allowed;
    opacity: 0.56;
  }

  button.primary {
    border-color: transparent;
    background: var(--home-action);
    color: var(--home-action-text);
  }

  section + section {
    margin-top: 18px;
  }

  h3 {
    margin-bottom: 9px;
    color: var(--home-text);
    font-size: 12px;
    line-height: 1.35;
  }

  label {
    display: block;
    margin-bottom: 5px;
    color: var(--home-text-muted);
    font-size: 12px;
    font-weight: 650;
  }

  .search-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 7px;
  }

  input {
    min-width: 0;
    min-height: 36px;
    padding: 0 10px;
    border: 1px solid var(--home-border);
    border-radius: 10px;
    background: var(--home-canvas);
    color: var(--home-text);
    font-size: 12px;
  }

  input::placeholder {
    color: var(--home-text-muted);
  }

  .search-row button {
    padding: 0 10px;
  }

  .beta-note,
  .friends-empty p {
    margin-top: 6px;
    color: var(--home-text-muted);
    font-size: 12px;
    line-height: 1.45;
  }

  .action-error {
    margin-top: 8px;
    color: #b42318;
    font-size: 12px;
  }

  .search-result {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 8px;
    padding: 9px 10px;
    border-radius: 10px;
    background: var(--home-subtle);
  }

  .search-result > div {
    display: grid;
    gap: 1px;
  }

  .search-result span {
    color: var(--home-text-muted);
    font-size: 12px;
  }

  .visit-request,
  li {
    display: grid;
    grid-template-columns: 38px minmax(0, 1fr) auto;
    align-items: center;
    gap: 9px;
    padding: 10px;
    border-radius: 12px;
    background: var(--home-subtle);
  }

  .pet-initial {
    display: grid;
    width: 38px;
    height: 38px;
    place-items: center;
    border-radius: 50%;
    background: var(--home-surface);
    color: var(--home-action);
    font-size: 13px;
    font-weight: 800;
  }

  .request-copy,
  .friend-copy {
    display: grid;
    min-width: 0;
    gap: 2px;
  }

  .request-copy strong,
  .friend-copy strong,
  .friends-empty strong {
    overflow: hidden;
    color: var(--home-text);
    font-size: 12px;
    line-height: 1.3;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .request-copy span,
  .friend-copy span,
  .friend-copy small {
    overflow: hidden;
    color: var(--home-text-muted);
    font-size: 12px;
    line-height: 1.35;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .friend-copy small {
    color: var(--home-action);
  }

  .request-actions {
    display: flex;
    gap: 5px;
  }

  .request-actions button,
  .friend-action button {
    min-height: 32px;
    padding: 0 9px;
  }

  .friend-action {
    display: grid;
    max-width: 112px;
    justify-items: end;
    gap: 4px;
  }

  .manage-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 3px;
  }

  .manage-actions button {
    min-height: 24px;
    padding: 0 5px;
    font-size: 12px;
  }

  .friend-action small {
    color: var(--home-text-muted);
    font-size: 12px;
    line-height: 1.25;
    text-align: right;
    white-space: normal;
  }

  ul {
    display: grid;
    gap: 7px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .friends-empty {
    padding: 14px;
    border-radius: 12px;
    background: var(--home-subtle);
  }

</style>
