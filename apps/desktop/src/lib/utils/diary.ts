// 宠物日记 + 早安问候 (设计: docs/superpowers/specs/2026-07-09-pet-diary-design.md).
// Pure logic: day counters, day-summary settlement, capped diary append, greeting
// selection. Entries store STRUCTURED data, never rendered text — the UI renders
// through i18n at display time, so a language switch re-renders the whole book.
import type { DayPart } from './circadian';
import { daysApart } from './daily-board';

export const DIARY_CAP = 500;
export const SUMMARY_VARIANTS = 3;

// One special moment worth a diary line of its own. `ref` carries the moment's
// parameter (achievement/souvenir/species id, stage index as a string). Unknown
// kinds from a NEWER build are preserved by the sanitizer and skipped by the UI.
export type DiaryMomentKind =
  | 'adopted'
  | 'evolution'
  | 'achievement'
  | 'perfect_day'
  | 'souvenir'
  | 'egg_found'
  | 'egg_hatched'
  | 'dex_completed';

export interface DiaryMoment {
  kind: string;
  day: string; // local YYYY-MM-DD the moment belongs to
  at: number;
  ref?: string;
}

// The one-paragraph day summary, settled from that day's counters after the day ends.
export interface DiaryDaySummary {
  kind: 'day';
  day: string;
  at: number;
  agentTasks: number;
  meals: number;
  coinsEarned: number;
}

export type DiaryEntry = DiaryDaySummary | DiaryMoment;

// Live counters for the CURRENT day; settled into a DiaryDaySummary on rollover.
export interface DiaryDayCounters {
  date: string;
  agentTasks: number;
  meals: number;
  coinsEarned: number;
}

export function freshCounters(date: string): DiaryDayCounters {
  return { date, agentTasks: 0, meals: 0, coinsEarned: 0 };
}

/** Local calendar date of a timestamp, matching the store's todayStr() convention. */
export function localDayOf(at: number): string {
  const d = new Date(at);
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  return `${d.getFullYear()}-${mm}-${dd}`;
}

/**
 * Fold a finished day's counters into a summary entry. Null on an all-zero day —
 * quiet days leave a natural blank instead of a row of "nothing happened".
 */
export function settleDay(c: DiaryDayCounters, at: number): DiaryDaySummary | null {
  if (c.agentTasks <= 0 && c.meals <= 0 && c.coinsEarned <= 0) return null;
  return {
    kind: 'day',
    day: c.date,
    at,
    agentTasks: c.agentTasks,
    meals: c.meals,
    coinsEarned: c.coinsEarned,
  };
}

/**
 * Append an entry, trimming oldest-first past DIARY_CAP. 'adopted' is never trimmed —
 * the first brick of the memory moat outlives everything else.
 */
export function appendDiary(diary: readonly DiaryEntry[], entry: DiaryEntry): DiaryEntry[] {
  const next = [...diary, entry];
  while (next.length > DIARY_CAP) {
    const idx = next.findIndex((e) => e.kind !== 'adopted');
    if (idx === -1) break; // pathological all-adopted book: never drop the entry itself
    next.splice(idx, 1);
  }
  return next;
}

/** The newest day summary in the diary, or null. Entries are append-ordered. */
export function lastDaySummary(diary: readonly DiaryEntry[]): DiaryDaySummary | null {
  for (let i = diary.length - 1; i >= 0; i--) {
    const e = diary[i];
    if (e.kind === 'day') return e as DiaryDaySummary;
  }
  return null;
}

export interface Greeting {
  part: DayPart;
  /** Yesterday's completed agent tasks — 0 suppresses the summary line (an older
   *  summary is NOT "yesterday" and must not be presented as one). */
  tasks: number;
}

export function greetingFor(part: DayPart, last: DiaryDaySummary | null, today: string): Greeting {
  const isYesterday = last !== null && daysApart(last.day, today) === 1;
  return { part, tasks: isYesterday && last.agentTasks > 0 ? last.agentTasks : 0 };
}

/**
 * Deterministic opener-variant index for a day summary: the same diary page reads
 * the same on every open, but different days vary their phrasing.
 */
export function summaryVariant(day: string): number {
  let h = 0;
  for (let i = 0; i < day.length; i++) h = (h * 31 + day.charCodeAt(i)) >>> 0;
  return h % SUMMARY_VARIANTS;
}

const DAY_RE = /^\d{4}-\d{2}-\d{2}$/;

function saneCount(v: unknown): number {
  return typeof v === 'number' && Number.isFinite(v) && v > 0 ? Math.floor(v) : 0;
}

/**
 * Restore a persisted diary. Structure-checks only (valid day string + timestamp);
 * unknown moment kinds are KEPT so an old build never destroys a newer build's
 * entries — the UI skips what it can't render. Day summaries get their counters
 * coerced sane.
 */
export function sanitizeDiary(raw: unknown): DiaryEntry[] {
  if (!Array.isArray(raw)) return [];
  const out: DiaryEntry[] = [];
  for (const item of raw) {
    if (typeof item !== 'object' || item === null) continue;
    const e = item as Record<string, unknown>;
    if (typeof e.kind !== 'string' || e.kind === '') continue;
    if (typeof e.day !== 'string' || !DAY_RE.test(e.day)) continue;
    if (typeof e.at !== 'number' || !Number.isFinite(e.at)) continue;
    if (e.kind === 'day') {
      out.push({
        kind: 'day',
        day: e.day,
        at: e.at,
        agentTasks: saneCount(e.agentTasks),
        meals: saneCount(e.meals),
        coinsEarned: saneCount(e.coinsEarned),
      });
    } else {
      const m: DiaryMoment = { kind: e.kind, day: e.day, at: e.at };
      if (typeof e.ref === 'string') m.ref = e.ref;
      out.push(m);
    }
  }
  return out.slice(-DIARY_CAP);
}

/** Restore the live day counters; corrupt input → null (the store starts fresh). */
export function sanitizeDiaryDay(raw: unknown): DiaryDayCounters | null {
  if (typeof raw !== 'object' || raw === null) return null;
  const c = raw as Record<string, unknown>;
  if (typeof c.date !== 'string' || !DAY_RE.test(c.date)) return null;
  return {
    date: c.date,
    agentTasks: saneCount(c.agentTasks),
    meals: saneCount(c.meals),
    coinsEarned: saneCount(c.coinsEarned),
  };
}
