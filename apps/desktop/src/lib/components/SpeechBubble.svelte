<script lang="ts">
  // Shared speech-bubble shell for the voice bubbles (heard echo + pet reply). Owns the
  // above/below positioning, the triangle pointer, the text-sizing rules, and the per-variant
  // colour; callers project their content via the default snippet. Extracted so the bubble
  // sizing/wrapping (max-content / 180px window cap) lives in one place instead of three.
  import type { Snippet } from 'svelte';

  interface SpeechBubbleProps {
    /** 'above' the pet (pet mode) or 'below' it (coding mode / mini window). */
    placement?: 'above' | 'below';
    variant?: 'reply' | 'voice' | 'voice-error';
    ariaLive?: 'polite' | 'off';
    children: Snippet;
  }

  let { placement = 'below', variant = 'reply', ariaLive, children }: SpeechBubbleProps = $props();
</script>

<div class="bubble-wrap {placement}" aria-live={ariaLive}>
  <div class="bubble {variant}">{@render children()}</div>
  <div class="pointer {placement} {variant}"></div>
</div>

<style>
  .bubble-wrap {
    position: absolute;
    left: 50%;
    display: flex;
    align-items: center;
    pointer-events: none;
    z-index: 100;
  }

  .bubble-wrap.above {
    top: 0;
    transform: translate(-50%, -100%);
    flex-direction: column;
  }

  /* In coding mode the pet sits at the top edge, so the bubble hangs below it and the tail
     flips to point upward. */
  .bubble-wrap.below {
    bottom: 0;
    transform: translate(-50%, 100%);
    flex-direction: column-reverse;
  }

  /* Shared text sizing: grow horizontally first (max-content = widest single line), wrap to
     a second line only past max-width, and stay inside the 200px mini window. fit-content
     collapses CJK to a per-character vertical column here, so use max-content. */
  .bubble {
    color: #1a1a20;
    font-size: 12px;
    font-weight: 600;
    width: max-content;
    max-width: 180px;
    white-space: normal;
    overflow-wrap: break-word;
    text-align: center;
    line-height: 1.3;
  }

  .bubble.reply {
    background: rgba(255, 255, 255, 0.96);
    border: 1px solid rgba(245, 166, 35, 0.5);
    border-radius: 14px;
    padding: 5px 12px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.18);
    animation: bubblePop 0.32s ease-out;
  }

  .bubble.voice {
    background: #f5a623;
    border-radius: 18px;
    padding: 6px 14px;
  }

  .bubble.voice-error {
    background: #e74c3c;
    color: white;
    border-radius: 18px;
    padding: 6px 14px;
  }

  .pointer {
    width: 0;
    height: 0;
    border-left: 6px solid transparent;
    border-right: 6px solid transparent;
  }

  .pointer.above.reply {
    border-top: 6px solid rgba(255, 255, 255, 0.96);
  }
  .pointer.below.reply {
    border-bottom: 6px solid rgba(255, 255, 255, 0.96);
  }
  /* Voice tail stays orange even in the error state, matching the prior VoiceBubble. */
  .pointer.above.voice,
  .pointer.above.voice-error {
    border-top: 6px solid #f5a623;
  }
  .pointer.below.voice,
  .pointer.below.voice-error {
    border-bottom: 6px solid #f5a623;
  }

  @keyframes bubblePop {
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
