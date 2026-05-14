<script lang="ts">
  import {
    fpsFor,
    loopRestMsFor,
    animationFor,
    type CodexPet,
    type CodexPetState,
  } from '../utils/codex-pet';

  interface Props {
    pet: CodexPet;
    state: CodexPetState;
    size: number;
    loop?: boolean;
    onOneShotEnd?: () => void;
    class?: string;
    style?: string;
  }

  let {
    pet,
    state: currentState,
    size,
    loop: loopAnim = false,
    onOneShotEnd,
    class: className = '',
    style = '',
  }: Props = $props();

  let spriteEl: HTMLDivElement | undefined = $state();

  const row = $derived(animationFor(pet, currentState) ?? animationFor(pet, 'idle'));
  const aspect = $derived(pet.atlas.cellH / pet.atlas.cellW);
  const renderW = $derived(Math.max(1, Math.round(size)));
  const renderH = $derived(Math.max(1, Math.round(size * aspect)));
  const totalW = $derived(renderW * pet.atlas.cols);
  const totalH = $derived(renderH * pet.atlas.rows);
  const rowScale = $derived(row?.displayScale ?? 1);
  const transform = $derived.by(() => {
    const parts: string[] = [];
    if (row?.flipX) parts.push('scaleX(-1)');
    if (rowScale !== 1) parts.push(`scale(${rowScale})`);
    return parts.length ? parts.join(' ') : undefined;
  });

  $effect(() => {
    const state = currentState;
    const curPet = pet;
    const el = spriteEl;
    if (!el) return;

    let frameIndex = 0;
    let restUntil = 0;
    let oneShotFired = false;
    let acc = 0;
    let last = performance.now();
    let raf: number;
    let cancelled = false;

    function tick(now: number) {
      if (cancelled) return;

      if (restUntil > 0) {
        if (now < restUntil) {
          last = now;
          acc = 0;
          raf = requestAnimationFrame(tick);
          return;
        }
        restUntil = 0;
        frameIndex = 0;
        last = now;
        acc = 0;
        raf = requestAnimationFrame(tick);
        return;
      }

      const fps = fpsFor(curPet, state);
      const frameMs = 1000 / fps;
      const dt = now - last;
      last = now;
      acc = Math.min(acc + dt, frameMs * 1.5);

      while (acc >= frameMs) {
        if (restUntil > 0) break;
        acc -= frameMs;

        const curRow = animationFor(curPet, state) ?? animationFor(curPet, 'idle');
        if (!curRow) continue;

        const next = frameIndex + 1;
        const isOneShot = curPet.oneShot.has(state);

        if (isOneShot && !loopAnim) {
          if (next >= curRow.frames) {
            if (!oneShotFired) {
              oneShotFired = true;
              onOneShotEnd?.();
            }
            frameIndex = curRow.frames - 1;
          } else {
            frameIndex = next;
          }
        } else {
          if (next >= curRow.frames) {
            const restMs = loopRestMsFor(curPet, state);
            if (restMs > 0) {
              restUntil = now + restMs;
              frameIndex = curRow.frames - 1;
            } else {
              frameIndex = 0;
            }
          } else {
            frameIndex = next;
          }
        }
      }

      const curRow = animationFor(curPet, state) ?? animationFor(curPet, 'idle');
      if (curRow && el) {
        const offsetCol = curRow.offsetCol ?? 0;
        const frame = Math.min(frameIndex, curRow.frames - 1);
        const bgX = -(offsetCol + frame) * renderW;
        const bgY = -curRow.row * renderH;
        el.style.backgroundPosition = `${bgX}px ${bgY}px`;
      }

      raf = requestAnimationFrame(tick);
    }

    // Set initial position immediately so the first frame is correct
    const initRow = animationFor(curPet, state) ?? animationFor(curPet, 'idle');
    if (initRow) {
      const offsetCol = initRow.offsetCol ?? 0;
      el.style.backgroundPosition = `${-(offsetCol) * renderW}px ${-initRow.row * renderH}px`;
    }

    raf = requestAnimationFrame(tick);
    return () => {
      cancelled = true;
      cancelAnimationFrame(raf);
    };
  });
</script>

{#if row}
  <div
    bind:this={spriteEl}
    data-physics-anchor
    class={className}
    style:width="{renderW}px"
    style:height="{renderH}px"
    style:background-image="url({pet.spritesheetUrl})"
    style:background-repeat="no-repeat"
    style:background-size="{totalW}px {totalH}px"
    style:image-rendering={pet.imageRendering}
    style:will-change="background-position"
    style:transform={transform}
    style:transform-origin={rowScale !== 1 ? 'bottom center' : undefined}
    style:overflow="visible"
    {style}
  ></div>
{/if}
