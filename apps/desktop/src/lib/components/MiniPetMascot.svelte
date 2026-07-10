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

  // Render priority: hover-jump > input reaction (P1-B) > base state. Hover-jump outranks the
  // reaction so an active jump is never interrupted even when hover is tracked internally
  // (Windows), where the MascotView guard can't observe it. Feature-detect the reaction art so
  // pets without those rows fall through to the base state (mirrors the `hasJump` guard above).
  const hasReaction = $derived(
    reactionSprite != null && pet?.animations?.[reactionSprite] != null
  );
  const renderState: CodexPetState = $derived(
    showJump ? 'jumping' : hasReaction ? (reactionSprite as CodexPetState) : baseState
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
