<script lang="ts">
  import { _ } from 'svelte-i18n';

  interface ApprovalNoteProps {
    /** Waiting sessions the note stands for; 0 hides it. */
    count: number;
    /** Celebrations, voice output and the expanded panel own the same space. */
    suppressed?: boolean;
    /** Injected click handler — the component is pure presentation. */
    onrespond: () => void;
  }

  let { count, suppressed = false, onrespond }: ApprovalNoteProps = $props();

  const visible = $derived(count > 0 && !suppressed);
  const countSuffix = $derived(count > 1 ? ` (${count})` : '');

  function handleClick(e: MouseEvent) {
    // The mascot root's own click handler headpats / toggles the panel — a note
    // click must not double as one.
    e.stopPropagation();
    onrespond();
  }
</script>

{#if visible}
  <div class="approval-note-wrap">
    <button
      type="button"
      class="approval-note"
      aria-label={$_('approvalNote.label')}
      onclick={handleClick}
    >
      📝 {$_('approvalNote.label')}{countSuffix}
    </button>
  </div>
{/if}

<style>
  .approval-note-wrap {
    position: absolute;
    bottom: 0;
    left: 50%;
    transform: translate(-50%, 100%);
    display: flex;
    justify-content: center;
    z-index: 95; /* above the agent bubble (90), below celebrations (100) */
  }

  /* A slip of paper, deliberately unlike the dark status bubbles: this one is a
     thing the pet brought you, not a status readout. */
  .approval-note {
    background: #fdf6e3;
    color: #4a3f2a;
    border: 1px solid rgba(245, 166, 35, 0.7);
    border-radius: 3px 10px 3px 10px;
    padding: 3px 9px;
    font: inherit;
    font-size: 10px;
    font-weight: 600;
    white-space: nowrap;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.25);
    animation:
      noteIn 0.25s ease-out,
      notePulse 2s ease-in-out infinite;
  }

  .approval-note:hover {
    background: #fff9e8;
  }

  @keyframes noteIn {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @keyframes notePulse {
    0%,
    100% {
      box-shadow: 0 1px 4px rgba(0, 0, 0, 0.25);
    }
    50% {
      box-shadow: 0 1px 8px rgba(245, 166, 35, 0.55);
    }
  }
</style>
