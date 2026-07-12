import './lib/i18n';
import { mount, tick, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import HomePreview from './HomePreview.svelte';
import mainSource from './main.ts?raw';

let component: object | null = null;

afterEach(async () => {
  if (component) await unmount(component);
  component = null;
  document.body.innerHTML = '';
  window.history.replaceState({}, '', '/');
});

async function mountPreview(search: string) {
  window.history.replaceState({}, '', `/${search}`);
  component = mount(HomePreview, { target: document.body });
  await vi.waitFor(() => {
    expect(document.querySelector('[data-home-shell]')).not.toBeNull();
  });
}

function control(name: 'lang' | 'theme' | 'pet' | 'state'): HTMLSelectElement {
  const select = document.querySelector<HTMLSelectElement>(`[data-preview-control="${name}"]`);
  if (!select) throw new Error(`Missing ${name} preview control`);
  return select;
}

describe('HomePreview', () => {
  it('validates query controls and keeps tooling outside the product shell', async () => {
    await mountPreview('?home-preview&lang=fr&theme=neon&pet=unknown&state=unsupported');

    expect(control('lang').value).toBe('en');
    expect(control('theme').value).toBe('system');
    expect(control('pet').value).toBe('muru');
    expect(control('state').value).toBe('idle');
    expect([...control('state').options].map(({ value }) => value)).toEqual([
      'idle',
      'working',
      'waiting',
      'compacting',
      'request',
      'hosting',
      'away',
      'memory',
      'offline',
      'visitor-offline',
      'realtime-degraded',
      'realtime-reconnecting',
    ]);

    const toolbar = document.querySelector('[data-preview-toolbar]');
    const shell = document.querySelector('[data-home-shell]');
    expect(toolbar?.contains(shell)).toBe(false);
    expect(document.querySelector('[data-preview-stage]')?.contains(shell)).toBe(true);
  });

  it('uses authorized 30-minute visit fixtures and changes scenarios locally', async () => {
    await mountPreview('?home-preview&lang=en&theme=light&pet=muru&state=request');

    expect(document.querySelector('[data-home-event="visit-request"]')).not.toBeNull();
    document.querySelector<HTMLButtonElement>('[data-action="accept-visit"]')?.click();
    await tick();

    expect(control('state').value).toBe('hosting');
    expect(window.location.search).toContain('state=hosting');
    expect(document.querySelector('[data-guest-pet]')).not.toBeNull();
    expect(document.querySelector('[data-visit-ends]')?.textContent).toContain('30');

    control('state').value = 'away';
    control('state').dispatchEvent(new Event('change', { bubbles: true }));
    await tick();

    expect(document.querySelector('[data-away-state]')).not.toBeNull();
    expect(document.querySelector('[data-local-pet]')).toBeNull();
  });

  it('exposes the request preview in native keyboard tab order with button activation handlers', async () => {
    await mountPreview('?home-preview&lang=en&theme=dark&pet=solu&state=request');

    const focusable = [
      ...document.querySelectorAll<HTMLSelectElement | HTMLButtonElement>(
        'select:not(:disabled), button:not(:disabled)',
      ),
    ].filter((element) => element.tabIndex === 0);
    const controlId = (element: HTMLSelectElement | HTMLButtonElement): string => {
      if (element instanceof HTMLSelectElement) {
        return `preview:${element.dataset.previewControl}`;
      }
      if (element.dataset.themeChoice) return `theme:${element.dataset.themeChoice}`;
      if (element.classList.contains('settings-action')) return 'home:settings';
      if (element.dataset.dock) return `dock:${element.dataset.dock}`;
      return `action:${element.dataset.action}`;
    };

    expect(focusable).toHaveLength(17);
    expect(focusable.map(controlId)).toEqual([
      'preview:lang',
      'preview:theme',
      'preview:pet',
      'preview:state',
      'theme:system',
      'theme:light',
      'theme:dark',
      'home:settings',
      'dock:friends',
      'dock:plaza',
      'dock:album',
      'action:delay-visit',
      'action:accept-visit',
      'action:feed',
      'action:gift',
      'action:diary',
      'action:send-to-desktop',
    ]);
    expect(focusable.slice(0, 4).every((element) => element instanceof HTMLSelectElement)).toBe(
      true,
    );
    expect(
      focusable
        .slice(4)
        .every((element) => element instanceof HTMLButtonElement && element.type === 'button'),
    ).toBe(true);

    document.querySelector<HTMLButtonElement>('[data-action="accept-visit"]')?.click();
    await tick();

    expect(window.location.search).toContain('state=hosting');
    expect(document.querySelector('[data-home-event="visit-request"]')).toBeNull();
    expect(document.querySelectorAll('[data-guest-pet]')).toHaveLength(1);
  });

  it('renders one localized memory-ready event and keeps opening it inside the preview', async () => {
    await mountPreview('?home-preview&lang=zh&theme=light&pet=muru&state=memory');

    const events = document.querySelectorAll('[data-home-event]');
    const memoryEvent = document.querySelector('[data-home-event="memory-ready"]');
    const openMemory = memoryEvent?.querySelector<HTMLButtonElement>('[data-action="open-memory"]');

    expect(events).toHaveLength(1);
    await vi.waitFor(() => {
      expect(memoryEvent?.textContent).toContain('共同记忆已整理好');
      expect(openMemory?.textContent).toContain('打开记忆');
    });

    openMemory?.click();
    await tick();

    expect(document.querySelector('[data-preview-toolbar]')?.textContent).toContain(
      'Opened local preview memory preview-memory-played-together.',
    );
    expect(window.location.search).toContain('state=memory');
  });

  it('applies validated language, theme, pet, and hosted state selections', async () => {
    await mountPreview('?home-preview&lang=zh&theme=dark&pet=riffi&state=hosting');

    expect(control('lang').value).toBe('zh');
    expect(control('theme').value).toBe('dark');
    expect(control('pet').value).toBe('riffi');
    expect(control('state').value).toBe('hosting');
    expect(document.querySelector('[data-theme="dark"]')).not.toBeNull();
    expect(document.querySelector('[data-local-pet] [data-pet-id="riffi"]')).not.toBeNull();
    expect(document.querySelector('[data-guest-pet] [data-pet-id="solu"]')).not.toBeNull();
    expect(document.querySelector('[aria-label="PawBae 小家"]')).not.toBeNull();
    expect(document.querySelector('.identity-capsule')?.textContent).toContain('雷栗');
    expect(
      document.querySelector('[data-local-pet] [role="img"]')?.getAttribute('aria-label'),
    ).toBe('雷栗');
    expect(
      document.querySelector('[data-guest-pet] [role="img"]')?.getAttribute('aria-label'),
    ).toBe('小煦');
  });

  it('provides URL-addressable visitor-offline and realtime degraded/recovery previews', async () => {
    await mountPreview('?home-preview&lang=en&theme=light&pet=muru&state=visitor-offline');
    expect(control('state').value).toBe('visitor-offline');
    expect(document.querySelector('[data-visitor-status]')?.textContent).toContain(
      "owner's agent is resting",
    );
    document.querySelector<HTMLButtonElement>('[data-dock="friends"]')?.click();
    await tick();
    expect(document.querySelector('[data-friend="preview-friend-momo"]')?.textContent).toContain(
      'Hosting a visitor',
    );

    control('state').value = 'realtime-degraded';
    control('state').dispatchEvent(new Event('change', { bubbles: true }));
    await tick();
    expect(window.location.search).toContain('state=realtime-degraded');
    expect(document.querySelector('[data-realtime-state="degraded"]')).not.toBeNull();
    expect(document.querySelector('[data-guest-pet]')).not.toBeNull();
    expect(document.querySelector('[data-visitor-status]')?.textContent).toContain('Agent idle');

    control('state').value = 'realtime-reconnecting';
    control('state').dispatchEvent(new Event('change', { bubbles: true }));
    await tick();
    expect(document.querySelector('[data-realtime-state="reconnecting"]')).not.toBeNull();
    expect(document.querySelector('[data-guest-pet]')).not.toBeNull();

    control('state').value = 'idle';
    control('state').dispatchEvent(new Event('change', { bubbles: true }));
    await tick();
    expect(document.querySelector('[data-realtime-state]')).toBeNull();
  });

  it('loads demo fixtures only from the explicit development Home branch', () => {
    expect(mainSource).not.toMatch(/^import HomePreview/m);
    expect(mainSource).toContain("if (import.meta.env.DEV && preview === 'home')");
    expect(mainSource).toContain("await import('./HomePreview.svelte')");
  });
});
