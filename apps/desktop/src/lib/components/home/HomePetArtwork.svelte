<script lang="ts">
  import type { CodexPet } from '../../utils/codex-pet';
  import type { OfficialPetId } from '../../utils/onboarding';
  import { tileFrameStyle } from '../../utils/skins';
  import type { HomePetIdentity } from '../../utils/social-home';

  let {
    pet,
    legacyPet,
    size,
    guest = false,
  }: {
    pet: HomePetIdentity;
    legacyPet: CodexPet | null;
    size: number;
    guest?: boolean;
  } = $props();

  const positions: Record<OfficialPetId, string> = {
    solu: '0%',
    muru: '33.333%',
    riffi: '66.667%',
    luma: '100%',
  };

  const glowColors: Record<OfficialPetId, string> = {
    solu: 'rgba(245, 143, 94, 0.18)',
    muru: 'rgba(179, 199, 240, 0.24)',
    riffi: 'rgba(168, 224, 192, 0.2)',
    luma: 'rgba(245, 175, 200, 0.2)',
  };

  const glow = $derived(
    pet.officialPetId ? glowColors[pet.officialPetId] : 'rgba(99, 94, 103, 0.12)',
  );
</script>

<div
  class="artwork"
  class:guest
  role="img"
  aria-label={pet.name}
  style={`--art-size:${size}px;--pet-glow:${glow}`}
>
  {#if pet.officialPetId}
    <div
      class="official-art"
      data-pet-id={pet.officialPetId}
      style={`--poster-x:${positions[pet.officialPetId]}`}
    ></div>
  {:else if legacyPet}
    <div class="legacy-art" data-pet-id={pet.id} style={tileFrameStyle(legacyPet, size)}></div>
  {:else}
    <div class="missing-art" aria-hidden="true">✦</div>
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

  .official-art {
    position: relative;
    width: 100%;
    height: 100%;
    background-image: url('/assets/onboarding/pet-family-poster.png');
    background-position: var(--poster-x) 36%;
    background-repeat: no-repeat;
    background-size: 400% auto;
  }

  .legacy-art {
    position: relative;
    max-width: 100%;
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
