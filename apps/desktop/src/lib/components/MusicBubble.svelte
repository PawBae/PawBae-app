<script lang="ts">
  // The line the pet says while it detects you're listening to music (music-reaction
  // feature). Parent-driven like PetReplyBubble: MascotView sets `text` when the music
  // machine reports `justEntered` (and on the rotate timer) and clears it on exit. Shares
  // the bubble shell so it matches the voice/reply bubbles.
  import SpeechBubble from './SpeechBubble.svelte';

  interface MusicBubbleProps {
    text?: string;
    /** 'above' for pet mode; 'below' for coding mode, where the menu-bar edge clips upward. */
    placement?: 'above' | 'below';
  }

  let { text = '', placement = 'above' }: MusicBubbleProps = $props();
</script>

{#if text}
  <!-- {#key} restarts the pop animation when one line rotates to the next. -->
  {#key text}
    <SpeechBubble {placement} variant="reply" ariaLive="off">🎧 {text}</SpeechBubble>
  {/key}
{/if}
