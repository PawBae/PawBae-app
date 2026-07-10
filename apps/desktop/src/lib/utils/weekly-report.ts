// Weekly Paw report assembler (设计: docs/superpowers/specs/
// 2026-07-08-weekly-share-card-design.md). Pure: turns the raw per-source stats
// plus pet state into the flat WeeklyReport the canvas renderer draws — all
// aggregation, windowing, capping, and number formatting happens here where it
// can be unit-tested; the renderer stays dumb.
import type { ClaudeStats, CoinAward } from '../types';
import { LEDGER_RECENT_CAP } from './rewards';

export const WEEK_DAYS = 7;
const DAY_MS = 86_400_000;

export interface WeeklyReport {
  /** Display range, e.g. "7.2 – 7.8" (language-neutral numerics). */
  weekLabel: string;
  /** input+output across all sources, last 7 days (cache excluded — feeding-loop口径). */
  totalTokens: number;
  /** Formatted hero number, e.g. "2.4M" / "240万". */
  totalTokensLabel: string;
  /** Per-day token sums, oldest first, always 7 entries. */
  dailyTokens: number[];
  /** agent_stop completions in the window (ledger-based, best effort). */
  agentTasks: number;
  /** The ledger was full — render agentTasks as "N+" instead of lying. */
  tasksCapped: boolean;
  messages: number;
  sessions: number;
  streak: number;
  shields: number;
  stageIndex: number;
  daysTogether: number;
  petName: string;
}

export interface WeeklyReportInputs {
  /** One ClaudeStats per source; a failed fetch rides along as null/undefined. */
  statsList: (ClaudeStats | null | undefined)[];
  recentAwards: CoinAward[];
  streak: number;
  shields: number;
  stageIndex: number;
  daysTogether: number;
  petName: string;
  lang: string;
  now: number;
}

/** "1.2K" / "2.4M" / "1.1B" for en; "1.2万" / "2.4亿" for zh. Floors below 1000 stay raw. */
export function formatTokens(n: number, lang: string): string {
  const v = Number.isFinite(n) && n > 0 ? n : 0;
  const zh = lang.startsWith('zh');
  const units: [number, string][] = zh
    ? [
        [100_000_000, '亿'],
        [10_000, '万'],
      ]
    : [
        [1_000_000_000, 'B'],
        [1_000_000, 'M'],
        [1_000, 'K'],
      ];
  for (const [base, suffix] of units) {
    if (v >= base) {
      const scaled = v / base;
      // One decimal under 10 units, whole numbers above ("2.4万", "38万").
      return `${scaled >= 10 ? Math.round(scaled) : Math.round(scaled * 10) / 10}${suffix}`;
    }
  }
  return String(v);
}

/** "7.2 – 7.8" from the window's first/last dates (language-neutral numerics). */
function weekLabelOf(dates: string[]): string {
  const fmt = (d: string) => {
    const [, m, day] = d.split('-');
    return m && day ? `${Number(m)}.${Number(day)}` : '';
  };
  const first = fmt(dates[0] ?? '');
  const last = fmt(dates[dates.length - 1] ?? '');
  return first && last ? `${first} – ${last}` : '';
}

/**
 * Merge the sources' trailing-7-day windows into one report. Sources are aligned
 * by date STRING (each source's array is generated independently — a fetch
 * straddling midnight must not shift another source's days), then the union's
 * last 7 dates win. Missing sources/days are zeros; the card renders regardless.
 */
export function assembleWeeklyReport(inputs: WeeklyReportInputs): WeeklyReport {
  const byDate = new Map<string, { tokens: number; messages: number; sessions: number }>();
  for (const stats of inputs.statsList) {
    for (const day of stats?.dailyStats ?? []) {
      if (typeof day?.date !== 'string' || day.date === '') continue;
      const agg = byDate.get(day.date) ?? { tokens: 0, messages: 0, sessions: 0 };
      agg.tokens += sane(day.input_tokens) + sane(day.output_tokens);
      agg.messages += sane(day.messages);
      agg.sessions += sane(day.sessions);
      byDate.set(day.date, agg);
    }
  }
  const dates = [...byDate.keys()].sort().slice(-WEEK_DAYS);
  const days = dates.map((d) => byDate.get(d) ?? { tokens: 0, messages: 0, sessions: 0 });
  while (days.length < WEEK_DAYS) {
    days.unshift({ tokens: 0, messages: 0, sessions: 0 });
  }

  const totalTokens = days.reduce((s, d) => s + d.tokens, 0);
  const since = inputs.now - WEEK_DAYS * DAY_MS;
  const agentTasks = inputs.recentAwards.filter(
    (a) => a.source === 'agent_stop' && a.at >= since,
  ).length;

  return {
    weekLabel: weekLabelOf(dates),
    totalTokens,
    totalTokensLabel: formatTokens(totalTokens, inputs.lang),
    dailyTokens: days.map((d) => d.tokens),
    agentTasks,
    tasksCapped: inputs.recentAwards.length >= LEDGER_RECENT_CAP,
    messages: days.reduce((s, d) => s + d.messages, 0),
    sessions: days.reduce((s, d) => s + d.sessions, 0),
    streak: Math.max(0, inputs.streak),
    shields: Math.max(0, inputs.shields),
    stageIndex: inputs.stageIndex,
    daysTogether: inputs.daysTogether,
    petName: inputs.petName,
  };
}

function sane(v: unknown): number {
  return typeof v === 'number' && Number.isFinite(v) && v > 0 ? v : 0;
}
