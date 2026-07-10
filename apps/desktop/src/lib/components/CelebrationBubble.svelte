<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { GrowthCelebration } from '../types';
  import { ACHIEVEMENTS } from '../utils/achievements';
  import { EVOLUTION_STAGES } from '../utils/evolution';
  import { SOUVENIR_CATALOG } from '../utils/souvenirs';

  interface CelebrationBubbleProps {
    celebration: GrowthCelebration | null;
    /** 'above' for pet mode; 'below' for coding mode, where the menu-bar edge clips upward. */
    placement?: 'above' | 'below';
  }

  let { celebration, placement = 'above' }: CelebrationBubbleProps = $props();

  const stage = $derived(
    celebration?.kind === 'evolution' ? EVOLUTION_STAGES[celebration.stageIndex] : null,
  );
  const achievement = $derived(
    celebration?.kind === 'achievement'
      ? (ACHIEVEMENTS.find((d) => d.id === celebration.id) ?? null)
      : null,
  );
  const souvenir = $derived(
    celebration?.kind === 'souvenir'
      ? (SOUVENIR_CATALOG.find((d) => d.id === celebration.id) ?? null)
      : null,
  );
  const GREETING_EMOJI = { morning: '🌅', day: '☀️', evening: '🌆', night: '🌙' } as const;
</script>

{#if stage}
  <!-- The {#key} restarts the CSS animations when one evolution follows another. -->
  {#key celebration}
    <div class="flash" aria-hidden="true"></div>
    <div class="bubble-wrap {placement}">
      <div class="bubble evolution">
        ✨ {$_('growth.evolvedTo')} {stage.emoji}
        {$_(`growth.stage.${stage.id}`)}
      </div>
    </div>
  {/key}
{:else if achievement}
  {#key celebration}
    <div class="bubble-wrap {placement}">
      <div class="bubble">
        🏆 {achievement.emoji}
        {$_(`growth.ach.${achievement.id}`)}
      </div>
    </div>
  {/key}
{:else if celebration?.kind === 'perfect_day'}
  {#key celebration}
    <div class="bubble-wrap {placement}">
      <div class="bubble">
        🎉 {$_('board.perfectDay')}
      </div>
    </div>
  {/key}
{:else if souvenir}
  {#key celebration}
    <div class="bubble-wrap {placement}">
      <div class="bubble">
        🎁 {souvenir.emoji}
        {$_(`souvenir.${souvenir.id}.name`)}
      </div>
    </div>
  {/key}
{:else if celebration?.kind === 'egg_found'}
  {#key celebration}
    <div class="bubble-wrap {placement}">
      <div class="bubble">
        🥚 {$_('egg.foundBubble')}
      </div>
    </div>
  {/key}
{:else if celebration?.kind === 'greeting'}
  {#key celebration}
    <div class="bubble-wrap {placement}">
      <div class="bubble greeting">
        {GREETING_EMOJI[celebration.part]}
        {$_(`greet.${celebration.part}`)}{#if celebration.tasks > 0}
          {' '}{$_('greet.yesterday', { values: { n: celebration.tasks } })}{/if}
      </div>
    </div>
  {/key}
{/if}

<style>
  /* Radial burst behind the sprite during an evolution. Decor only: never intercepts
     the drag region or headpat clicks. */
  .flash {
    position: absolute;
    inset: -30%;
    border-radius: 50%;
    background: radial-gradient(
      circle,
      rgba(255, 235, 130, 0.85) 0%,
      rgba(255, 215, 80, 0.35) 45%,
      transparent 70%
    );
    animation: evoFlash 1.4s ease-out forwards;
    pointer-events: none;
    z-index: 99;
  }

  .bubble-wrap {
    position: absolute;
    left: 50%;
    display: flex;
    justify-content: center;
    pointer-events: none;
    z-index: 100;
  }

  .bubble-wrap.above {
    top: 0;
    transform: translate(-50%, -100%);
  }

  .bubble-wrap.below {
    bottom: 0;
    transform: translate(-50%, 100%);
  }

  .bubble {
    background: rgba(26, 26, 32, 0.92);
    color: rgba(255, 255, 255, 0.92);
    border: 1px solid rgba(255, 215, 80, 0.4);
    border-radius: 14px;
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 600;
    white-space: nowrap;
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    animation: bubblePop 0.35s ease-out;
  }

  .bubble.evolution {
    background: linear-gradient(135deg, #f5a623, #f7ce4d);
    color: #1a1a20;
    border: none;
  }

  /* Greetings run longer than one-liner celebrations — let them wrap instead of
     ellipsizing mid-sentence. Still a pure decoration layer (no pointer events). */
  .bubble.greeting {
    white-space: normal;
    text-align: center;
    line-height: 1.5;
  }

  @keyframes evoFlash {
    0% {
      opacity: 0;
      transform: scale(0.4);
    }
    25% {
      opacity: 1;
      transform: scale(1.05);
    }
    100% {
      opacity: 0;
      transform: scale(1.5);
    }
  }

  @keyframes bubblePop {
    0% {
      opacity: 0;
      transform: scale(0.7);
    }
    100% {
      opacity: 1;
      transform: scale(1);
    }
  }
</style>
