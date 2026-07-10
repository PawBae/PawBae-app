// Daily-board store glue: board ticks from the real entry points (headpat, feed,
// gift), the perfect-day payout + celebration, persistence keys, and the legacy
// gift-streak migration. The streak/shield MATH is covered by utils/daily-board.test.ts.
import { beforeEach, describe, expect, it, vi } from 'vitest';

const harness = vi.hoisted(() => ({
  data: new Map<string, unknown>(),
}));

vi.mock('@tauri-apps/plugin-store', () => ({
  load: async () => ({
    get: async (key: string) => harness.data.get(key),
    set: async (key: string, value: unknown) => {
      harness.data.set(key, value);
    },
    save: async () => {},
  }),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: async () => () => {},
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: async () => null,
}));

import { PERFECT_DAY_COINS } from '../utils/daily-board';
import { dailyGiftAmount, initialRewardState } from '../utils/rewards';
import { petStore } from './pet.svelte';

function dateStr(ms: number): string {
  const d = new Date(ms);
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  return `${d.getFullYear()}-${mm}-${dd}`;
}
const yesterday = () => dateStr(Date.now() - 86_400_000);

beforeEach(() => {
  harness.data.clear();
  petStore.rewards = initialRewardState();
  petStore.achievements = {};
  petStore.evolutionStageSeen = 0;
  petStore.celebrations = [];
  petStore.loadPetData(petStore.defaultPetData());
});

describe('board ticks from real entry points', () => {
  it('a headpat ticks the board and checks in the day', () => {
    petStore.applyHeadpat();
    expect(petStore.boardDoneToday).toEqual(['headpat']);
    expect(petStore.petData.streak).toBe(1);
    expect(petStore.streakLive).toBe(1);
  });

  it('repeat headpats do not re-tick or re-advance', () => {
    petStore.applyHeadpat();
    petStore.applyHeadpat();
    expect(petStore.boardDoneToday).toEqual(['headpat']);
    expect(petStore.petData.streak).toBe(1);
  });

  it('a manual feed ticks the meal task', () => {
    petStore.loadPetData({ ...petStore.defaultPetData(), coins: 10, hunger: 50 });
    expect(petStore.applyFeed()).toBe(true);
    expect(petStore.boardDoneToday).toContain('meal');
  });

  it('an agent meal ticks the same meal task', () => {
    petStore.applyTokenMeal({ tier: 'snack', restore: 5, tokens: 2_000 });
    expect(petStore.boardDoneToday).toContain('meal');
  });
});

describe('unified streak through the gift claim', () => {
  it('extends a yesterday-checked-in streak and pays its bonus', () => {
    petStore.loadPetData({
      ...petStore.defaultPetData(),
      coins: 0,
      lastDailyGift: yesterday(),
      streak: 2,
      streakDate: yesterday(),
    });
    expect(petStore.nextGiftAmount).toBe(dailyGiftAmount(3));
    expect(petStore.claimDailyGift()).toBe(true);
    expect(petStore.petData.streak).toBe(3);
    expect(petStore.petData.coins).toBe(dailyGiftAmount(3));
    expect(petStore.boardDoneToday).toContain('gift');
  });

  it('quietly restarts a long-broken streak at 1', () => {
    petStore.loadPetData({
      ...petStore.defaultPetData(),
      lastDailyGift: '2020-01-01',
      streak: 30,
      streakDate: '2020-01-01',
    });
    expect(petStore.streakLive).toBe(0);
    expect(petStore.claimDailyGift()).toBe(true);
    expect(petStore.petData.streak).toBe(1);
    expect(petStore.petData.coins).toBe(dailyGiftAmount(1));
  });
});

describe('perfect day', () => {
  it('completing all four tasks pays the bonus and queues one celebration', () => {
    petStore.loadPetData({ ...petStore.defaultPetData(), coins: 100, hunger: 50 });
    petStore.applyHeadpat();
    petStore.applyFeed(); // meal
    petStore.claimDailyGift();
    petStore.markBoardTask('agent');
    expect(petStore.boardDoneToday).toHaveLength(4);
    expect(petStore.celebrations).toContainEqual({ kind: 'perfect_day' });
    expect(petStore.rewards.totals.task_board).toMatchObject({
      earned: PERFECT_DAY_COINS,
      count: 1,
    });
    // A repeat mark can never double-pay.
    petStore.markBoardTask('agent');
    expect(petStore.rewards.totals.task_board.count).toBe(1);
  });
});

describe('persistence + migration', () => {
  it('persists the board slice under its own keys', async () => {
    petStore.applyHeadpat();
    // persistRewards chains onto saveInFlight — settle the microtask queue.
    await new Promise((r) => setTimeout(r, 0));
    expect(harness.data.get('streak')).toBe(1);
    expect(harness.data.get('streak_date')).toBe(dateStr(Date.now()));
    expect(harness.data.get('board_done')).toEqual(['headpat']);
  });

  it('seeds the unified streak from a live legacy gift streak (no streak_date key)', async () => {
    harness.data.set('gift_streak', 5);
    harness.data.set('last_daily_gift', yesterday());
    const dispose = await petStore.init();
    expect(petStore.petData.streak).toBe(5);
    expect(petStore.petData.streakDate).toBe(yesterday());
    expect(petStore.streakLive).toBe(5);
    dispose();
  });

  it('does not resurrect a dead legacy streak', async () => {
    harness.data.set('gift_streak', 30);
    harness.data.set('last_daily_gift', '2020-01-01');
    const dispose = await petStore.init();
    expect(petStore.petData.streak).toBe(0);
    expect(petStore.petData.streakDate).toBe('');
    dispose();
  });
});
