// Daily task board + forgiving streak (设计: docs/superpowers/specs/
// 2026-07-08-daily-task-board-design.md). Pure reducer in the approval-note mold:
// the store owns persistence and side effects; every rule here — date rollover,
// dedupe, check-in, shield spend/mint, clock regression — is a plain function of
// (state, taskId, today) and unit-tested as such.
//
// Philosophy guardrails (照护倒置, never punish):
// - Checking in takes ONE task; the full board is a bonus, never a requirement.
// - A gap the shields can't cover restarts the streak QUIETLY at 1 — and keeps
//   the shields. They failed to save this streak; they stay for the next one.
//   Losing the streak AND the shields would be a double punishment.

/** Fixed v1 task set. IDs are persisted in pet.json — never rename. */
export type BoardTaskId = 'gift' | 'headpat' | 'meal' | 'agent';

/** Display order in the panel. Labels live in i18n under `board.task.<id>`. */
export const BOARD_TASKS: readonly { id: BoardTaskId; emoji: string }[] = [
  { id: 'gift', emoji: '🎁' },
  { id: 'headpat', emoji: '🤚' },
  { id: 'meal', emoji: '🍖' },
  { id: 'agent', emoji: '🤖' },
] as const;

const TASK_IDS = new Set<string>(BOARD_TASKS.map((t) => t.id));

export function isBoardTaskId(v: unknown): v is BoardTaskId {
  return typeof v === 'string' && TASK_IDS.has(v);
}

/** Every 7 consecutive check-in days mints one shield… */
export const SHIELD_EARN_INTERVAL = 7;
/** …up to this many banked. */
export const SHIELD_CAP = 2;
/** Coins for completing the whole board (ledger source `task_board`). */
export const PERFECT_DAY_COINS = 15;

export interface BoardState {
  /** Day `boardDone` refers to; '' before the first mark ever. */
  boardDate: string;
  boardDone: BoardTaskId[];
  /** Consecutive check-in days as of `streakDate` (0 = no streak yet). */
  streak: number;
  /** Last check-in day; '' = never checked in. */
  streakDate: string;
  shields: number;
}

export interface BoardStepResult {
  state: BoardState;
  /** This call newly ticked the task (false = duplicate mark, full no-op). */
  taskCompleted: boolean;
  /** This call was today's first task: the streak advanced (or restarted). */
  checkedIn: boolean;
  /** Shields consumed to bridge the gap since the last check-in. */
  shieldsSpent: number;
  /** This check-in minted a new shield (streak hit a multiple of 7 under cap). */
  shieldEarned: boolean;
  /** This call completed the LAST remaining task of the day. */
  perfectDay: boolean;
}

export function initialBoardState(): BoardState {
  return { boardDate: '', boardDone: [], streak: 0, streakDate: '', shields: 0 };
}

/** Defensive read of persisted numbers: corrupt/hand-edited values become sane. */
function sanitizeCount(v: unknown, cap = Number.MAX_SAFE_INTEGER): number {
  return typeof v === 'number' && Number.isFinite(v) && v > 0 ? Math.min(cap, Math.floor(v)) : 0;
}

/** Sanitize a persisted done-list: known task ids only, deduped. */
export function sanitizeBoardDone(raw: unknown): BoardTaskId[] {
  if (!Array.isArray(raw)) return [];
  return [...new Set(raw.filter(isBoardTaskId))];
}

/**
 * Whole days from `from` to `to` for YYYY-MM-DD strings — abstract date math
 * (both parse as UTC midnights), so it is timezone-independent even though the
 * strings themselves now come from the local calendar. NaN on corrupt input.
 */
export function daysApart(from: string, to: string): number {
  return (Date.parse(to) - Date.parse(from)) / 86_400_000;
}

const noop = (state: BoardState): BoardStepResult => ({
  state,
  taskCompleted: false,
  checkedIn: false,
  shieldsSpent: 0,
  shieldEarned: false,
  perfectDay: false,
});

/**
 * Tick `task` for `today`. Rolls the board on a new day, dedupes repeat marks,
 * advances the streak on the day's first task (spending shields to bridge missed
 * days), mints earned shields, and flags a completed board. Pure — returns a new
 * state; the caller persists and pays rewards.
 */
export function markTask(s: BoardState, task: BoardTaskId, today: string): BoardStepResult {
  if (Number.isNaN(Date.parse(today))) return noop(s);

  const done = s.boardDate === today ? s.boardDone : [];
  if (done.includes(task)) return noop(s);
  const boardDone = [...done, task];

  let streak = sanitizeCount(s.streak);
  let streakDate = s.streakDate;
  let shields = sanitizeCount(s.shields, SHIELD_CAP);
  let checkedIn = false;
  let shieldsSpent = 0;
  let shieldEarned = false;

  if (streakDate !== today) {
    const gap = streakDate === '' ? Number.NaN : daysApart(streakDate, today) - 1;
    if (streakDate === '' || Number.isNaN(gap)) {
      // First check-in ever (or corrupt stored date): start fresh.
      streak = 1;
      checkedIn = true;
    } else if (gap < 0) {
      // Clock regression: freeze rather than corrupt — no advance, no reset.
      // The stored streakDate stays put; a later real "today" resolves it.
    } else if (gap === 0) {
      streak += 1;
      checkedIn = true;
    } else if (gap <= shields) {
      // The forgiving part: banked shields bridge the missed days seamlessly.
      shieldsSpent = gap;
      shields -= gap;
      streak += 1;
      checkedIn = true;
    } else {
      // Quiet restart; shields intentionally survive (see header note).
      streak = 1;
      checkedIn = true;
    }
    if (checkedIn) {
      streakDate = today;
      if (streak % SHIELD_EARN_INTERVAL === 0 && shields < SHIELD_CAP) {
        shields += 1;
        shieldEarned = true;
      }
    }
  }

  return {
    state: { boardDate: today, boardDone, streak, streakDate, shields },
    taskCompleted: true,
    checkedIn,
    shieldsSpent,
    shieldEarned,
    perfectDay: boardDone.length === BOARD_TASKS.length,
  };
}

/**
 * Streak for display: the stored value while it is alive OR still salvageable by
 * banked shields (the promise the UI makes must match what markTask will honor),
 * else 0. Clock regression shows the stored value — never a scary flash to zero.
 */
export function displayStreak(s: BoardState, today: string): number {
  const streak = sanitizeCount(s.streak);
  if (s.streakDate === '' || streak === 0) return 0;
  if (s.streakDate === today) return streak;
  const gap = daysApart(s.streakDate, today) - 1;
  if (Number.isNaN(gap)) return 0;
  if (gap < 0) return streak;
  return gap <= sanitizeCount(s.shields, SHIELD_CAP) ? streak : 0;
}

/** Anonymous telemetry bucket for `board_checkin` — never the raw count. */
export function streakBucket(streak: number): string {
  if (streak <= 2) return '1-2';
  if (streak <= 6) return '3-6';
  if (streak <= 29) return '7-29';
  return '30+';
}
