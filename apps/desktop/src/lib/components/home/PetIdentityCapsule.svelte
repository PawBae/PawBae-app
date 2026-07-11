<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { SocialHomeModel } from '../../utils/social-home';

  let { model }: { model: SocialHomeModel } = $props();

  const initial = $derived(model.localPet.name.slice(0, 1).toLocaleUpperCase());
</script>

<header class="identity-capsule">
  <span class="pet-initial" aria-hidden="true">{initial}</span>
  <span class="identity-copy">
    <strong>{model.localPet.name}</strong>
    <span>{$_(`home.status.${model.agentState}`)}</span>
  </span>
  <span class="relationship-stats">
    <span aria-label={`${$_('home.metrics.affection')} ${model.affection}`}>
      <span aria-hidden="true">♥</span> {model.affection}
    </span>
    <span aria-label={`${$_('home.metrics.coins')} ${model.coins}`}>
      <span aria-hidden="true">◆</span> {model.coins}
    </span>
  </span>
</header>

<style>
  .identity-capsule {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) auto;
    align-items: center;
    gap: 11px;
    width: min(320px, 100%);
    min-height: 68px;
    padding: 10px 13px 10px 10px;
    border: 1px solid var(--home-border);
    border-radius: 16px;
    background: var(--home-surface);
    color: var(--home-text);
  }

  .pet-initial {
    display: grid;
    width: 44px;
    height: 44px;
    place-items: center;
    border-radius: 50%;
    background: color-mix(in srgb, var(--home-pet-glow) 55%, var(--home-subtle));
    color: var(--home-action);
    font-size: 17px;
    font-weight: 800;
  }

  .identity-copy {
    display: grid;
    min-width: 0;
    gap: 3px;
  }

  .identity-copy strong {
    overflow: hidden;
    font-size: 15px;
    letter-spacing: -0.015em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .identity-copy span {
    overflow: hidden;
    color: var(--home-text-muted);
    font-size: 12px;
    line-height: 1.25;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .relationship-stats {
    display: grid;
    gap: 5px;
    color: var(--home-text-muted);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    font-weight: 700;
    text-align: right;
  }

  .relationship-stats > span:first-child span {
    color: #a94452;
  }

  .relationship-stats > span:last-child span {
    color: var(--home-action);
  }
</style>
