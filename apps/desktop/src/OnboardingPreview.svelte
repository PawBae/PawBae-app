<script lang="ts">
  import { onMount } from 'svelte';
  import { locale, waitLocale } from 'svelte-i18n';
  import Onboarding from './lib/components/Onboarding.svelte';

  const isWindows = typeof navigator !== 'undefined' && navigator.userAgent.includes('Windows');
  let ready = $state(false);

  onMount(async () => {
    const requestedLocale = new URLSearchParams(window.location.search).get('lang');
    if (requestedLocale === 'zh' || requestedLocale === 'en') locale.set(requestedLocale);
    await waitLocale();
    ready = true;
  });
</script>

{#if ready}
  <Onboarding open {isWindows} onComplete={() => Promise.resolve()} />
{/if}
