<script lang="ts">
  import type { ProjectionStatus } from '../../platform/types';
  import { skinsStore } from '../../stores/skins.svelte';
  import type { CodexPet } from '../../utils/codex-pet';
  import { petStateToCodexState } from '../../utils/codex-pet';
  import type { HomePetIdentity } from '../../utils/social-home';
  import SpritePet from '../SpritePet.svelte';

  let {
    pet,
    legacyPet,
    size,
    guest = false,
    state = 'idle',
  }: {
    pet: HomePetIdentity;
    legacyPet: CodexPet | null;
    size: number;
    guest?: boolean;
    state?: ProjectionStatus;
  } = $props();

  const glowColors: Record<string, string> = {
    solu: 'rgba(245, 143, 94, 0.18)',
    muru: 'rgba(179, 199, 240, 0.24)',
    riffi: 'rgba(168, 224, 192, 0.2)',
    luma: 'rgba(245, 175, 200, 0.2)',
  };

  const glow = $derived(
    pet.officialPetId ? glowColors[pet.officialPetId] : 'rgba(99, 94, 103, 0.12)',
  );
  const resolvedPet = $derived.by(() => {
    skinsStore.revision;
    const assetId = pet.officialPetId ?? pet.id;
    return skinsStore.resolveExact(assetId) ?? (legacyPet?.id === assetId ? legacyPet : null);
  });
  const assetId = $derived(pet.officialPetId ?? pet.id);
  const spriteState = $derived.by(() => {
    if (!resolvedPet) return 'idle';
    if (state === 'offline' && resolvedPet.animations.sleep) return 'sleep';
    return petStateToCodexState(resolvedPet, state === 'offline' ? 'idle' : state);
  });
</script>

<div
  class="artwork"
  class:guest
  role="img"
  aria-label={pet.name}
  style={`--art-size:${size}px;--pet-glow:${glow}`}
>
  {#if resolvedPet}
    <div class="sprite-art" data-pet-id={resolvedPet.id}>
      <SpritePet pet={resolvedPet} state={spriteState} size={Math.round(size * 0.88)} loop />
    </div>
  {:else}
    <div class="missing-art" data-pet-id={assetId} aria-hidden="true">✦</div>
  {/if}
</div>

<style>
  .artwork {
    position: relative;
    display: grid;
    width: var(--art-size);
    max-width: 100%;
    height: calc(var(--art-size) * 1.08);
    place-items: center;
    filter: drop-shadow(0 8px 8px color-mix(in srgb, var(--pet-glow) 70%, transparent));
    animation: arrive 220ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  .artwork::before {
    position: absolute;
    right: 6%;
    bottom: 4%;
    left: 6%;
    height: 16%;
    border-radius: 50%;
    background: var(--pet-glow);
    content: '';
    filter: blur(14px);
    transform: scaleY(0.45);
  }

  .sprite-art {
    position: relative;
    display: grid;
    place-items: end center;
  }

  .missing-art {
    position: relative;
    display: grid;
    width: 52%;
    aspect-ratio: 1;
    place-items: center;
    border-radius: 50%;
    background: var(--home-subtle);
    color: var(--home-text-muted);
    font-size: calc(var(--art-size) * 0.2);
  }

  .guest {
    animation-duration: 360ms;
  }

  @keyframes arrive {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .artwork {
      animation: none;
    }
  }
</style>
