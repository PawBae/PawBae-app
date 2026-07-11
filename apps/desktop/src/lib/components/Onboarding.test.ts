import '../i18n';
import { mount, tick, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import Onboarding from './Onboarding.svelte';
import onboardingSource from './Onboarding.svelte?raw';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

let mounted: object | null = null;

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(undefined);
  const values = new Map<string, string>();
  vi.stubGlobal('localStorage', {
    clear: () => values.clear(),
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
    removeItem: (key: string) => values.delete(key),
  });
});

afterEach(async () => {
  if (mounted) await unmount(mounted);
  mounted = null;
  document.body.innerHTML = '';
  localStorage.clear();
  vi.unstubAllGlobals();
});

describe('Onboarding', () => {
  async function advanceToAgents(onComplete = vi.fn()) {
    mounted = mount(Onboarding, {
      target: document.body,
      props: { open: true, isWindows: false, onComplete },
    });
    await tick();
    document.querySelector<HTMLButtonElement>('[data-action="continue"]')?.click();
    await tick();
    document.querySelector<HTMLButtonElement>('[data-action="skip-github"]')?.click();
    await tick();
    return onComplete;
  }

  it('advances from welcome to an honest unavailable GitHub step', async () => {
    mounted = mount(Onboarding, {
      target: document.body,
      props: {
        open: true,
        isWindows: false,
        onComplete: vi.fn(),
      },
    });

    await tick();
    expect(document.querySelector('[data-step="welcome"]')).not.toBeNull();
    document.querySelector<HTMLButtonElement>('[data-action="continue"]')?.click();
    await tick();
    expect(document.querySelector('[data-step="github"]')).not.toBeNull();
    expect(document.querySelector<HTMLButtonElement>('[data-action="github"]')?.disabled).toBe(
      true,
    );
    expect(document.querySelector('[data-action="skip-github"]')).not.toBeNull();
  });

  it('restores and persists an explicit appearance choice', async () => {
    localStorage.setItem('pawbae-onboarding-theme', 'dark');
    mounted = mount(Onboarding, {
      target: document.body,
      props: {
        open: true,
        isWindows: false,
        onComplete: vi.fn(),
      },
    });

    await tick();
    expect(document.querySelector('.onboarding-overlay')?.getAttribute('data-theme')).toBe('dark');
    document.querySelector<HTMLButtonElement>('[data-theme-choice="light"]')?.click();
    await tick();
    expect(document.querySelector('.onboarding-overlay')?.getAttribute('data-theme')).toBe('light');
    expect(localStorage.getItem('pawbae-onboarding-theme')).toBe('light');
  });

  it('keeps the adoption grid four across at the narrow desktop breakpoint', () => {
    const petGridRules = [...onboardingSource.matchAll(/\.pet-grid\s*\{([^}]*)\}/g)]
      .map((match) => match[1])
      .join('\n');
    expect(petGridRules).toContain('grid-template-columns: repeat(4, minmax(0, 1fr))');
    expect(petGridRules).not.toMatch(/grid-template-columns:\s*repeat\((?!4,)/);
  });

  it('reserves enough narrow-header width for both appearance and setup actions', () => {
    expect(onboardingSource).toMatch(
      /@media \(max-width: 940px\)[\s\S]*?\.topbar\s*\{[^}]*grid-template-columns:\s*100px minmax\(0, 1fr\) 166px/,
    );
  });

  it('cannot leave agent setup or persist a selection while Hook installation is pending', async () => {
    let resolveInstall!: () => void;
    invokeMock.mockReturnValue(new Promise<void>((resolve) => (resolveInstall = resolve)));
    const onComplete = await advanceToAgents();

    document.querySelector<HTMLButtonElement>('[data-agent="claude"] .switch')?.click();
    await tick();

    const forward = document.querySelector<HTMLButtonElement>('[data-action="agents-continue"]');
    expect(forward?.disabled).toBe(true);
    expect(document.querySelector<HTMLButtonElement>('.setup-later')?.disabled).toBe(false);
    forward?.click();
    await tick();
    expect(document.querySelector('[data-step="agents"]')).not.toBeNull();
    expect(onComplete).not.toHaveBeenCalled();

    resolveInstall();
    await tick();
    await tick();
    expect(forward?.disabled).toBe(false);
  });

  it('uses Set up later as an immediate pet-only escape while Hook installation never settles', async () => {
    invokeMock.mockReturnValue(new Promise<void>(() => {}));
    const onComplete = await advanceToAgents();

    document.querySelector<HTMLButtonElement>('[data-agent="claude"] .switch')?.click();
    await tick();
    document.querySelector<HTMLButtonElement>('.setup-later')?.click();
    await tick();

    expect(onComplete).toHaveBeenCalledOnce();
    expect(onComplete).toHaveBeenCalledWith(
      expect.objectContaining({ mode: 'pet', selectedAgents: [], starterPetId: null }),
    );
  });

  it('ignores a stale Hook rejection after Set up later completes', async () => {
    let rejectInstall!: (error: Error) => void;
    invokeMock.mockReturnValue(
      new Promise<void>((_resolve, reject) => {
        rejectInstall = reject;
      }),
    );
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const onComplete = await advanceToAgents();

    document.querySelector<HTMLButtonElement>('[data-agent="claude"] .switch')?.click();
    await tick();
    document.querySelector<HTMLButtonElement>('.setup-later')?.click();
    await tick();
    rejectInstall(new Error('late failure'));
    await tick();
    await tick();

    expect(onComplete).toHaveBeenCalledOnce();
    expect(warn).not.toHaveBeenCalled();
    expect(document.querySelector('[data-agent="claude"]')?.textContent).not.toContain('Retry');
  });

  it('ignores a stale Hook resolution after Set up later completes', async () => {
    let resolveInstall!: () => void;
    invokeMock.mockReturnValue(new Promise<void>((resolve) => (resolveInstall = resolve)));
    const onComplete = await advanceToAgents();

    document.querySelector<HTMLButtonElement>('[data-agent="claude"] .switch')?.click();
    await tick();
    document.querySelector<HTMLButtonElement>('.setup-later')?.click();
    await tick();
    resolveInstall();
    await tick();
    await tick();

    expect(onComplete).toHaveBeenCalledOnce();
    expect(document.querySelector('[data-agent="claude"]')?.textContent).not.toContain('Connected');
  });

  it('clears a connected coding agent when Just keep me company is chosen', async () => {
    const onComplete = await advanceToAgents();
    document.querySelector<HTMLButtonElement>('[data-agent="claude"] .switch')?.click();
    await vi.waitFor(() => {
      expect(document.querySelector('[data-agent="claude"]')?.textContent).toContain('Connected');
    });

    document.querySelector<HTMLButtonElement>('[data-action="pet-only"]')?.click();
    await tick();
    document.querySelector<HTMLButtonElement>('[data-pet-id="muru"]')?.click();
    await tick();
    document.querySelector<HTMLButtonElement>('[data-action="complete-adoption"]')?.click();
    await tick();

    expect(onComplete).toHaveBeenCalledWith(
      expect.objectContaining({ mode: 'pet', selectedAgents: [], starterPetId: 'muru' }),
    );
  });

  it('does not persist or complete with an agent whose Hook installation failed', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    invokeMock.mockRejectedValue(new Error('permission denied'));
    const onComplete = await advanceToAgents();

    document.querySelector<HTMLButtonElement>('[data-agent="claude"] .switch')?.click();
    await vi.waitFor(() => {
      expect(document.querySelector('[data-agent="claude"]')?.textContent).toContain('Retry');
    });

    document.querySelector<HTMLButtonElement>('[data-action="agents-continue"]')?.click();
    await tick();
    expect(document.querySelector('[data-step="adopt"]')).not.toBeNull();
    document.querySelector<HTMLButtonElement>('[data-pet-id="solu"]')?.click();
    await tick();
    document.querySelector<HTMLButtonElement>('[data-action="complete-adoption"]')?.click();
    await tick();

    expect(onComplete).toHaveBeenCalledWith(
      expect.objectContaining({ selectedAgents: [], mode: 'pet', starterPetId: 'solu' }),
    );
  });

  it('uses roving radio focus and arrows to select and focus official pets', async () => {
    await advanceToAgents();
    document.querySelector<HTMLButtonElement>('[data-action="agents-continue"]')?.click();
    await tick();

    const cards = [...document.querySelectorAll<HTMLButtonElement>('[role="radio"]')];
    expect(cards.map((card) => card.tabIndex)).toEqual([0, -1, -1, -1]);
    cards[0].focus();
    cards[0].dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    await tick();

    expect(document.activeElement).toBe(cards[1]);
    expect(cards[1].getAttribute('aria-checked')).toBe('true');
    expect(cards.map((card) => card.tabIndex)).toEqual([-1, 0, -1, -1]);

    cards[1].dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
    await tick();
    expect(document.activeElement).toBe(cards[0]);
    expect(cards[0].getAttribute('aria-checked')).toBe('true');
  });

  it('completes adoption when Enter is pressed on the already selected pet', async () => {
    const onComplete = await advanceToAgents();
    document.querySelector<HTMLButtonElement>('[data-action="agents-continue"]')?.click();
    await tick();

    const solu = document.querySelector<HTMLButtonElement>('[data-pet-id="solu"]');
    solu?.click();
    await tick();
    solu?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await tick();

    expect(onComplete).toHaveBeenCalledOnce();
    expect(onComplete).toHaveBeenCalledWith(expect.objectContaining({ starterPetId: 'solu' }));
  });
});
