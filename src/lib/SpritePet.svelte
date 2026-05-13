<script lang="ts">
  import { onMount } from 'svelte';
  import {
    fpsFor,
    loopRestMsFor,
    animationFor,
    type CodexPet,
    type CodexPetState,
  } from './codexPet';

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
    loop = false,
    onOneShotEnd,
    class: className = '',
    style = '',
  }: Props = $props();

  let frameIndex = $state(0);
  let restUntil = $state(0);
  let oneShotFired = false;
  let clockResetVersion = $state(0);

  // Sync state changes
  $effect(() => {
    const row = animationFor(pet, currentState) ?? animationFor(pet, 'idle');
    oneShotFired = false;
    restUntil = 0;
    // We don't have a perfect "prev" easily without another variable,
    // but for now, we'll just reset or keep frame if it makes sense.
    // In React it checked if it's the same row/atlas to carry over.
    // Here we'll simplify: reset frame if state changes significantly.
    frameIndex = 0;
    clockResetVersion += 1;
  });

  $effect(() => {
    let raf: number;
    let acc = 0;
    let last = performance.now();
    let localClockVersion = clockResetVersion;
    let cancelled = false;

    const tick = (now: number) => {
      if (cancelled) return;

      if (localClockVersion !== clockResetVersion) {
        localClockVersion = clockResetVersion;
        acc = 0;
        last = now;
      }

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

      const frameMs = 1000 / fpsFor(pet, currentState);
      const dt = now - last;
      last = now;

      acc = Math.min(acc + dt, frameMs * 1.5);

      while (acc >= frameMs) {
        if (restUntil > 0) break;
        acc -= frameMs;

        const row = animationFor(pet, currentState) ?? animationFor(pet, 'idle');
        if (!row) continue;

        const next = frameIndex + 1;
        const isOneShot = pet.oneShot.has(currentState);

        if (isOneShot && !loop) {
          if (next >= row.frames) {
            if (!oneShotFired) {
              oneShotFired = true;
              if (onOneShotEnd) onOneShotEnd();
            }
            frameIndex = row.frames - 1;
          } else {
            frameIndex = next;
          }
        } else {
          if (next >= row.frames) {
            const restMs = loopRestMsFor(pet, currentState);
            if (restMs > 0) {
              restUntil = now + restMs;
              frameIndex = row.frames - 1;
            } else {
              frameIndex = 0;
            }
          } else {
            frameIndex = next;
          }
        }
      }

      raf = requestAnimationFrame(tick);
    };

    raf = requestAnimationFrame(tick);

    return () => {
      cancelled = true;
      cancelAnimationFrame(raf);
    };
  });

  const row = $derived(animationFor(pet, currentState) ?? animationFor(pet, 'idle'));
  const frame = $derived(row ? Math.min(frameIndex, row.frames - 1) : 0);
  const offsetCol = $derived(row?.offsetCol ?? 0);
  const aspect = $derived(pet.atlas.cellH / pet.atlas.cellW);

  const renderW = $derived(Math.max(1, Math.round(size)));
  const renderH = $derived(Math.max(1, Math.round(size * aspect)));
  const totalW = $derived(renderW * pet.atlas.cols);
  const totalH = $derived(renderH * pet.atlas.rows);
  const bgX = $derived(row ? -(offsetCol + frame) * renderW : 0);
  const bgY = $derived(row ? -row.row * renderH : 0);

  const rowScale = $derived(row?.displayScale ?? 1);
  const transform = $derived.by(() => {
    const parts: string[] = [];
    if (row?.flipX) parts.push('scaleX(-1)');
    if (rowScale !== 1) parts.push(`scale(${rowScale})`);
    return parts.length ? parts.join(' ') : undefined;
  });
</script>

{#if row}
  <div
    class={className}
    style:width="{renderW}px"
    style:height="{renderH}px"
    style:background-image="url({pet.spritesheetUrl})"
    style:background-repeat="no-repeat"
    style:background-size="{totalW}px {totalH}px"
    style:background-position="{bgX}px {bgY}px"
    style:image-rendering={pet.imageRendering}
    style:will-change="background-position"
    style:transform={transform}
    style:transform-origin={rowScale !== 1 ? 'bottom center' : undefined}
    style:overflow="visible"
    {style}
  ></div>
{/if}
