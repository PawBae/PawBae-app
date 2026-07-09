import { listen } from '@tauri-apps/api/event';
import { load } from '@tauri-apps/plugin-store';
import type {
  ClaudeStats,
  ClaudeStatsSource,
  ClaudeTaskCompleteEvent,
  CoinAward,
  CoinSource,
  GrowthCelebration,
  PetAction,
  PetData,
  PomodoroState,
  RewardLedgerSnapshot,
  UserInputEvent,
} from '../types';
import {
  type AchievementContext,
  evaluateAchievements,
  sanitizeUnlockMap,
} from '../utils/achievements';
import {
  ADVENTURE_MIN_MS,
  type AdventureState,
  consumeTrip,
  initialAdventureState,
  stepAdventure,
} from '../utils/adventure';
import { approvalAwardFor } from '../utils/approval-note';
import {
  type BoardState,
  type BoardTaskId,
  displayStreak,
  markTask,
  PERFECT_DAY_COINS,
  SHIELD_CAP,
  sanitizeBoardDone,
  streakBucket,
} from '../utils/daily-board';
import {
  addWarmth,
  EGG_COST_COINS,
  type EggState,
  eggReady,
  hatchablePool,
  rollNeighbor,
  sanitizeEgg,
  sanitizeMetNeighbors,
  shouldDropEgg,
  unmetNeighbors,
} from '../utils/eggs';
import { type EvolutionInfo, evolutionInfo } from '../utils/evolution';
import { tryInvoke } from '../utils/invoke';
import {
  type AwardResult,
  applyAward,
  applyUserInput,
  awardAgentStop,
  clearFocusStreak,
  currentGiftStreak,
  dailyGiftAmount,
  FEED_COST_COINS,
  initialRewardState,
  type MutableRewardState,
  restoreRewardState,
  sanitizeStoredCount,
  snapshotRewardState,
} from '../utils/rewards';
import {
  addSouvenir,
  LONG_TRIP_MS,
  rollSouvenir,
  type SouvenirOwned,
  sanitizeSouvenirs,
} from '../utils/souvenirs';
import { track } from '../utils/telemetry';
import {
  initialTokenFeedState,
  nutritionOf,
  primeTokenBaseline,
  settleTokenMeal,
  TOKEN_FEED_SOURCES,
  type TokenMeal,
} from '../utils/token-feed';
import { settingsStore } from './settings.svelte';
import { skinsStore } from './skins.svelte';

export const HUNGER_MAX = 100;
export const HUNGER_INIT = 100;
export const HUNGER_DECAY_PER_HOUR = 2;
export const HUNGER_DECAY_SLEEP_PER_HOUR = 1;
export const HUNGER_OFFLINE_FLOOR = 10;
export const AFFECTION_MAX = 100;
export const AFFECTION_INIT = 100;
export const AFFECTION_DECAY_PER_DAY = 5;
export const AFFECTION_HUNGRY_DECAY_PER_HOUR = 2;
export const AFFECTION_OFFLINE_FLOOR = 10;
export const AFFECTION_HEADPAT = 2;
export const AFFECTION_HEADPAT_DAILY_LIMIT = 5;
// Voice praise/headpat/play intents bump affection, but only once per cooldown so a
// user can't farm it by talking continuously. Session-ephemeral like headpat.
export const AFFECTION_VOICE_COOLDOWN_MS = 10_000;
export const AFFECTION_ACTIVITY_PER_10MIN = 1;
export const AFFECTION_FEED_HUNGRY = 5;
export const HUNGER_ACTIVITY_PER_HOUR = 3;
export const POMODORO_COINS_PER_MIN = 1;
// Tier-2 persistence cadence: the lifetime input counter mutates on every ~80ms input
// batch, so it is flushed lazily on this interval (awards themselves persist immediately).
const PET_PERSIST_FLUSH_MS = 60_000;
const PET_STATE_SCHEMA_VERSION = 1;

function todayStr(): string {
  // LOCAL calendar date on purpose: the UTC version made "a new day" start at
  // 4-5pm for US-Pacific users, visibly wrong once the task board showed it.
  // Comparisons/arithmetic on these strings are abstract-date operations
  // (yesterdayOf, daysApart), so the switch is safe beyond a one-time reset shift.
  const d = new Date();
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  return `${d.getFullYear()}-${mm}-${dd}`;
}

function defaultPetData(): PetData {
  return {
    hunger: HUNGER_INIT,
    affection: AFFECTION_INIT,
    coins: 0,
    lastTickAt: Date.now(),
    lastDailyGift: '',
    headpatToday: 0,
    headpatDate: todayStr(),
    approvalToday: 0,
    approvalDate: todayStr(),
    pomodoroCoins: 0,
    giftStreak: 0,
    firstMeetAt: Date.now(),
    boardDate: '',
    boardDone: [],
    streak: 0,
    streakDate: '',
    shields: 0,
  };
}

class PetStore {
  petData = $state<PetData>(defaultPetData());
  currentAction = $state<PetAction>('idle');
  pomodoro = $state<PomodoroState | null>(null);
  // Reward ledger/totals + milestone/focus bookkeeping (P1-C). The pure reducer in
  // utils/rewards.ts mutates this slice in place; petData.coins stays the single source
  // of truth for the displayed balance and is only ever set via commitCoins().
  rewards = $state<MutableRewardState>(initialRewardState());
  // Phase 6 growth: unlock map (id -> epoch ms), the highest evolution stage already
  // celebrated, and the FIFO of pending celebration moments MascotView plays back.
  achievements = $state<Record<string, number>>({});
  evolutionStageSeen = $state(0);
  celebrations = $state<GrowthCelebration[]>([]);
  // Agent adventure (Phase 1 冒险): the souvenir shelf (id → count/firstAt, persisted)
  // and the display layer's "some session has been busy long enough" flag. The trip
  // machine itself is plain state — MascotView steps it off the 2s session poll.
  souvenirs = $state<Record<string, SouvenirOwned>>({});
  // 孵蛋与物种图鉴: builtin neighbors already met (dex unlocks, persisted) and the
  // single incubating egg. Ready-to-hatch is derived from warmth, never stored.
  metNeighbors = $state<string[]>([]);
  egg = $state<EggState | null>(null);
  adventureAway = $state(false);
  private adventure: AdventureState = initialAdventureState();
  private pomodoroInterval: ReturnType<typeof setInterval> | null = null;
  private storeInstance: Awaited<ReturnType<typeof load>> | null = null;
  private initPromise: Promise<() => void> | null = null;
  private inputCountDirty = false;
  private flushTimer: ReturnType<typeof setInterval> | null = null;
  // Token feeding loop: per-source nutrition watermarks (ephemeral, like hunger itself)
  // and a busy lock so a completion can't race a stats scan already in flight.
  private tokenFeed = initialTokenFeedState();
  private tokenFeedBusy = false;

  applyDecay() {
    const now = Date.now();
    const hours = (now - this.petData.lastTickAt) / 3_600_000;
    if (hours < 0.01) return;

    const hungerDecay = HUNGER_DECAY_PER_HOUR * hours;
    const newHunger = Math.max(HUNGER_OFFLINE_FLOOR, this.petData.hunger - hungerDecay);

    let affectionDecay = (AFFECTION_DECAY_PER_DAY / 24) * hours;
    if (newHunger < 30) {
      affectionDecay += AFFECTION_HUNGRY_DECAY_PER_HOUR * hours;
    }
    const newAffection = Math.max(AFFECTION_OFFLINE_FLOOR, this.petData.affection - affectionDecay);

    this.petData = {
      ...this.petData,
      hunger: Math.round(newHunger * 10) / 10,
      affection: Math.round(newAffection * 10) / 10,
      lastTickAt: now,
    };
  }

  /** Whether the feed button should be live: the cost is covered and hunger isn't full. */
  get canFeed(): boolean {
    return this.petData.coins >= FEED_COST_COINS && this.petData.hunger < HUNGER_MAX;
  }

  /**
   * Shared meal application: hunger restore (clamped), the hungry-affection bonus,
   * and the eat beat with its revert timer.
   */
  private consumeMeal(restore: number) {
    this.markBoardTask('meal'); // both meal paths (manual feed + agent meal) land here
    const wasHungry = this.petData.hunger < 30;
    const affectionBonus = wasHungry ? AFFECTION_FEED_HUNGRY : 0;
    this.petData = {
      ...this.petData,
      hunger: Math.min(HUNGER_MAX, this.petData.hunger + restore),
      affection: Math.min(AFFECTION_MAX, this.petData.affection + affectionBonus),
      lastTickAt: Date.now(),
    };
    this.currentAction = 'eat';
    setTimeout(() => {
      if (this.currentAction === 'eat') this.currentAction = 'idle';
    }, 3000);
  }

  applyFeed(amount: number = 20): boolean {
    // UI gate: feeding while broke would be free (the reducer clamps the spend at
    // zero) and feeding at full hunger would burn coins for nothing. The reducer's
    // clamp stays as a defensive backstop behind this.
    if (!this.canFeed) return false;
    this.consumeMeal(amount);
    this.awardCoins('feed', -FEED_COST_COINS);
    this.warmEgg();
    return true;
  }

  /**
   * Free food the agent brought home (token feeding loop): restores hunger by the meal
   * size with no coin movement — the coin economy already pays agent_stop separately,
   * and the ledger stays coins-only. Safe at full hunger (clamps — the pet just
   * nibbles).
   */
  applyTokenMeal(meal: TokenMeal) {
    track('meal_fed', { tier: meal.tier });
    this.consumeMeal(meal.restore);
  }

  applyHeadpat() {
    const today = todayStr();
    // Board tick before the affection cap: the cap limits affection farming, not
    // the "petted today" fact (dedupe makes repeats free anyway).
    this.markBoardTask('headpat');
    let count = this.petData.headpatDate === today ? this.petData.headpatToday : 0;
    if (count >= AFFECTION_HEADPAT_DAILY_LIMIT) return;
    count++;
    this.petData = {
      ...this.petData,
      affection: Math.min(AFFECTION_MAX, this.petData.affection + AFFECTION_HEADPAT),
      headpatToday: count,
      headpatDate: today,
    };
    this.currentAction = 'headpat';
    setTimeout(() => {
      if (this.currentAction === 'headpat') this.currentAction = 'idle';
    }, 2000);
  }

  /**
   * A waiting agent got its answer (approval note): affection for a fast response,
   * regardless of whether the user clicked the note or answered in the terminal
   * directly. Slow responses are a silent no-op — never punish. Window and daily-cap
   * math live in utils/approval-note.ts; the ephemeral counter mirrors headpat's.
   */
  applyApprovalResponse(waitedMs: number): boolean {
    const today = todayStr();
    const count = this.petData.approvalDate === today ? this.petData.approvalToday : 0;
    const award = approvalAwardFor(waitedMs, count);
    track('approval_response', { awarded: award });
    if (award === 0) return false;
    this.petData = {
      ...this.petData,
      affection: Math.min(AFFECTION_MAX, this.petData.affection + award),
      approvalToday: count + 1,
      approvalDate: today,
    };
    return true;
  }

  private lastVoiceAffectionAt = 0;

  /** Add affection from a positive voice intent, rate-limited so talking can't farm it.
   *  No-op for non-positive deltas or while still on cooldown. */
  applyVoiceAffection(delta: number) {
    if (delta <= 0) return;
    const now = Date.now();
    if (now - this.lastVoiceAffectionAt < AFFECTION_VOICE_COOLDOWN_MS) return;
    this.lastVoiceAffectionAt = now;
    this.petData = {
      ...this.petData,
      affection: Math.min(AFFECTION_MAX, this.petData.affection + delta),
    };
  }

  /** Whether today's gift is still unclaimed. */
  get canClaimDailyGift(): boolean {
    return this.petData.lastDailyGift !== todayStr();
  }

  /** The unified streak for display: stored value while alive or shield-savable, else 0. */
  get streakLive(): number {
    return displayStreak(this.boardState, todayStr());
  }

  /** Today's ticked board tasks — [] when the stored board belongs to a previous day. */
  get boardDoneToday(): BoardTaskId[] {
    return this.petData.boardDate === todayStr() ? this.petData.boardDone : [];
  }

  /** What the next claim pays: probe the pure reducer for the streak a claim would reach. */
  get nextGiftAmount(): number {
    return dailyGiftAmount(Math.max(1, markTask(this.boardState, 'gift', todayStr()).state.streak));
  }

  /** Whole days since the pet was adopted (firstMeetAt). */
  get daysTogether(): number {
    return Math.max(0, Math.floor((Date.now() - this.petData.firstMeetAt) / 86_400_000));
  }

  /** Evolution snapshot derived from lifetime earnings. Cheap pure compute on access. */
  get evolution(): EvolutionInfo {
    return evolutionInfo(this.rewards.totals);
  }

  /** The daily-board slice of petData, in the pure reducer's shape. */
  private get boardState(): BoardState {
    const { boardDate, boardDone, streak, streakDate, shields } = this.petData;
    return { boardDate, boardDone, streak, streakDate, shields };
  }

  /**
   * Tick a daily-board task (utils/daily-board.ts owns every rule). The day's first
   * task checks in and advances the unified streak; completing all four pays the
   * perfect-day bonus and queues a celebration. Duplicate marks are free no-ops,
   * so call sites don't need their own gating.
   */
  markBoardTask(task: BoardTaskId) {
    const result = markTask(this.boardState, task, todayStr());
    if (!result.taskCompleted) return;
    this.petData = { ...this.petData, ...result.state };
    if (result.checkedIn) {
      track('board_checkin', { streak_bucket: streakBucket(result.state.streak) });
    }
    if (result.perfectDay) {
      this.celebrations = [...this.celebrations, { kind: 'perfect_day' }];
      track('board_perfect_day');
      // awardCoins → commitCoins persists, carrying the board slice with the coins.
      this.awardCoins('task_board', PERFECT_DAY_COINS, { reason: result.state.boardDate });
    } else {
      this.persistRewards();
    }
  }

  claimDailyGift() {
    const today = todayStr();
    if (this.petData.lastDailyGift === today) return false;
    // Date first (the double-claim gate), then the board tick — the day's first task
    // advances the unified streak, so the payout below reads the streak this claim
    // just earned. petData.giftStreak is frozen (absorbed by the board streak).
    this.petData = { ...this.petData, lastDailyGift: today };
    this.markBoardTask('gift');
    this.awardCoins('daily_gift', dailyGiftAmount(Math.max(1, this.petData.streak)), {
      reason: today,
    });
    return true;
  }

  startPomodoro(durationMin: number = 25) {
    if (this.pomodoroInterval) {
      clearInterval(this.pomodoroInterval);
      this.pomodoroInterval = null;
    }
    const duration = durationMin * 60;
    this.pomodoro = {
      active: true,
      duration,
      remaining: duration,
      startedAt: Date.now(),
    };
    this.petData = { ...this.petData, pomodoroCoins: 0 };
    // Drop any in-flight focus streak NOW: a short/canceled pomodoro with no input
    // during it would otherwise carry the old streak across the 90s gap window and
    // double-count this span with pomodoro coins (PR #15 review).
    clearFocusStreak(this.rewards);
    this.currentAction = 'work';
    tryInvoke('set_pet_pomodoro_active', { active: true });

    this.pomodoroInterval = setInterval(() => {
      if (!this.pomodoro?.active) return;
      const elapsed = Math.floor((Date.now() - this.pomodoro.startedAt) / 1000);
      const remaining = Math.max(0, this.pomodoro.duration - elapsed);
      const earnedCoins = Math.floor(elapsed / 60) * POMODORO_COINS_PER_MIN;

      this.pomodoro = { ...this.pomodoro, remaining };
      this.petData = { ...this.petData, pomodoroCoins: earnedCoins };

      if (remaining <= 0) {
        this.stopPomodoro();
      }
    }, 1000);
  }

  stopPomodoro() {
    if (this.pomodoroInterval) {
      clearInterval(this.pomodoroInterval);
      this.pomodoroInterval = null;
    }
    if (this.pomodoro) {
      // Zero the staging FIRST so a UI stop racing the interval's auto-stop commits once.
      const staged = this.petData.pomodoroCoins;
      this.petData = { ...this.petData, pomodoroCoins: 0 };
      if (staged > 0) this.awardCoins('pomodoro', staged);
    }
    this.pomodoro = null;
    this.currentAction = 'idle';
    tryInvoke('set_pet_pomodoro_active', { active: false });
  }

  setAction(action: PetAction) {
    this.currentAction = action;
  }

  loadPetData(data: PetData) {
    this.petData = data;
    this.applyDecay();
  }

  // ── P1-C reward model ────────────────────────────────────────────

  /**
   * The single entry point for every coin change (earn or spend). Delegates to the pure
   * reducer (clamp-at-zero, ledger append, totals) and applies the result to petData via
   * the usual immutable spread. Returns the ledger entries actually recorded.
   */
  awardCoins(
    source: CoinSource,
    amount: number,
    meta: { reason?: string; sessionId?: string; at?: number } = {},
  ): CoinAward[] {
    const result = applyAward(this.rewards, this.petData.coins, {
      source,
      amount,
      at: meta.at ?? Date.now(),
      reason: meta.reason,
      sessionId: meta.sessionId,
    });
    return this.commitCoins(result);
  }

  private commitCoins(result: AwardResult): CoinAward[] {
    if (result.coinsAfter !== this.petData.coins) {
      this.petData = { ...this.petData, coins: result.coinsAfter };
    }
    if (result.awards.length > 0) {
      // Growth runs BEFORE the persist so one save carries the balance, any new unlock
      // timestamps and the celebrated-stage marker together.
      this.checkGrowth(result.awards[result.awards.length - 1].at);
      this.persistRewards();
    }
    return result.awards;
  }

  /**
   * Re-derive evolution stage + achievements from current state and queue celebrations
   * for anything newly reached. Idempotent: everything is a predicate over persisted
   * counters, so re-running can never double-celebrate (stage marker / unlock map gate).
   */
  private checkGrowth(at: number): boolean {
    let dirty = false;
    const info = this.evolution;
    if (info.stageIndex > this.evolutionStageSeen) {
      // Jumping several stages at once (e.g. a restored ledger meeting this feature for
      // the first time) celebrates only the stage actually reached, not each rung.
      this.evolutionStageSeen = info.stageIndex;
      this.celebrations = [
        ...this.celebrations,
        { kind: 'evolution', stageIndex: info.stageIndex },
      ];
      dirty = true;
    }
    const ctx: AchievementContext = {
      totals: this.rewards.totals,
      lifetimeInputCount: this.rewards.lifetimeInputCount,
      streak: this.streakLive,
      daysTogether: this.daysTogether,
      stageIndex: info.stageIndex,
    };
    const fresh = evaluateAchievements(ctx, this.achievements);
    if (fresh.length > 0) {
      const next: Record<string, number> = { ...this.achievements };
      for (const def of fresh) next[def.id] = at;
      this.achievements = next;
      this.celebrations = [
        ...this.celebrations,
        ...fresh.map((d): GrowthCelebration => ({ kind: 'achievement', id: d.id })),
      ];
      dirty = true;
    }
    return dirty;
  }

  /** Pop the celebration currently shown; MascotView calls this after its display beat. */
  shiftCelebration() {
    if (this.celebrations.length > 0) this.celebrations = this.celebrations.slice(1);
  }

  /**
   * Step the adventure trip machine against the session poll (MascotView owns the
   * cadence: on busy/alive set changes plus a slow tick, since a threshold crossing
   * changes no set). Only updates the display flag when it actually flips — this is
   * called from an effect, and an unconditional $state write would re-trigger it.
   */
  stepAdventure(busyIds: readonly string[], aliveIds: readonly string[], now: number) {
    const { away } = stepAdventure(this.adventure, busyIds, aliveIds, now);
    if (this.adventureAway !== away) this.adventureAway = away;
  }

  // ── 孵蛋与物种图鉴 ────────────────────────────────────────────────

  /** Builtin ids only — customs are never gated and never hatch (UGC 红线). */
  private get builtinSkinIds(): string[] {
    return skinsStore.all.filter((p) => !skinsStore.customIds.has(p.id)).map((p) => p.id);
  }

  /** Neighbors the next hatch can reveal. Empty until the skins store has loaded. */
  get unmetNeighborIds(): string[] {
    return unmetNeighbors(hatchablePool(this.builtinSkinIds), this.metNeighbors);
  }

  get eggReady(): boolean {
    return eggReady(this.egg);
  }

  get canBuyEgg(): boolean {
    return (
      this.egg === null && this.petData.coins >= EGG_COST_COINS && this.unmetNeighborIds.length > 0
    );
  }

  buyEgg(): boolean {
    if (!this.canBuyEgg) return false;
    // Egg first: awardCoins → commitCoins persists, and that save must carry the egg.
    this.egg = { warmth: 0, since: Date.now() };
    this.awardCoins('egg', -EGG_COST_COINS);
    track('egg_bought');
    return true;
  }

  /** One unit of 完工暖香 (a genuine agent completion or a meal) toward the egg. */
  private warmEgg() {
    if (this.egg === null || eggReady(this.egg)) return;
    this.egg = addWarmth(this.egg);
    this.persistRewards();
  }

  /**
   * Crack the ready egg: roll an unmet neighbor, mark it met, clear the egg. Returns
   * the revealed id (the caller switches the active skin). If a migration edge emptied
   * the pool after purchase, the egg is refunded instead — never punish.
   */
  revealEgg(): string | null {
    if (!eggReady(this.egg)) return null;
    const unmet = this.unmetNeighborIds;
    const id = rollNeighbor(unmet, Math.random);
    this.egg = null;
    if (id === null) {
      this.awardCoins('egg', EGG_COST_COINS, { reason: 'refund' });
      return null;
    }
    this.metNeighbors = [...this.metNeighbors, id];
    // Builtin ids are a fixed vocabulary (never user content), safe for telemetry.
    track('egg_hatched', { species: id });
    if (unmet.length === 1) track('dex_completed');
    this.persistRewards();
    return id;
  }

  /**
   * One-shot migration guard (Main calls this once settings + skins are loaded): an
   * install already using a builtin neighbor keeps it — never confiscate the current pet.
   */
  noteCurrentSkinMet(currentId: string) {
    if (!hatchablePool(this.builtinSkinIds).includes(currentId)) return;
    if (this.metNeighbors.includes(currentId)) return;
    this.metNeighbors = [...this.metNeighbors, currentId];
    this.persistRewards();
  }

  /** A genuine completion ends the session's trip; a long-enough one earns a souvenir. */
  private settleAdventure(sessionId: string, now: number) {
    const elapsed = consumeTrip(this.adventure, sessionId, now);
    if (elapsed === null || elapsed < ADVENTURE_MIN_MS) return;
    // A long trip may bring a free egg home instead of a souvenir (only while no egg
    // is incubating and someone is left to meet) — the bigger surprise wins the slot.
    if (
      shouldDropEgg(elapsed >= LONG_TRIP_MS, this.egg, this.unmetNeighborIds.length, Math.random)
    ) {
      this.egg = { warmth: 0, since: now };
      this.celebrations = [...this.celebrations, { kind: 'egg_found' }];
      this.persistRewards();
      return;
    }
    const found = rollSouvenir(elapsed, Math.random);
    this.souvenirs = addSouvenir(this.souvenirs, found.id, now);
    this.celebrations = [...this.celebrations, { kind: 'souvenir', id: found.id }];
    // Rarity only — the dictionary stays minimal (no item ids).
    track('souvenir_found', { rarity: found.rarity });
    this.persistRewards();
  }

  private handleTaskComplete(payload: ClaudeTaskCompleteEvent) {
    // Rust already filters subagent stops, ESC interrupts, and compaction; the reducer
    // drops permission-waits (waiting: true) and dedupes per session with a cooldown.
    this.commitCoins(
      awardAgentStop(this.rewards, this.petData.coins, {
        sessionId: payload.sessionId,
        waiting: payload.waiting,
        at: Date.now(), // the wire payload carries no timestamp
      }),
    );
    // Token feeding loop: a genuine completion (not a permission wait) may earn a meal.
    if (!payload.waiting) {
      track('agent_task_complete', { source: payload.source });
      this.markBoardTask('agent');
      // Warm the incubating egg BEFORE the trip settles: an egg dropped by this very
      // completion starts cold instead of instantly absorbing its own arrival.
      this.warmEgg();
      this.settleAdventure(payload.sessionId, Date.now());
      void this.settleTokenFeed(payload.source);
    }
  }

  /**
   * Fetch the source's cumulative token totals and settle them against the watermark;
   * a meal feeds the pet. Busy-locked (CLAUDE.md polling lesson): a completion landing
   * while a previous scan is in flight is skipped — its tokens stay in the delta and
   * are picked up by the next completion instead.
   */
  private async settleTokenFeed(source: ClaudeStatsSource) {
    if (this.tokenFeedBusy) return;
    this.tokenFeedBusy = true;
    try {
      const stats = await tryInvoke<ClaudeStats>('get_claude_stats', { source });
      if (!stats) return;
      const meal = settleTokenMeal(this.tokenFeed, source, nutritionOf(stats));
      if (meal) this.applyTokenMeal(meal);
    } finally {
      this.tokenFeedBusy = false;
    }
  }

  /**
   * Best-effort baseline priming so the FIRST completion of a run feeds a real delta
   * instead of only setting the watermark. Fire-and-forget per source; a failed fetch
   * merely downgrades that source's first completion to baseline-setting (never a
   * retro-feast), and primeTokenBaseline refuses to rewind a watermark a fast settle
   * already advanced.
   */
  private primeTokenBaselines() {
    for (const source of TOKEN_FEED_SOURCES) {
      void tryInvoke<ClaudeStats>('get_claude_stats', { source }).then((stats) => {
        if (stats) primeTokenBaseline(this.tokenFeed, source, nutritionOf(stats));
      });
    }
  }

  private handleUserInput(ev: UserInputEvent) {
    const result = applyUserInput(this.rewards, this.petData.coins, ev, {
      pomodoroActive: this.pomodoro?.active === true,
    });
    // Always mark dirty: the lifetime counter moved either way, and if an award's
    // immediate persist fails the 60s flush timer then retries it.
    this.inputCountDirty = true;
    if (result.awards.length > 0) this.commitCoins(result);
  }

  /**
   * Register the reward event listeners and hydrate persisted state. Idempotent (HMR /
   * re-mounted effects share one promise); returns a dispose function for the caller's
   * effect cleanup. Hydration completes BEFORE listeners register, so a restored balance
   * can never be overwritten by a racing award.
   */
  init(): Promise<() => void> {
    if (!this.initPromise) this.initPromise = this.doInit();
    return this.initPromise;
  }

  private async doInit(): Promise<() => void> {
    try {
      await this.hydrate();
    } catch (e) {
      console.warn('[pet] hydrate failed, starting with defaults:', e);
    }
    const unsubs: (() => void)[] = [];
    unsubs.push(
      await listen<ClaudeTaskCompleteEvent>('claude-task-complete', (e) =>
        this.handleTaskComplete(e.payload),
      ),
    );
    unsubs.push(await listen<UserInputEvent>('user-input', (e) => this.handleUserInput(e.payload)));
    // Global input capture is OFF by default in the backend (privacy) — opt in AFTER the
    // listener exists so the first flushed batch cannot fall into a gap. Idempotent in
    // Rust, so MascotView's own tracking lifecycle composes safely with this.
    tryInvoke('set_input_tracking', { active: true });
    this.primeTokenBaselines();
    this.startFlushTimer();
    return () => {
      for (const unsub of unsubs) unsub();
      this.stopFlushTimer();
      this.persistRewards(); // best-effort final flush
      this.initPromise = null;
    };
  }

  private async getStore() {
    if (!this.storeInstance) {
      this.storeInstance = await load('pet.json', { defaults: {}, autoSave: true });
    }
    return this.storeInstance;
  }

  private async hydrate() {
    const store = await this.getStore();
    // Future schema migrations key off schema_version (written on every save; v1 today).
    // Numeric reads are sanitized: a corrupt/hand-edited pet.json (strings, negatives,
    // Infinity) must never produce a NaN balance or an unbounded reducer loop.
    const coins = sanitizeStoredCount(await store.get('coins'));
    const rawGift = await store.get('last_daily_gift');
    const lastDailyGift = typeof rawGift === 'string' ? rawGift : '';
    // May be undefined on a fresh install — restoreRewardState() backfills zeros.
    const totals = (await store.get('reward_totals')) as RewardLedgerSnapshot['totals'];
    const recent = ((await store.get('reward_ledger')) as CoinAward[]) ?? [];
    const lifetimeInputCount = sanitizeStoredCount(await store.get('lifetime_input_count'));
    const lastAwardedMilestone = sanitizeStoredCount(await store.get('last_input_milestone'));
    const giftStreak = sanitizeStoredCount(await store.get('gift_streak'));
    // firstMeetAt is written exactly once: a missing/corrupt value means this install
    // predates the growth system (or is fresh) — adopt now and persist it on first save.
    const rawFirstMeet = await store.get('first_meet_at');
    const firstMeetAt =
      typeof rawFirstMeet === 'number' && Number.isFinite(rawFirstMeet) && rawFirstMeet > 0
        ? rawFirstMeet
        : Date.now();
    this.achievements = sanitizeUnlockMap(await store.get('achievements'));
    this.evolutionStageSeen = sanitizeStoredCount(await store.get('evolution_stage_seen'));
    this.souvenirs = sanitizeSouvenirs(await store.get('souvenirs'));
    this.metNeighbors = sanitizeMetNeighbors(await store.get('met_neighbors'));
    this.egg = sanitizeEgg(await store.get('egg'));
    // Daily task board. Migration: an install that predates the board (no streak_date
    // key ever written) seeds the unified streak from the legacy gift streak, so a
    // live 30-day streak survives the upgrade instead of silently restarting.
    const rawStreakDate = await store.get('streak_date');
    const rawBoardDate = await store.get('board_date');
    let streak = sanitizeStoredCount(await store.get('streak'));
    let streakDate = typeof rawStreakDate === 'string' ? rawStreakDate : '';
    if (rawStreakDate === undefined) {
      streak = currentGiftStreak(lastDailyGift, todayStr(), giftStreak);
      streakDate = streak > 0 ? lastDailyGift : '';
    }
    const boardDate = typeof rawBoardDate === 'string' ? rawBoardDate : '';
    const boardDone = sanitizeBoardDone(await store.get('board_done'));
    const shields = Math.min(SHIELD_CAP, sanitizeStoredCount(await store.get('shields')));
    // Hunger/affection/headpat stay session-ephemeral (no pet-behavior change in P1-C);
    // only the coin slice persists. restoreRewardState() backfills anything missing.
    this.loadPetData({
      ...defaultPetData(),
      coins,
      lastDailyGift,
      giftStreak,
      firstMeetAt,
      boardDate,
      boardDone,
      streak,
      streakDate,
      shields,
    });
    this.rewards = restoreRewardState({
      totals,
      recent,
      lifetimeInputCount,
      lastAwardedMilestone,
    });
    // Time/stage-driven growth (days-together, a ledger that out-leveled the celebrated
    // stage while this feature shipped) must be caught at startup, not only on the next
    // coin event. Persist immediately so a crash can't replay the celebration.
    if (this.checkGrowth(Date.now())) this.persistRewards();
  }

  private saveInFlight: Promise<void> = Promise.resolve();

  private persistRewards() {
    // Serialize saves: overlapping fire-and-forget writes could otherwise land out of
    // order and leave the file with the older snapshot.
    this.saveInFlight = this.saveInFlight
      .then(() => this.savePetState())
      .catch((e) => {
        // Re-mark dirty so the 60s timer retries the counter after a failed write.
        this.inputCountDirty = true;
        console.warn('[pet] persist failed:', e);
      });
  }

  private async savePetState() {
    // Clear the dirty flag BEFORE snapshotting: input landing while this save is in
    // flight re-marks it, so the 60s timer flushes the newer count instead of the flag
    // being wiped by this save's completion (PR #15 review).
    this.inputCountDirty = false;
    const store = await this.getStore();
    const snap = snapshotRewardState(this.rewards);
    await store.set('schema_version', PET_STATE_SCHEMA_VERSION);
    await store.set('coins', this.petData.coins);
    await store.set('last_daily_gift', this.petData.lastDailyGift);
    await store.set('reward_totals', snap.totals);
    await store.set('reward_ledger', snap.recent);
    await store.set('lifetime_input_count', snap.lifetimeInputCount);
    await store.set('last_input_milestone', snap.lastAwardedMilestone);
    await store.set('gift_streak', this.petData.giftStreak);
    await store.set('board_date', this.petData.boardDate);
    await store.set('board_done', [...this.petData.boardDone]);
    await store.set('streak', this.petData.streak);
    await store.set('streak_date', this.petData.streakDate);
    await store.set('shields', this.petData.shields);
    await store.set('first_meet_at', this.petData.firstMeetAt);
    await store.set('achievements', { ...this.achievements });
    await store.set('evolution_stage_seen', this.evolutionStageSeen);
    await store.set('souvenirs', { ...this.souvenirs });
    await store.set('met_neighbors', [...this.metNeighbors]);
    await store.set('egg', this.egg ? { ...this.egg } : null);
    await store.save();
  }

  private startFlushTimer() {
    if (this.flushTimer) return;
    this.flushTimer = setInterval(() => {
      if (this.inputCountDirty) this.persistRewards();
      // Self-heal: re-assert input tracking (idempotent no-op while already active) in
      // case another owner's lifecycle disabled it mid-session. Gated on the privacy
      // toggle — a re-assert must never override an explicit user OFF.
      if (settingsStore.inputTrackingEnabled) {
        tryInvoke('set_input_tracking', { active: true });
      }
    }, PET_PERSIST_FLUSH_MS);
  }

  private stopFlushTimer() {
    if (this.flushTimer) {
      clearInterval(this.flushTimer);
      this.flushTimer = null;
    }
  }

  defaultPetData = defaultPetData;
}

export const petStore = new PetStore();
