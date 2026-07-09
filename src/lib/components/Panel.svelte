<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { agentStore } from '../stores/agents.svelte';
  import { petStore } from '../stores/pet.svelte';
  import { sessionStore } from '../stores/sessions.svelte';
  import { settingsStore } from '../stores/settings.svelte';
  import { windowStore } from '../stores/window.svelte';
  import { ACHIEVEMENTS } from '../utils/achievements';
  import { loadCodexPets, loadDefaultCodexPet } from '../utils/codex-pet';
  import { BOARD_TASKS } from '../utils/daily-board';
  import { tryInvoke } from '../utils/invoke';
  import { effectiveName } from '../utils/pet-name';
  import { FEED_COST_COINS } from '../utils/rewards';
  import ProfileCard from './ProfileCard.svelte';
  import ShareCardModal from './ShareCardModal.svelte';

  let {
    class: className = '',
  }: {
    class?: string;
  } = $props();

  const maxHeight = $derived(settingsStore.panelMaxHeight);

  const evo = $derived(petStore.evolution);
  const unlockedCount = $derived(
    ACHIEVEMENTS.filter((d) => petStore.achievements[d.id] !== undefined).length,
  );

  function achievementTitle(id: string, locked: boolean, secret: boolean | undefined): string {
    if (locked && secret) return '???';
    return `${$_(`growth.ach.${id}`)} — ${$_(`growth.achDesc.${id}`)}`;
  }

  let shareOpen = $state(false);
  let profileOpen = $state(false);

  // Official name of the active pet (sprite-pack displayName). The async write
  // lands after the await — untracked, so no effect self-retrigger.
  let officialName = $state('PawBae');
  $effect(() => {
    if (settingsStore.appMode === 'coding') return;
    const wanted = settingsStore.miniPetId;
    void loadCodexPets().then(async (pets) => {
      const p = pets.find((x) => x.id === wanted) ?? (await loadDefaultCodexPet());
      officialName = p?.displayName ?? 'PawBae';
    });
  });
  const petName = $derived(
    effectiveName(settingsStore.petNicknames[settingsStore.miniPetId], officialName),
  );

  // In-app settings entry: the tray icon can be hidden by the MacBook notch,
  // so the panel needs its own way in. Mirrors Main.svelte's openSettings().
  async function openSettings() {
    if (windowStore.settingsOpen) return;
    windowStore.setSettingsOpen(true);
    await tryInvoke('set_mini_size', { restore: false });
  }
</script>

{#if windowStore.expanded}
  <div
    class="panel {className}"
    style="max-height: {maxHeight}px;"
  >
    <div class="panel-content">
      <div class="panel-topbar">
        <button
          class="settings-btn"
          title={$_('mini.settings')}
          aria-label={$_('mini.settings')}
          onclick={openSettings}
        >
          ⚙️
        </button>
      </div>
      {#if settingsStore.appMode === 'coding'}
        <div class="session-list">
          {#if sessionStore.claudeSessions.length === 0 && agentStore.agents.length === 0}
            <div class="empty">
              <p>No active sessions</p>
              <p class="hint">Start Claude Code, Codex, or Cursor to see sessions here</p>
            </div>
          {:else}
            {#each sessionStore.claudeSessions as session (session.sessionId)}
              <button
                class="session-card"
                class:active={session.status === 'active'}
                onclick={() => sessionStore.selectSession(session.sessionId)}
              >
                <div class="session-header">
                  <span class="session-source">{session.source || 'cc'}</span>
                  <span class="dot" class:active={session.status === 'active'}></span>
                </div>
                <div class="session-cwd">{session.cwd || session.sessionId}</div>
                {#if session.model}
                  <div class="session-model">{session.model}</div>
                {/if}
              </button>
            {/each}

            {#each agentStore.agents as agent (agent.id)}
              <button
                class="session-card"
                class:active={agentStore.healthMap[agent.id]}
                onclick={() => agentStore.selectAgent(agent.id)}
              >
                <div class="session-header">
                  <span class="session-source">
                    {agent.identityEmoji || '🤖'} {agent.identityName || agent.id}
                  </span>
                  <span class="dot" class:active={agentStore.healthMap[agent.id]}></span>
                </div>
              </button>
            {/each}
          {/if}
        </div>
      {:else}
        <div class="pet-panel">
          <button class="name-row" onclick={() => (profileOpen = true)}>
            🐾 <span class="name-text">{petName}</span><span class="name-chev">›</span>
          </button>

          <div class="growth-header">
            <span class="stage-emoji">{evo.stage.emoji}</span>
            <div class="growth-text">
              <div class="stage-line">
                <span class="stage-name">{$_(`growth.stage.${evo.stage.id}`)}</span>
                {#if evo.style}
                  <span class="style-tag style-{evo.style}">{$_(`growth.style.${evo.style}`)}</span>
                {/if}
              </div>
              <div class="days-line">
                {$_('growth.daysTogether', { values: { days: petStore.daysTogether + 1 } })}
              </div>
            </div>
          </div>

          {#if evo.next}
            <div class="xp-bar">
              <div class="xp-fill" style="width: {Math.round(evo.progress * 100)}%"></div>
            </div>
            <div class="xp-label">{evo.xp} / {evo.next.minXp} XP → {evo.next.emoji}</div>
          {:else}
            <div class="xp-label">👑 {$_('growth.maxStage')} · {evo.xp} XP</div>
          {/if}

          <p class="pet-status">
            ❤️ {Math.round(petStore.petData.affection)} &nbsp;
            🍗 {Math.round(petStore.petData.hunger)} &nbsp;
            🪙 {petStore.petData.coins}
          </p>

          <div class="btn-row">
            <button
              class="action-btn"
              disabled={!petStore.canFeed}
              onclick={() => petStore.applyFeed()}
            >
              🍖 {$_('pet.feed')} (-{FEED_COST_COINS})
            </button>
            <button
              class="action-btn gift"
              disabled={!petStore.canClaimDailyGift}
              onclick={() => petStore.claimDailyGift()}
            >
              {#if petStore.canClaimDailyGift}
                🎁 {$_('pet.dailyGift')} +{petStore.nextGiftAmount}
              {:else}
                ✓ {$_('pet.claimed')}
              {/if}
            </button>
            <button class="action-btn" onclick={() => (shareOpen = true)}>
              📸 {$_('share.button')}
            </button>
          </div>

          <ShareCardModal open={shareOpen} onclose={() => (shareOpen = false)} />
          <ProfileCard open={profileOpen} onclose={() => (profileOpen = false)} />

          <div class="board-section">
            <div class="board-title">
              <span>📋 {$_('board.title')}</span>
              <span class="board-badges">
                {#if petStore.streakLive >= 2}
                  <span class="streak" title={$_('board.streakHint')}>
                    🔥 {$_('growth.streak', { values: { days: petStore.streakLive } })}
                  </span>
                {/if}
                {#if petStore.petData.shields > 0}
                  <span class="shields" title={$_('board.shieldHint')}>
                    {'🛡️'.repeat(petStore.petData.shields)}
                  </span>
                {/if}
              </span>
            </div>
            {#each BOARD_TASKS as t (t.id)}
              {@const done = petStore.boardDoneToday.includes(t.id)}
              <div class="board-row" class:done>
                <span aria-hidden="true">{done ? '✅' : '⬜'}</span>
                <span>{t.emoji} {$_(`board.task.${t.id}`)}</span>
              </div>
            {/each}
          </div>

          <div class="ach-section">
            <div class="ach-title">
              🏆 {$_('growth.achievements')}
              <span class="ach-count">{unlockedCount}/{ACHIEVEMENTS.length}</span>
            </div>
            <div class="ach-grid">
              {#each ACHIEVEMENTS as def (def.id)}
                {@const locked = petStore.achievements[def.id] === undefined}
                <div
                  class="ach-tile"
                  class:locked
                  title={achievementTitle(def.id, locked, def.secret)}
                >
                  {locked && def.secret ? '❓' : def.emoji}
                </div>
              {/each}
            </div>
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .panel {
    background: rgba(26, 26, 32, 0.95);
    backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 12px;
    overflow-y: auto;
    overflow-x: hidden;
    width: 100%;
  }

  .panel-content {
    padding: 8px;
  }

  .panel-topbar {
    display: flex;
    justify-content: flex-end;
    margin-bottom: 2px;
  }

  .settings-btn {
    background: none;
    border: none;
    padding: 1px 3px;
    font-size: 12px;
    line-height: 1;
    color: rgba(255, 255, 255, 0.4);
    opacity: 0.55;
    cursor: pointer;
    transition: opacity 0.15s;
  }

  .settings-btn:hover {
    opacity: 1;
  }

  .session-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .empty {
    text-align: center;
    padding: 20px 12px;
  }

  .empty p {
    color: rgba(255, 255, 255, 0.5);
    font-size: 12px;
    margin: 0;
  }

  .empty .hint {
    color: rgba(255, 255, 255, 0.3);
    font-size: 11px;
    margin-top: 4px;
  }

  .session-card {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 8px;
    padding: 8px 10px;
    cursor: pointer;
    text-align: left;
    width: 100%;
    transition: all 0.15s;
    color: inherit;
    font: inherit;
  }

  .session-card:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  .session-card.active {
    border-color: rgba(100, 149, 237, 0.3);
  }

  .session-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .session-source {
    color: rgba(255, 255, 255, 0.8);
    font-size: 12px;
    font-weight: 500;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.2);
  }

  .dot.active {
    background: #27ae60;
  }

  .session-cwd {
    color: rgba(255, 255, 255, 0.4);
    font-size: 10px;
    margin-top: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-model {
    color: rgba(255, 255, 255, 0.3);
    font-size: 10px;
    margin-top: 1px;
  }

  .pet-panel {
    padding: 12px;
    text-align: center;
  }

  .pet-status {
    color: rgba(255, 255, 255, 0.7);
    font-size: 13px;
    margin: 10px 0 0;
  }

  .growth-header {
    display: flex;
    align-items: center;
    gap: 8px;
    text-align: left;
  }

  .name-row {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    background: none;
    border: none;
    padding: 2px 0 8px;
    color: #fff;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
  }

  .name-row:hover .name-text {
    color: #6495ed;
  }

  .name-chev {
    color: rgba(255, 255, 255, 0.35);
    font-weight: 400;
  }

  .stage-emoji {
    font-size: 22px;
    line-height: 1;
  }

  .growth-text {
    min-width: 0;
  }

  .stage-line {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .stage-name {
    color: rgba(255, 255, 255, 0.9);
    font-size: 13px;
    font-weight: 600;
  }

  .style-tag {
    font-size: 9px;
    font-weight: 600;
    border-radius: 6px;
    padding: 1px 6px;
    color: #1a1a20;
    background: rgba(255, 200, 120, 0.85);
  }

  .style-tag.style-commander {
    background: rgba(120, 160, 240, 0.9);
  }

  .style-tag.style-zen {
    background: rgba(100, 210, 150, 0.9);
  }

  .style-tag.style-companion {
    background: rgba(255, 150, 185, 0.9);
  }

  .days-line {
    color: rgba(255, 255, 255, 0.4);
    font-size: 10px;
    margin-top: 1px;
  }

  .xp-bar {
    margin-top: 8px;
    height: 6px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.08);
    overflow: hidden;
  }

  .xp-fill {
    height: 100%;
    border-radius: 3px;
    background: linear-gradient(90deg, #f5a623, #f7ce4d);
    transition: width 0.4s ease;
  }

  .xp-label {
    margin-top: 3px;
    color: rgba(255, 255, 255, 0.35);
    font-size: 10px;
    text-align: right;
  }

  .btn-row {
    margin-top: 10px;
    display: flex;
    gap: 6px;
    justify-content: center;
  }

  .action-btn {
    flex: 1;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 6px 8px;
    color: rgba(255, 255, 255, 0.85);
    font-size: 11px;
    cursor: pointer;
    transition: all 0.15s;
    white-space: nowrap;
  }

  .action-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
  }

  .action-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .action-btn.gift:not(:disabled) {
    border-color: rgba(245, 166, 35, 0.45);
    color: #f7ce4d;
  }

  .board-section {
    margin-top: 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    padding-top: 10px;
    text-align: left;
  }

  .board-title {
    color: rgba(255, 255, 255, 0.75);
    font-size: 11px;
    font-weight: 600;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .board-badges {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .streak {
    color: rgba(255, 180, 90, 0.9);
    font-size: 11px;
    font-weight: 600;
  }

  .shields {
    font-size: 11px;
    letter-spacing: 1px;
  }

  .board-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    color: rgba(255, 255, 255, 0.65);
    font-size: 11px;
  }

  .board-row.done {
    color: rgba(255, 255, 255, 0.4);
  }

  .ach-section {
    margin-top: 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    padding-top: 10px;
    text-align: left;
  }

  .ach-title {
    color: rgba(255, 255, 255, 0.75);
    font-size: 11px;
    font-weight: 600;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .ach-count {
    color: rgba(255, 255, 255, 0.35);
    font-weight: 500;
  }

  .ach-grid {
    margin-top: 8px;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(26px, 1fr));
    gap: 4px;
  }

  .ach-tile {
    aspect-ratio: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 14px;
    border-radius: 6px;
    background: rgba(245, 166, 35, 0.14);
    border: 1px solid rgba(245, 166, 35, 0.3);
    cursor: help;
  }

  .ach-tile.locked {
    background: rgba(255, 255, 255, 0.03);
    border-color: rgba(255, 255, 255, 0.06);
    filter: grayscale(1);
    opacity: 0.45;
  }
</style>
