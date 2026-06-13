<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { AgentActivity, BubbleKind } from '../utils/agent-activity';
  import { bubbleKindFor } from '../utils/agent-activity';

  interface AgentBubbleProps {
    activity: AgentActivity;
    /** Celebrations and voice output outrank the activity bubble. */
    suppressed?: boolean;
  }

  let { activity, suppressed = false }: AgentBubbleProps = $props();

  // Persistence policy (roadmap 2.3: a bubble, not a log window): waiting/compacting
  // stay visible while true — they are the states worth a glance. 'working' flashes only
  // on its rising edge for a beat, then the pet's animation carries the signal alone.
  const WORKING_FLASH_MS = 3500;
  const kind = $derived(bubbleKindFor(activity));
  let workingFlash = $state(false);
  let flashTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    if (kind !== 'working') {
      workingFlash = false;
      if (flashTimer) {
        clearTimeout(flashTimer);
        flashTimer = null;
      }
      return;
    }
    // Rising edge into 'working' (also fires when coming from waiting/compacting —
    // that transition is news too: "they're moving again").
    workingFlash = true;
    flashTimer = setTimeout(() => {
      workingFlash = false;
      flashTimer = null;
    }, WORKING_FLASH_MS);
    return () => {
      if (flashTimer) {
        clearTimeout(flashTimer);
        flashTimer = null;
      }
    };
  });

  const visibleKind = $derived<BubbleKind>(
    suppressed ? null : kind === 'working' ? (workingFlash ? 'working' : null) : kind,
  );

  const countSuffix = $derived(
    visibleKind === 'waiting' && activity.waiting > 1 ? ` (${activity.waiting})` : '',
  );
</script>

{#if visibleKind}
  <div class="agent-bubble-wrap" class:urgent={visibleKind === 'waiting'}>
    <div class="agent-bubble">
      {#if visibleKind === 'waiting'}👀{:else if visibleKind === 'compacting'}🧹{:else}⚙️{/if}
      {$_(`agentBubble.${visibleKind}`)}{countSuffix}
    </div>
  </div>
{/if}

<style>
  .agent-bubble-wrap {
    position: absolute;
    bottom: 0;
    left: 50%;
    transform: translate(-50%, 100%);
    display: flex;
    justify-content: center;
    pointer-events: none;
    z-index: 90; /* below celebrations (100) */
  }

  .agent-bubble {
    background: rgba(26, 26, 32, 0.92);
    color: rgba(255, 255, 255, 0.85);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 12px;
    padding: 3px 9px;
    font-size: 10px;
    font-weight: 600;
    white-space: nowrap;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    animation: agentBubbleIn 0.25s ease-out;
  }

  .urgent .agent-bubble {
    border-color: rgba(245, 166, 35, 0.6);
    color: #f7ce4d;
    animation:
      agentBubbleIn 0.25s ease-out,
      urgentPulse 2s ease-in-out infinite 0.25s;
  }

  @keyframes agentBubbleIn {
    0% {
      opacity: 0;
      transform: translateY(-4px);
    }
    100% {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @keyframes urgentPulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.65;
    }
  }
</style>
