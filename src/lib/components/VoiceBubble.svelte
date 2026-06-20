<script lang="ts">
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
  <div class="voice-bubble-wrap">
    <div class="voice-bubble" class:error={!!error}>
      {#if error}
        {error}
      {:else}
        {text}
        {#if recording}
          <span class="blink-cursor">|</span>
        {/if}
      {/if}
    </div>
    <div class="voice-pointer"></div>
  </div>
{/if}

<style>
  .voice-dot {
    position: fixed;
    top: 2px;
    right: 2px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #F5A623;
    animation: voicePulse 1.2s infinite;
    z-index: 100;
  }

  .voice-bubble-wrap {
    position: absolute;
    bottom: 0;
    left: 50%;
    transform: translate(-50%, 100%);
    display: flex;
    flex-direction: column-reverse;
    align-items: center;
    z-index: 100;
  }

  .voice-bubble {
    background: #F5A623;
    color: #1a1a20;
    border-radius: 18px;
    padding: 6px 14px;
    font-size: 12px;
    font-weight: 600;
    /* Grow horizontally first (max-content = widest single line), wrapping only past
       max-width. fit-content collapses CJK to a vertical column here. */
    width: max-content;
    max-width: 180px;
    white-space: normal;
    overflow-wrap: break-word;
    text-align: center;
    line-height: 1.3;
  }

  .voice-bubble.error {
    background: #e74c3c;
    color: white;
  }

  .voice-pointer {
    width: 0;
    height: 0;
    border-left: 6px solid transparent;
    border-right: 6px solid transparent;
    border-bottom: 6px solid #F5A623;
  }

  .blink-cursor {
    animation: voiceBlink 1s step-end infinite;
  }

  @keyframes voicePulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.5; transform: scale(1.2); }
  }

  @keyframes voiceBlink {
    0%, 50% { opacity: 1; }
    51%, 100% { opacity: 0; }
  }
</style>
