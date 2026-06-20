<script lang="ts">
  // Echoes what the recognizer heard. Mini/coding mode shows a small pulsing dot while
  // recording; pet mode shows the live transcript (with a blinking cursor) or an error,
  // using the shared SpeechBubble shell.
  import SpeechBubble from './SpeechBubble.svelte';

  interface VoiceBubbleProps {
    visible?: boolean;
    text?: string;
    recording?: boolean;
    error?: string;
    petMode?: boolean;
  }

  let {
    visible = false,
    text = '',
    recording = false,
    error = '',
    petMode = false,
  }: VoiceBubbleProps = $props();

  const show = $derived(visible && (recording || text || error));
</script>

{#if !petMode && recording}
  <div class="voice-dot"></div>
{/if}

{#if petMode && show}
  <SpeechBubble placement="below" variant={error ? 'voice-error' : 'voice'}>
    {#if error}
      {error}
    {:else}
      {text}
      {#if recording}
        <span class="blink-cursor">|</span>
      {/if}
    {/if}
  </SpeechBubble>
{/if}

<style>
  .voice-dot {
    position: fixed;
    top: 2px;
    right: 2px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #f5a623;
    animation: voicePulse 1.2s infinite;
    z-index: 100;
  }

  .blink-cursor {
    animation: voiceBlink 1s step-end infinite;
  }

  @keyframes voicePulse {
    0%,
    100% {
      opacity: 1;
      transform: scale(1);
    }
    50% {
      opacity: 0.5;
      transform: scale(1.2);
    }
  }

  @keyframes voiceBlink {
    0%,
    50% {
      opacity: 1;
    }
    51%,
    100% {
      opacity: 0;
    }
  }
</style>
