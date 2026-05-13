<script lang="ts">
  import SpritePet from './SpritePet.svelte';
  import type { CodexPet, CodexPetState } from './codexPet';

  let {
    pet,
    baseState,
    size,
    enableHoverJump = true,
    externalHover = false,
    useExternalHover = false,
    suppressHover = false,
    class: className = '',
    style = '',
  }: {
    pet: CodexPet;
    baseState: CodexPetState;
    size: number;
    enableHoverJump?: boolean;
    externalHover?: boolean;
    useExternalHover?: boolean;
    suppressHover?: boolean;
    class?: string;
    style?: string;
  } = $props();

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

  const renderState: CodexPetState = $derived(showJump ? 'jumping' : baseState);
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
