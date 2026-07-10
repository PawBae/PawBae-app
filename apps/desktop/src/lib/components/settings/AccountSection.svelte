<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { accountStore } from '../../stores/account.svelte';
  import { settingsStore } from '../../stores/settings.svelte';

  // opt-in 开关组全部默认关（交接文档 W3 红线）。主开关关闭时分项禁用：
  // 「连接你的 agent」= 心跳/投影/事件的总闸，关 = 数据不出本机。
  const masterOn = $derived(settingsStore.platformConnectEnabled);
</script>

<section class="section">
  <h2>{$_('settings.account.title')}</h2>
  <div class="card">
    {#if accountStore.phase === 'unconfigured'}
      <div class="setting-row">
        <div class="setting-info">
          <span class="setting-label">{$_('settings.account.unconfigured')}</span>
          <span class="setting-desc">{$_('settings.account.unconfiguredDesc')}</span>
        </div>
      </div>
    {:else if accountStore.phase !== 'signedIn'}
      <div class="setting-row">
        <div class="setting-info">
          <span class="setting-label">{$_('settings.account.signInLabel')}</span>
          <span class="setting-desc">{$_('settings.account.signInDesc')}</span>
          {#if accountStore.error}
            <span class="hint-warn">{accountStore.error}</span>
          {/if}
        </div>
        <button
          class="gh-signin"
          disabled={accountStore.phase === 'signingIn'}
          onclick={() => accountStore.login()}
        >
          {accountStore.phase === 'signingIn'
            ? $_('settings.account.signingIn')
            : $_('settings.account.signIn')}
        </button>
      </div>
    {:else}
      <div class="setting-row border-bottom">
        <div class="setting-info account-identity">
          {#if accountStore.session?.avatarUrl}
            <img class="avatar" src={accountStore.session.avatarUrl} alt="" />
          {/if}
          <div class="identity-text">
            <span class="setting-label">@{accountStore.session?.handle}</span>
            {#if accountStore.session?.displayName}
              <span class="setting-desc">{accountStore.session.displayName}</span>
            {/if}
          </div>
        </div>
        <button class="signout" onclick={() => accountStore.logout()}>
          {$_('settings.account.signOut')}
        </button>
      </div>

      <div class="setting-row">
        <div class="setting-info">
          <span class="setting-label">{$_('settings.account.connect')}</span>
          <span class="setting-desc">{$_('settings.account.connectDesc')}</span>
        </div>
        <button
          class="toggle"
          class:on={masterOn}
          role="switch"
          aria-label={$_('settings.account.connect')}
          aria-checked={masterOn}
          onclick={() => settingsStore.setPlatformConnectEnabled(!masterOn)}
        >
          <span class="toggle-thumb"></span>
        </button>
      </div>

      <div class="setting-row sub" class:dimmed={!masterOn}>
        <div class="setting-info">
          <span class="setting-label">{$_('settings.account.uploadRewards')}</span>
        </div>
        <button
          class="toggle"
          class:on={settingsStore.uploadRewardsEnabled}
          role="switch"
          aria-label={$_('settings.account.uploadRewards')}
          aria-checked={settingsStore.uploadRewardsEnabled}
          disabled={!masterOn}
          onclick={() => settingsStore.setUploadRewardsEnabled(!settingsStore.uploadRewardsEnabled)}
        >
          <span class="toggle-thumb"></span>
        </button>
      </div>
      <div class="setting-row sub" class:dimmed={!masterOn}>
        <div class="setting-info">
          <span class="setting-label">{$_('settings.account.uploadEggs')}</span>
        </div>
        <button
          class="toggle"
          class:on={settingsStore.uploadEggsEnabled}
          role="switch"
          aria-label={$_('settings.account.uploadEggs')}
          aria-checked={settingsStore.uploadEggsEnabled}
          disabled={!masterOn}
          onclick={() => settingsStore.setUploadEggsEnabled(!settingsStore.uploadEggsEnabled)}
        >
          <span class="toggle-thumb"></span>
        </button>
      </div>
      <div class="setting-row sub" class:dimmed={!masterOn}>
        <div class="setting-info">
          <span class="setting-label">{$_('settings.account.uploadSouvenirs')}</span>
        </div>
        <button
          class="toggle"
          class:on={settingsStore.uploadSouvenirsEnabled}
          role="switch"
          aria-label={$_('settings.account.uploadSouvenirs')}
          aria-checked={settingsStore.uploadSouvenirsEnabled}
          disabled={!masterOn}
          onclick={() =>
            settingsStore.setUploadSouvenirsEnabled(!settingsStore.uploadSouvenirsEnabled)}
        >
          <span class="toggle-thumb"></span>
        </button>
      </div>
      <div class="setting-row sub" class:dimmed={!masterOn}>
        <div class="setting-info">
          <span class="setting-label">{$_('settings.account.uploadStreaks')}</span>
        </div>
        <button
          class="toggle"
          class:on={settingsStore.uploadStreaksEnabled}
          role="switch"
          aria-label={$_('settings.account.uploadStreaks')}
          aria-checked={settingsStore.uploadStreaksEnabled}
          disabled={!masterOn}
          onclick={() => settingsStore.setUploadStreaksEnabled(!settingsStore.uploadStreaksEnabled)}
        >
          <span class="toggle-thumb"></span>
        </button>
      </div>
    {/if}
  </div>
</section>

<style>
  .hint-warn {
    font-size: 11px;
    color: #fbbf24;
    margin-top: 2px;
  }

  .account-identity {
    flex-direction: row;
    align-items: center;
    gap: 10px;
  }

  .avatar {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .identity-text {
    display: flex;
    flex-direction: column;
  }

  .gh-signin,
  .signout {
    border: 1px solid rgba(255, 255, 255, 0.18);
    background: rgba(255, 255, 255, 0.08);
    color: inherit;
    border-radius: 8px;
    padding: 6px 12px;
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
  }

  .gh-signin:hover:not(:disabled),
  .signout:hover {
    background: rgba(255, 255, 255, 0.14);
  }

  .gh-signin:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .sub {
    padding-left: 14px;
  }

  .dimmed {
    opacity: 0.45;
  }
</style>
