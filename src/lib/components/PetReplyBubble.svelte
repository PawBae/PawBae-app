<script lang="ts">
  // The pet's spoken-back reply to a recognized voice intent (voice-interaction Phase B).
  //
  // Deliberately separate from VoiceBubble: VoiceBubble (orange) echoes "what I heard
  // you say"; this bubble (light, pet-tinted) is "what the pet says back", so the two
  // never share one bubble and read as a back-and-forth. Visibility is parent-driven —
  // the wiring layer sets `text` on a final intent and clears it on a timer.

  interface PetReplyBubbleProps {
    text?: string;
    /** 'above' for pet mode; 'below' for coding mode, where the menu-bar edge clips upward. */
    placement?: 'above' | 'below';
  }

  let { text = '', placement = 'above' }: PetReplyBubbleProps = $props();
</script>

{#if text}
  <!-- {#key} restarts the pop animation when one reply follows another. -->
  {#key text}
    <div class="reply-wrap {placement}" aria-live="polite">
      <div class="reply-bubble">{text}</div>
      <div class="reply-pointer {placement}"></div>
    </div>
  {/key}
{/if}

<style>
  .reply-wrap {
    position: absolute;
    left: 50%;
    display: flex;
    flex-direction: column;
    align-items: center;
    pointer-events: none;
    z-index: 100;
  }

  .reply-wrap.above {
    top: 0;
    transform: translate(-50%, -100%);
  }

  /* In coding mode the pet sits at the top edge, so the reply hangs below it and
     the tail flips to point upward. */
  .reply-wrap.below {
    bottom: 0;
    transform: translate(-50%, 100%);
    flex-direction: column-reverse;
  }

  .reply-bubble {
    background: rgba(255, 255, 255, 0.96);
    color: #1a1a20;
    border: 1px solid rgba(245, 166, 35, 0.5);
    border-radius: 14px;
    padding: 5px 12px;
    font-size: 12px;
    font-weight: 600;
    /* Grow horizontally first (max-content = widest single line), only wrapping to a
       second line once the text would exceed max-width. fit-content collapses CJK to a
       per-character vertical column here, so use max-content. Capped below the 200px
       mini-window width so the bubble never overflows the window edge. */
    width: max-content;
    max-width: 180px;
    white-space: normal;
    overflow-wrap: break-word;
    text-align: center;
    line-height: 1.3;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.18);
    animation: replyPop 0.32s ease-out;
  }

  .reply-pointer {
    width: 0;
    height: 0;
    border-left: 6px solid transparent;
    border-right: 6px solid transparent;
  }

  .reply-pointer.above {
    border-top: 6px solid rgba(255, 255, 255, 0.96);
  }

  .reply-pointer.below {
    border-bottom: 6px solid rgba(255, 255, 255, 0.96);
  }

  @keyframes replyPop {
    0% {
      opacity: 0;
      transform: scale(0.7) translateY(4px);
    }
    100% {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }
</style>
