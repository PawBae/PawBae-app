// The forgiving-streak contract: check-in on the day's first task, shields bridge
// gaps (and survive a restart), the board rolls at the date line, and display
// never promises what markTask won't honor.
import { describe, expect, it } from 'vitest';
import {
  BOARD_TASKS,
  type BoardState,
  displayStreak,
  initialBoardState,
  markTask,
  SHIELD_CAP,
  sanitizeBoardDone,
  streakBucket,
} from './daily-board';

const D1 = '2026-07-08';
const D2 = '2026-07-09';
const D3 = '2026-07-10';
const D5 = '2026-07-12';

function state(over: Partial<BoardState> = {}): BoardState {
  return { ...initialBoardState(), ...over };
}

describe('markTask', () => {
  it('first mark ever checks in and starts the streak at 1', () => {
    const r = markTask(state(), 'headpat', D1);
    expect(r).toMatchObject({ taskCompleted: true, checkedIn: true, perfectDay: false });
    expect(r.state).toEqual({
      boardDate: D1,
      boardDone: ['headpat'],
      streak: 1,
      streakDate: D1,
      shields: 0,
    });
  });

  it('dedupes a repeat mark as a full no-op', () => {
    const first = markTask(state(), 'gift', D1).state;
    const r = markTask(first, 'gift', D1);
    expect(r.taskCompleted).toBe(false);
    expect(r.checkedIn).toBe(false);
    expect(r.state).toEqual(first);
  });

  it('second task of the day ticks the board without touching the streak', () => {
    const first = markTask(state(), 'gift', D1).state;
    const r = markTask(first, 'meal', D1);
    expect(r.taskCompleted).toBe(true);
    expect(r.checkedIn).toBe(false);
    expect(r.state.streak).toBe(1);
    expect(r.state.boardDone).toEqual(['gift', 'meal']);
  });

  it('consecutive-day check-in extends the streak and rolls the board', () => {
    const day1 = markTask(state(), 'meal', D1).state;
    const r = markTask(day1, 'agent', D2);
    expect(r.checkedIn).toBe(true);
    expect(r.state.streak).toBe(2);
    expect(r.state.boardDone).toEqual(['agent']); // yesterday's ticks gone
  });

  it('a one-day gap is bridged by a banked shield', () => {
    const r = markTask(state({ streak: 9, streakDate: D1, shields: 1 }), 'gift', D3);
    expect(r).toMatchObject({ checkedIn: true, shieldsSpent: 1 });
    expect(r.state.streak).toBe(10);
    expect(r.state.shields).toBe(0);
  });

  it('a gap beyond the shields restarts at 1 and KEEPS the shields', () => {
    const r = markTask(state({ streak: 30, streakDate: D1, shields: 2 }), 'gift', D5); // 3-day gap
    expect(r).toMatchObject({ checkedIn: true, shieldsSpent: 0 });
    expect(r.state.streak).toBe(1);
    expect(r.state.shields).toBe(2);
  });

  it('mints a shield at each 7-day multiple, capped at SHIELD_CAP', () => {
    const minted = markTask(state({ streak: 6, streakDate: D1 }), 'gift', D2);
    expect(minted.shieldEarned).toBe(true);
    expect(minted.state).toMatchObject({ streak: 7, shields: 1 });

    const capped = markTask(state({ streak: 13, streakDate: D1, shields: SHIELD_CAP }), 'gift', D2);
    expect(capped.shieldEarned).toBe(false);
    expect(capped.state.shields).toBe(SHIELD_CAP);
  });

  it('completing the last task flags perfectDay exactly once', () => {
    let s = state();
    const last = BOARD_TASKS.length - 1;
    BOARD_TASKS.forEach(({ id }, i) => {
      const r = markTask(s, id, D1);
      expect(r.perfectDay).toBe(i === last);
      s = r.state;
    });
  });

  it('clock regression freezes the streak instead of resetting it', () => {
    const r = markTask(state({ streak: 5, streakDate: D3 }), 'gift', D1);
    expect(r.taskCompleted).toBe(true);
    expect(r.checkedIn).toBe(false);
    expect(r.state.streak).toBe(5);
    expect(r.state.streakDate).toBe(D3);
  });

  it('rejects an unparseable today outright', () => {
    const before = state({ streak: 3, streakDate: D1 });
    const r = markTask(before, 'gift', 'not-a-date');
    expect(r.taskCompleted).toBe(false);
    expect(r.state).toEqual(before);
  });

  it('sanitizes corrupt stored numbers instead of propagating them', () => {
    const r = markTask(state({ streak: Number.NaN, streakDate: D1, shields: 99 }), 'gift', D2);
    expect(r.state.streak).toBe(1); // NaN → 0 → +1
    expect(r.state.shields).toBeLessThanOrEqual(SHIELD_CAP);
  });
});

describe('displayStreak', () => {
  it('shows the stored streak while alive, salvageable, or under clock regression', () => {
    expect(displayStreak(state({ streak: 5, streakDate: D1 }), D1)).toBe(5); // today
    expect(displayStreak(state({ streak: 5, streakDate: D1 }), D2)).toBe(5); // yesterday
    expect(displayStreak(state({ streak: 5, streakDate: D1, shields: 1 }), D3)).toBe(5); // shield-savable
    expect(displayStreak(state({ streak: 5, streakDate: D3 }), D1)).toBe(5); // regression
  });

  it('shows 0 when broken beyond the shields or never started', () => {
    expect(displayStreak(state(), D1)).toBe(0);
    expect(displayStreak(state({ streak: 5, streakDate: D1 }), D3)).toBe(0); // gap 1, no shield
    expect(displayStreak(state({ streak: 5, streakDate: D1, shields: 2 }), D5)).toBe(0); // gap 3 > 2
  });
});

describe('sanitizeBoardDone / streakBucket', () => {
  it('filters unknown ids and dedupes', () => {
    expect(sanitizeBoardDone(['gift', 'nope', 'gift', 7, 'meal'])).toEqual(['gift', 'meal']);
    expect(sanitizeBoardDone('corrupt')).toEqual([]);
  });

  it('buckets streaks anonymously', () => {
    expect([1, 3, 7, 30].map(streakBucket)).toEqual(['1-2', '3-6', '7-29', '30+']);
  });
});
