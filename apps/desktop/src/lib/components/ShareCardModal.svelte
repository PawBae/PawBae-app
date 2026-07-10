<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { Image } from '@tauri-apps/api/image';
  import { writeImage } from '@tauri-apps/plugin-clipboard-manager';
  import { save } from '@tauri-apps/plugin-dialog';
  import { untrack } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { petStore } from '../stores/pet.svelte';
  import { settingsStore } from '../stores/settings.svelte';
  import { skinsStore } from '../stores/skins.svelte';
  import type { ClaudeStats } from '../types';
  import { animationFor } from '../utils/codex-pet';
  import { EVOLUTION_STAGES } from '../utils/evolution';
  import { tryInvoke } from '../utils/invoke';
  import { effectiveName } from '../utils/pet-name';
  import { renderShareCard, type SpriteFrame } from '../utils/share-card';
  import { track } from '../utils/telemetry';
  import { STATS_BUCKET_SOURCES } from '../utils/token-feed';
  import { assembleWeeklyReport } from '../utils/weekly-report';

  let { open = false, onclose }: { open?: boolean; onclose: () => void } = $props();

  let canvasEl: HTMLCanvasElement | undefined = $state();
  let building = $state(false);
  let feedback = $state<'' | 'copied' | 'saved' | 'error'>('');

  // Rebuild exactly once per open (PrivacySection's prevOpen pattern). buildCard
  // reads AND writes `building` synchronously — calling it tracked would make the
  // effect retrigger itself forever (buttons stuck disabled, stats refetched in a
  // loop). untrack() confines the effect's dependencies to open/canvasEl.
  let prevOpen = false;
  $effect(() => {
    const ready = open && canvasEl !== undefined;
    if (ready && !prevOpen) {
      feedback = '';
      untrack(() => void buildCard());
    }
    prevOpen = ready;
  });

  async function loadSprite(): Promise<{ sprite: SpriteFrame | null; name: string }> {
    try {
      await skinsStore.ensureLoaded();
      const pet = skinsStore.resolve(settingsStore.miniPetId);
      if (!pet) return { sprite: null, name: 'PawBae' };
      // The "together with __" line uses the name the user calls her (nickname
      // wins); the pawbae.ai watermark stays the brand.
      const name = effectiveName(settingsStore.petNicknames[pet.id], pet.displayName);
      const idle = animationFor(pet, 'idle');
      const image = await new Promise<HTMLImageElement | null>((resolve) => {
        const img = new globalThis.Image();
        img.onload = () => resolve(img);
        img.onerror = () => resolve(null);
        img.src = pet.spritesheetUrl;
      });
      if (!image) return { sprite: null, name };
      return {
        sprite: {
          image,
          sx: 0,
          sy: (idle?.row ?? 0) * pet.atlas.cellH,
          sw: pet.atlas.cellW,
          sh: pet.atlas.cellH,
        },
        name,
      };
    } catch {
      return { sprite: null, name: 'PawBae' };
    }
  }

  async function buildCard() {
    if (!canvasEl || building) return;
    building = true;
    try {
      const [{ sprite, name }, ...statsList] = await Promise.all([
        loadSprite(),
        ...STATS_BUCKET_SOURCES.map((source) =>
          tryInvoke<ClaudeStats>('get_claude_stats', { source }),
        ),
      ]);
      const report = assembleWeeklyReport({
        statsList,
        recentAwards: petStore.rewards.recent,
        streak: petStore.streakLive,
        shields: petStore.petData.shields,
        stageIndex: petStore.evolution.stageIndex,
        daysTogether: petStore.daysTogether + 1,
        petName: name,
        lang: settingsStore.language,
        now: Date.now(),
      });
      const stage = EVOLUTION_STAGES[report.stageIndex] ?? EVOLUTION_STAGES[0];
      const tasks = `${report.agentTasks}${report.tasksCapped ? '+' : ''}`;
      renderShareCard(
        canvasEl,
        {
          weekLabel: report.weekLabel,
          stageLine: `${stage.emoji} ${$_(`growth.stage.${stage.id}`)}`,
          heroLabel: $_('share.heroLabel'),
          heroNumber: report.totalTokensLabel,
          heroSuffix: 'tokens',
          statsLine: `🤖 ${tasks} ${$_('share.tasks')} · 💬 ${report.messages} ${$_('share.messages')}`,
          streakLine:
            report.streak > 0
              ? `🔥 ${$_('share.streak', { values: { days: report.streak } })} ${'🛡️'.repeat(report.shields)}`.trimEnd()
              : '',
          togetherLine: $_('share.together', {
            values: { name: report.petName, days: report.daysTogether },
          }),
          dailyTokens: report.dailyTokens,
        },
        sprite,
      );
    } finally {
      building = false;
    }
  }

  async function pngBytes(): Promise<Uint8Array | null> {
    if (!canvasEl) return null;
    const blob = await new Promise<Blob | null>((resolve) =>
      canvasEl?.toBlob((b) => resolve(b), 'image/png'),
    );
    if (!blob) return null;
    return new Uint8Array(await blob.arrayBuffer());
  }

  async function saveCard() {
    try {
      const day = new Date();
      const stamp = `${day.getFullYear()}-${String(day.getMonth() + 1).padStart(2, '0')}-${String(day.getDate()).padStart(2, '0')}`;
      const path = await save({
        defaultPath: `pawbae-weekly-${stamp}.png`,
        filters: [{ name: 'PNG', extensions: ['png'] }],
      });
      if (!path) return; // user cancelled — not an error
      const bytes = await pngBytes();
      if (!bytes) throw new Error('empty canvas');
      let bin = '';
      for (const b of bytes) bin += String.fromCharCode(b);
      // Raw invoke, not tryInvoke: a swallowed write failure (permissions, disk
      // full) would fall through to feedback = 'saved' with no PNG on disk.
      await invoke('save_png_file', { path, data: btoa(bin) });
      feedback = 'saved';
      track('share_card_export', { method: 'save' });
    } catch (e) {
      console.warn('[share-card] save failed:', e);
      feedback = 'error';
    }
  }

  async function copyCard() {
    try {
      const bytes = await pngBytes();
      if (!bytes) throw new Error('empty canvas');
      await writeImage(await Image.fromBytes(bytes));
      feedback = 'copied';
      track('share_card_export', { method: 'copy' });
    } catch (e) {
      console.warn('[share-card] copy failed:', e);
      feedback = 'error';
    }
  }
</script>

{#if open}
  <div class="overlay" role="presentation" onclick={onclose}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <canvas bind:this={canvasEl} class="preview" aria-label={$_('share.button')}></canvas>
      <div class="btn-row">
        <button class="btn primary" onclick={saveCard} disabled={building}>
          💾 {$_('share.save')}
        </button>
        <button class="btn" onclick={copyCard} disabled={building}>
          📋 {$_('share.copy')}
        </button>
        <button class="btn" onclick={onclose}>✕</button>
      </div>
      {#if feedback === 'copied'}<p class="hint ok">{$_('share.copied')}</p>{/if}
      {#if feedback === 'saved'}<p class="hint ok">{$_('share.saved')}</p>{/if}
      {#if feedback === 'error'}<p class="hint err">{$_('share.exportFailed')}</p>{/if}
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
    background: #1a1a20;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    padding: 14px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }

  .preview {
    width: 240px;
    aspect-ratio: 3 / 4;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .btn-row {
    display: flex;
    gap: 8px;
  }

  .btn {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.85);
    border-radius: 8px;
    padding: 6px 12px;
    font-size: 12px;
    cursor: pointer;
  }

  .btn.primary {
    background: rgba(100, 149, 237, 0.25);
    border-color: rgba(100, 149, 237, 0.5);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .hint {
    margin: 0;
    font-size: 11px;
  }

  .hint.ok {
    color: #7dd3a0;
  }

  .hint.err {
    color: #fbbf24;
  }
</style>
