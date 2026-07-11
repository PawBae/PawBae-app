import '../../i18n';
import { mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { OFFICIAL_PETS } from '../../utils/onboarding';
import AgentConnectionRow from './AgentConnectionRow.svelte';
import PetAdoptionCard from './PetAdoptionCard.svelte';

const mounted: object[] = [];

function target(): HTMLDivElement {
  const node = document.createElement('div');
  document.body.appendChild(node);
  return node;
}

afterEach(async () => {
  for (const component of mounted.splice(0)) await unmount(component);
  document.body.innerHTML = '';
});

describe('onboarding choice components', () => {
  it('exposes and triggers pet radio selection', () => {
    const onSelect = vi.fn();
    mounted.push(
      mount(PetAdoptionCard, {
        target: target(),
        props: { pet: OFFICIAL_PETS[0], selected: true, tabIndex: 0, onSelect },
      }),
    );
    const radio = document.querySelector<HTMLButtonElement>('[role="radio"]');
    expect(radio?.getAttribute('aria-checked')).toBe('true');
    radio?.click();
    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  it.each([
    { available: false, status: 'idle' as const },
    { available: true, status: 'installing' as const },
  ])('disables unavailable or installing agent switches', ({ available, status }) => {
    mounted.push(
      mount(AgentConnectionRow, {
        target: target(),
        props: {
          id: 'claude',
          selected: false,
          available,
          status,
          error: '',
          onToggle: vi.fn(),
          onRetry: vi.fn(),
        },
      }),
    );
    expect(document.querySelector('[role="switch"]')?.getAttribute('aria-disabled')).toBe('true');
  });
});
