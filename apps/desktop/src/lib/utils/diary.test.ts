import { describe, expect, it } from 'vitest';
import {
  appendDiary,
  DIARY_CAP,
  type DiaryDaySummary,
  type DiaryEntry,
  freshCounters,
  greetingFor,
  lastDaySummary,
  localDayOf,
  SUMMARY_VARIANTS,
  sanitizeDiary,
  sanitizeDiaryDay,
  settleDay,
  summaryVariant,
} from './diary';

const summary = (day: string, agentTasks = 3): DiaryDaySummary => ({
  kind: 'day',
  day,
  at: Date.parse(day),
  agentTasks,
  meals: 1,
  coinsEarned: 40,
});

describe('settleDay', () => {
  it('folds counters into a summary carrying day and counts', () => {
    const c = { date: '2026-07-08', agentTasks: 7, meals: 3, coinsEarned: 140 };
    const s = settleDay(c, 123);
    expect(s).toEqual({
      kind: 'day',
      day: '2026-07-08',
      at: 123,
      agentTasks: 7,
      meals: 3,
      coinsEarned: 140,
    });
  });

  it('returns null on an all-zero day (quiet days leave a blank)', () => {
    expect(settleDay(freshCounters('2026-07-08'), 1)).toBeNull();
  });

  it('settles a day with only meals (a pet-mode day has no agent tasks)', () => {
    const s = settleDay({ date: '2026-07-08', agentTasks: 0, meals: 2, coinsEarned: 0 }, 1);
    expect(s?.meals).toBe(2);
    expect(s?.agentTasks).toBe(0);
  });
});

describe('appendDiary', () => {
  it('appends newest-last', () => {
    const d = appendDiary([], summary('2026-07-08'));
    expect(d).toHaveLength(1);
    expect(d[0].day).toBe('2026-07-08');
  });

  it('trims oldest past the cap', () => {
    let diary: DiaryEntry[] = [];
    for (let i = 0; i < DIARY_CAP + 10; i++) {
      diary = appendDiary(diary, { kind: 'perfect_day', day: '2026-01-01', at: i });
    }
    expect(diary).toHaveLength(DIARY_CAP);
    expect(diary[0].at).toBe(10); // the 10 oldest were dropped
  });

  it('never trims the adopted entry', () => {
    let diary: DiaryEntry[] = [{ kind: 'adopted', day: '2025-01-01', at: 0 }];
    for (let i = 0; i < DIARY_CAP + 5; i++) {
      diary = appendDiary(diary, { kind: 'perfect_day', day: '2026-01-01', at: i + 1 });
    }
    expect(diary).toHaveLength(DIARY_CAP);
    expect(diary[0].kind).toBe('adopted');
  });
});

describe('lastDaySummary', () => {
  it('finds the newest day summary behind trailing moments', () => {
    const diary: DiaryEntry[] = [
      summary('2026-07-07'),
      summary('2026-07-08'),
      { kind: 'egg_found', day: '2026-07-09', at: 5 },
    ];
    expect(lastDaySummary(diary)?.day).toBe('2026-07-08');
  });

  it('returns null when the diary has no summaries', () => {
    expect(lastDaySummary([{ kind: 'adopted', day: '2026-07-09', at: 1 }])).toBeNull();
  });
});

describe('greetingFor', () => {
  it("carries yesterday's task count", () => {
    const g = greetingFor('morning', summary('2026-07-08', 7), '2026-07-09');
    expect(g).toEqual({ part: 'morning', tasks: 7 });
  });

  it('suppresses the summary line when the last summary is older than yesterday', () => {
    expect(greetingFor('morning', summary('2026-07-05', 7), '2026-07-09').tasks).toBe(0);
  });

  it('suppresses the summary line with no summary or zero tasks', () => {
    expect(greetingFor('night', null, '2026-07-09').tasks).toBe(0);
    expect(greetingFor('day', summary('2026-07-08', 0), '2026-07-09').tasks).toBe(0);
  });
});

describe('summaryVariant', () => {
  it('is deterministic and in range', () => {
    const v = summaryVariant('2026-07-08');
    expect(v).toBe(summaryVariant('2026-07-08'));
    expect(v).toBeGreaterThanOrEqual(0);
    expect(v).toBeLessThan(SUMMARY_VARIANTS);
  });

  it('varies across days (at least two variants over a month)', () => {
    const days = Array.from({ length: 30 }, (_, i) =>
      summaryVariant(`2026-07-${String(i + 1).padStart(2, '0')}`),
    );
    expect(new Set(days).size).toBeGreaterThan(1);
  });
});

describe('localDayOf', () => {
  it('formats a local calendar date', () => {
    const at = new Date(2026, 6, 9, 15, 30).getTime(); // July 9, local
    expect(localDayOf(at)).toBe('2026-07-09');
  });
});

describe('sanitizeDiary', () => {
  it('keeps valid entries and drops structural garbage', () => {
    const raw = [
      summary('2026-07-08'),
      { kind: 'egg_hatched', day: '2026-07-09', at: 2, ref: 'riffi' },
      { kind: 'evolution', day: 'not-a-day', at: 3 }, // bad day
      { kind: '', day: '2026-07-09', at: 4 }, // empty kind
      { kind: 'adopted', day: '2026-07-09' }, // missing at
      'garbage',
      null,
    ];
    const out = sanitizeDiary(raw);
    expect(out).toHaveLength(2);
    expect((out[1] as { ref?: string }).ref).toBe('riffi');
  });

  it('keeps unknown moment kinds (a newer build wrote them; UI skips them)', () => {
    const out = sanitizeDiary([{ kind: 'from_the_future', day: '2026-07-09', at: 1 }]);
    expect(out).toHaveLength(1);
    expect(out[0].kind).toBe('from_the_future');
  });

  it('coerces day-summary counters sane', () => {
    const out = sanitizeDiary([
      { kind: 'day', day: '2026-07-08', at: 1, agentTasks: -3, meals: 'x', coinsEarned: 2.7 },
    ]);
    expect(out[0]).toMatchObject({ agentTasks: 0, meals: 0, coinsEarned: 2 });
  });

  it('returns [] for non-arrays and caps oversized input', () => {
    expect(sanitizeDiary({ evil: true })).toEqual([]);
    const big = Array.from({ length: DIARY_CAP + 50 }, (_, i) => ({
      kind: 'perfect_day',
      day: '2026-01-01',
      at: i,
    }));
    expect(sanitizeDiary(big)).toHaveLength(DIARY_CAP);
  });
});

describe('sanitizeDiaryDay', () => {
  it('round-trips valid counters', () => {
    const c = { date: '2026-07-09', agentTasks: 2, meals: 1, coinsEarned: 40 };
    expect(sanitizeDiaryDay(c)).toEqual(c);
  });

  it('rejects corrupt shapes', () => {
    expect(sanitizeDiaryDay(null)).toBeNull();
    expect(sanitizeDiaryDay('2026-07-09')).toBeNull();
    expect(sanitizeDiaryDay({ date: 'yesterday' })).toBeNull();
  });

  it('coerces corrupt counter values instead of rejecting the day', () => {
    expect(sanitizeDiaryDay({ date: '2026-07-09', agentTasks: Infinity, meals: -1 })).toEqual({
      date: '2026-07-09',
      agentTasks: 0,
      meals: 0,
      coinsEarned: 0,
    });
  });
});
