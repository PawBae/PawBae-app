<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { petStore } from '../stores/pet.svelte';
  import { settingsStore } from '../stores/settings.svelte';
  import { skinsStore } from '../stores/skins.svelte';
  import { ACHIEVEMENTS } from '../utils/achievements';
  import { daysApart } from '../utils/daily-board';
  import {
    type DiaryDaySummary,
    type DiaryMoment,
    localDayOf,
    summaryVariant,
  } from '../utils/diary';
  import { EVOLUTION_STAGES } from '../utils/evolution';
  import { effectiveName } from '../utils/pet-name';
  import { track } from '../utils/telemetry';

  let { open = false, onclose }: { open?: boolean; onclose: () => void } = $props();

  let prevOpen = false;
  $effect(() => {
    if (open && !prevOpen) track('diary_opened');
    prevOpen = open;
  });

  interface DayGroup {
    day: string;
    summary: DiaryDaySummary | null;
    moments: DiaryMoment[];
  }

  // Group by the entry's own `day` (a summary is APPENDED the day after it happened,
  // so array order alone would misfile it); newest day first, moments in event order.
  const groups = $derived.by(() => {
    const byDay = new Map<string, DayGroup>();
    for (const e of petStore.diary) {
      let g = byDay.get(e.day);
      if (!g) {
        g = { day: e.day, summary: null, moments: [] };
        byDay.set(e.day, g);
      }
      if (e.kind === 'day') g.summary = e as DiaryDaySummary;
      else g.moments.push(e as DiaryMoment);
    }
    for (const g of byDay.values()) g.moments.sort((a, b) => a.at - b.at);
    return [...byDay.values()].sort((a, b) => (a.day < b.day ? 1 : -1));
  });

  function dayLabel(day: string): string {
    const [, m, d] = day.split('-');
    const date = $_('diary.dateLabel', { values: { m: Number(m), d: Number(d) } });
    const gap = daysApart(day, localDayOf(Date.now()));
    if (gap === 0) return `${date} · ${$_('diary.today')}`;
    if (gap === 1) return `${date} · ${$_('diary.yesterday')}`;
    return date;
  }

  /** Compose the day paragraph from its non-zero segments; the opener varies by a
   *  date-seeded pick so the same page always reads the same. */
  function summaryText(s: DiaryDaySummary): string {
    const segs: string[] = [];
    if (s.agentTasks > 0) segs.push($_('diary.seg.tasks', { values: { n: s.agentTasks } }));
    if (s.meals > 0) segs.push($_('diary.seg.meals', { values: { n: s.meals } }));
    if (s.coinsEarned > 0) segs.push($_('diary.seg.coins', { values: { n: s.coinsEarned } }));
    const opener = $_(`diary.opener.v${summaryVariant(s.day)}`);
    return opener + $_('diary.joiner') + segs.join($_('diary.sep')) + $_('diary.end');
  }

  const MOMENT_EMOJI: Record<string, string> = {
    adopted: '🐾',
    evolution: '✨',
    achievement: '🏆',
    perfect_day: '🎉',
    souvenir: '🎁',
    egg_found: '🥚',
    egg_hatched: '🐣',
    dex_completed: '📖',
  };

  /** Null skips the line — unknown kinds from a newer build must not crash the book. */
  function momentText(m: DiaryMoment): string | null {
    switch (m.kind) {
      case 'adopted':
        return $_('diary.m.adopted');
      case 'evolution': {
        const stage = EVOLUTION_STAGES[Number(m.ref)];
        return stage
          ? $_('diary.m.evolution', { values: { stage: $_(`growth.stage.${stage.id}`) } })
          : null;
      }
      case 'achievement': {
        const def = ACHIEVEMENTS.find((d) => d.id === m.ref);
        return def
          ? $_('diary.m.achievement', { values: { name: $_(`growth.ach.${def.id}`) } })
          : null;
      }
      case 'perfect_day':
        return $_('diary.m.perfectDay');
      case 'souvenir':
        return m.ref
          ? $_('diary.m.souvenir', { values: { name: $_(`souvenir.${m.ref}.name`) } })
          : null;
      case 'egg_found':
        return $_('diary.m.eggFound');
      case 'egg_hatched': {
        if (!m.ref) return null;
        const pet = skinsStore.resolve(m.ref);
        const name = pet
          ? effectiveName(settingsStore.petNicknames[pet.id], pet.displayName)
          : m.ref;
        return $_('diary.m.eggHatched', { values: { name } });
      }
      case 'dex_completed':
        return $_('diary.m.dexCompleted');
      default:
        return null;
    }
  }
</script>

{#if open}
  <div class="overlay" role="presentation" onclick={onclose}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <div class="head">
        <span class="title">📖 {$_('diary.title')}</span>
        <button class="close" onclick={onclose} aria-label="close">✕</button>
      </div>
      <div class="pages">
        {#if groups.length === 0}
          <p class="empty">{$_('diary.empty')}</p>
        {:else}
          {#each groups as g (g.day)}
            <section class="page">
              <h3 class="day">{dayLabel(g.day)}</h3>
              {#if g.summary}
                <p class="summary">{summaryText(g.summary)}</p>
              {/if}
              {#each g.moments as m}
                {@const text = momentText(m)}
                {#if text !== null}
                  <p class="moment">{MOMENT_EMOJI[m.kind] ?? '✦'} {text}</p>
                {/if}
              {/each}
            </section>
          {/each}
        {/if}
      </div>
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
    width: 300px;
    max-height: 70vh;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .title {
    font-size: 13px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.92);
  }

  .close {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.85);
    border-radius: 8px;
    padding: 2px 8px;
    font-size: 12px;
    cursor: pointer;
  }

  .pages {
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding-right: 4px;
  }

  .page {
    border-bottom: 1px dashed rgba(255, 255, 255, 0.08);
    padding-bottom: 10px;
  }

  .page:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  .day {
    margin: 0 0 6px;
    font-size: 11px;
    font-weight: 700;
    color: rgba(255, 215, 80, 0.85);
  }

  .summary {
    margin: 0 0 4px;
    font-size: 12px;
    line-height: 1.6;
    color: rgba(255, 255, 255, 0.88);
  }

  .moment {
    margin: 0 0 2px;
    font-size: 11.5px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.72);
  }

  .empty {
    margin: 8px 0;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.55);
    text-align: center;
  }
</style>
