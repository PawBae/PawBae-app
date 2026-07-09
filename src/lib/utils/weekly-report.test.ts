// The assembler is the share card's honesty layer: window alignment across
// sources, the ledger-cap "N+" flag, zero-week resilience, and the per-language
// number formatting all live here.
import { describe, expect, it } from 'vitest';
import type { ClaudeDailyStats, ClaudeStats, CoinAward } from '../types';
import { LEDGER_RECENT_CAP } from './rewards';
import { assembleWeeklyReport, formatTokens, WEEK_DAYS } from './weekly-report';

const NOW = Date.parse('2026-07-08T12:00:00Z');

function day(date: string, over: Partial<ClaudeDailyStats> = {}): ClaudeDailyStats {
  return {
    date,
    input_tokens: 0,
    output_tokens: 0,
    cache_read_tokens: 0,
    cache_write_tokens: 0,
    messages: 0,
    sessions: 0,
    ...over,
  };
}

/** 14 zero days ending 2026-07-08, like the Rust command emits. */
function fourteenDays(): ClaudeDailyStats[] {
  return Array.from({ length: 14 }, (_, i) =>
    day(new Date(Date.parse('2026-06-25') + i * 86_400_000).toISOString().slice(0, 10)),
  );
}

function stats(days: ClaudeDailyStats[]): ClaudeStats {
  return {
    totalInputTokens: 0,
    totalOutputTokens: 0,
    totalCacheReadTokens: 0,
    totalCacheWriteTokens: 0,
    totalMessages: 0,
    totalSessions: 0,
    dailyStats: days,
  };
}

function award(at: number, source = 'agent_stop'): CoinAward {
  return { source: source as CoinAward['source'], amount: 1, at };
}

function base(over: Partial<Parameters<typeof assembleWeeklyReport>[0]> = {}) {
  return assembleWeeklyReport({
    statsList: [],
    recentAwards: [],
    streak: 0,
    shields: 0,
    stageIndex: 1,
    daysTogether: 45,
    petName: 'Yoonie',
    lang: 'en',
    now: NOW,
    ...over,
  });
}

describe('assembleWeeklyReport', () => {
  it('takes the trailing 7 days and sums input+output across sources by date', () => {
    const a = fourteenDays();
    a[13] = day('2026-07-08', { input_tokens: 100, output_tokens: 50, messages: 3, sessions: 1 });
    const b = fourteenDays();
    b[13] = day('2026-07-08', { input_tokens: 10, output_tokens: 5, messages: 2, sessions: 1 });
    b[7] = day('2026-07-02', { input_tokens: 1000, output_tokens: 0 });

    const r = base({ statsList: [stats(a), stats(b), null] });
    expect(r.dailyTokens).toHaveLength(WEEK_DAYS);
    expect(r.dailyTokens[6]).toBe(165); // 7-08 merged across sources
    expect(r.dailyTokens[0]).toBe(1000); // 7-02 is the window's first day
    expect(r.totalTokens).toBe(1165);
    expect(r.messages).toBe(5);
    expect(r.sessions).toBe(2);
    expect(r.weekLabel).toBe('7.2 – 7.8');
  });

  it('excludes cache tokens from the hero number', () => {
    const a = fourteenDays();
    a[13] = day('2026-07-08', {
      input_tokens: 10,
      cache_read_tokens: 9_999,
      cache_write_tokens: 500,
    });
    expect(base({ statsList: [stats(a)] }).totalTokens).toBe(10);
  });

  it('renders a zero week instead of failing (all sources null)', () => {
    const r = base({ statsList: [null, null, null] });
    expect(r.totalTokens).toBe(0);
    expect(r.dailyTokens).toEqual([0, 0, 0, 0, 0, 0, 0]);
    expect(r.weekLabel).toBe('');
  });

  it('counts only in-window agent_stop awards and flags a full ledger', () => {
    const awards = [
      award(NOW - 1000),
      award(NOW - 6 * 86_400_000),
      award(NOW - 8 * 86_400_000), // outside the window
      award(NOW - 1000, 'daily_gift'), // wrong source
    ];
    const r = base({ recentAwards: awards });
    expect(r.agentTasks).toBe(2);
    expect(r.tasksCapped).toBe(false);

    const full = Array.from({ length: LEDGER_RECENT_CAP }, (_, i) => award(NOW - i * 1000));
    expect(base({ recentAwards: full }).tasksCapped).toBe(true);
  });

  it('sanitizes corrupt daily numbers to zero', () => {
    const a = fourteenDays();
    a[13] = day('2026-07-08', {
      input_tokens: Number.NaN,
      output_tokens: -50 as number,
      messages: Number.POSITIVE_INFINITY,
    });
    const r = base({ statsList: [stats(a)] });
    expect(r.totalTokens).toBe(0);
    expect(r.messages).toBe(0);
  });
});

describe('formatTokens', () => {
  it('formats en with K/M/B, one decimal under 10 units', () => {
    expect(formatTokens(0, 'en')).toBe('0');
    expect(formatTokens(999, 'en')).toBe('999');
    expect(formatTokens(1_234, 'en')).toBe('1.2K');
    expect(formatTokens(2_400_000, 'en')).toBe('2.4M');
    expect(formatTokens(38_000_000, 'en')).toBe('38M');
    expect(formatTokens(1_100_000_000, 'en')).toBe('1.1B');
  });

  it('formats zh with 万/亿', () => {
    expect(formatTokens(9_999, 'zh')).toBe('9999');
    expect(formatTokens(24_000, 'zh')).toBe('2.4万');
    expect(formatTokens(2_400_000, 'zh')).toBe('240万');
    expect(formatTokens(110_000_000, 'zh')).toBe('1.1亿');
  });

  it('treats corrupt input as zero', () => {
    expect(formatTokens(Number.NaN, 'en')).toBe('0');
    expect(formatTokens(-5, 'zh')).toBe('0');
  });
});
