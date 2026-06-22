<script lang="ts">
  // The pet's spoken-back reply to a recognized voice intent (voice-interaction Phase B).
  //
  // Deliberately separate from the orange "heard" echo: VoiceBubble echoes "what I heard
  // you say"; this bubble (light, pet-tinted) is "what the pet says back", so the two read
  // as a back-and-forth. Visibility is parent-driven — the wiring layer sets `text` on a
  // final intent and clears it on a timer. Shares the bubble shell with VoiceBubble.
  import SpeechBubble from './SpeechBubble.svelte';

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
    <SpeechBubble {placement} variant="reply" ariaLive="polite">{text}</SpeechBubble>
  {/key}
{/if}
