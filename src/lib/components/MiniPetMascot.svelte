<script lang="ts">
  import type { CodexPet, CodexPetState } from '../utils/codex-pet';
  import SpritePet from './SpritePet.svelte';

  interface MiniPetMascotProps {
    pet: CodexPet;
    baseState: CodexPetState;
    size: number;
    enableHoverJump?: boolean;
    externalHover?: boolean;
    useExternalHover?: boolean;
    suppressHover?: boolean;
    reactionSprite?: CodexPetState | null;
    class?: string;
    style?: string;
  }

  let {
    pet,
    baseState,
    size,
    enableHoverJump = true,
    externalHover = false,
    useExternalHover = false,
    suppressHover = false,
    reactionSprite = null,
    class: className = '',
    style = '',
  }: MiniPetMascotProps = $props();

  let internalHover = $state(false);
  let showJump = $state(false);
  let jumpKey = $state(0);
  let hovering = false;
  let restTimer: ReturnType<typeof setTimeout> | null = null;

  const hasJump = $derived(pet?.animations?.jumping != null);
  const isHovering = $derived(
    hasJump && enableHoverJump && !suppressHover && (useExternalHover ? externalHover : internalHover)
  );

  $effect(() => {
    if (isHovering && !showJump) {
      hovering = true;
      showJump = true;
      jumpKey++;
    }
    if (!isHovering) {
      hovering = false;
      showJump = false;
      if (restTimer) {
        clearTimeout(restTimer);
        restTimer = null;
      }
    }
    return () => {
      if (restTimer) {
        clearTimeout(restTimer);
        restTimer = null;
      }
    };
  });

  function handleJumpEnd() {
    if (!hovering) {
      showJump = false;
      return;
    }
    restTimer = setTimeout(() => {
      if (hovering) {
        jumpKey++;
      } else {
        showJump = false;
      }
    }, 400);
  }

  function onEnter() {
    if (!useExternalHover) internalHover = true;
  }

  function onLeave() {
    if (!useExternalHover) internalHover = false;
  }

  // The input-reaction overlay (P1-B) outranks the hover-jump; the MascotView busy-guard
  // ensures they are never active at once. Feature-detect the art so pets without reaction
  // rows degrade to the base state (mirrors the `hasJump` guard above).
  const hasReaction = $derived(
    reactionSprite != null && pet?.animations?.[reactionSprite] != null
  );
  const renderState: CodexPetState = $derived(
    hasReaction ? (reactionSprite as CodexPetState) : showJump ? 'jumping' : baseState
  );
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class={className}
  {style}
  onmouseenter={onEnter}
  onmouseleave={onLeave}
>
  {#key jumpKey}
    <SpritePet {pet} state={renderState} {size} onOneShotEnd={handleJumpEnd} />
  {/key}
</div>
