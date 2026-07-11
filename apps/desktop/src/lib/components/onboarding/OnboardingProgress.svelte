<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { ONBOARDING_STEPS, type OnboardingStep } from '../../utils/onboarding';

  let { step }: { step: OnboardingStep } = $props();

  const currentIndex = $derived(ONBOARDING_STEPS.indexOf(step));
</script>

<nav class="progress" aria-label={$_('onboarding.progress.current', {
  values: {
    current: currentIndex + 1,
    total: ONBOARDING_STEPS.length,
    name: $_(`onboarding.progress.${step}`),
  },
})}>
  <ol>
    {#each ONBOARDING_STEPS as item, index}
      <li class:complete={index < currentIndex} class:current={item === step}>
        <span class="marker" aria-hidden="true">{index < currentIndex ? '✓' : index + 1}</span>
        <span class="label">{$_(`onboarding.progress.${item}`)}</span>
      </li>
    {/each}
  </ol>
  <span class="sr-only" aria-live="polite">
    {$_('onboarding.progress.current', {
      values: {
        current: currentIndex + 1,
        total: ONBOARDING_STEPS.length,
        name: $_(`onboarding.progress.${step}`),
      },
    })}
  </span>
</nav>

<style>
  .progress { min-width: 0; }
  ol {
    display: flex;
    align-items: center;
    gap: 0;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--ob-text-muted);
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
  }
  li:not(:last-child)::after {
    content: '';
    width: clamp(20px, 4vw, 58px);
    height: 1px;
    margin: 0 12px;
    background: var(--ob-border);
  }
  .marker {
    display: grid;
    width: 24px;
    height: 24px;
    place-items: center;
    border: 1px solid var(--ob-border-strong);
    border-radius: 50%;
    background: var(--ob-surface);
    color: var(--ob-text-muted);
    font-size: 11px;
  }
  .complete, .current { color: var(--ob-text); }
  .complete .marker {
    border-color: var(--ob-ink);
    background: var(--ob-ink);
    color: var(--ob-surface);
  }
  .current .marker {
    border-color: var(--ob-action);
    background: var(--ob-action);
    color: var(--ob-action-text);
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
  @media (max-width: 720px) {
    .label { display: none; }
    li:not(:last-child)::after { width: 18px; margin: 0 8px; }
  }
</style>
