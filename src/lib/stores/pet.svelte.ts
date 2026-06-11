import { listen } from '@tauri-apps/api/event';
import { load } from '@tauri-apps/plugin-store';
import type {
  ClaudeTaskCompleteEvent,
  CoinAward,
  CoinSource,
  PetAction,
  PetData,
  PomodoroState,
  RewardLedgerSnapshot,
  UserInputEvent,
} from '../types';
import { tryInvoke } from '../utils/invoke';
import {
  type AwardResult,
  applyAward,
  applyUserInput,
  awardAgentStop,
  clearFocusStreak,
  DAILY_GIFT_COINS,
  FEED_COST_COINS,
  initialRewardState,
  type MutableRewardState,
  restoreRewardState,
  sanitizeStoredCount,
  snapshotRewardState,
} from '../utils/rewards';

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
export const AFFECTION_ACTIVITY_PER_10MIN = 1;
export const AFFECTION_FEED_HUNGRY = 5;
export const HUNGER_ACTIVITY_PER_HOUR = 3;
export const POMODORO_COINS_PER_MIN = 1;
// Tier-2 persistence cadence: the lifetime input counter mutates on every ~80ms input
// batch, so it is flushed lazily on this interval (awards themselves persist immediately).
const PET_PERSIST_FLUSH_MS = 60_000;
const PET_STATE_SCHEMA_VERSION = 1;

function todayStr(): string {
  return new Date().toISOString().slice(0, 10);
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
    pomodoroCoins: 0,
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
  private pomodoroInterval: ReturnType<typeof setInterval> | null = null;
  private storeInstance: Awaited<ReturnType<typeof load>> | null = null;
  private initPromise: Promise<() => void> | null = null;
  private inputCountDirty = false;
  private flushTimer: ReturnType<typeof setInterval> | null = null;

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

  applyFeed(amount: number = 20) {
    const wasHungry = this.petData.hunger < 30;
    const newHunger = Math.min(HUNGER_MAX, this.petData.hunger + amount);
    const affectionBonus = wasHungry ? AFFECTION_FEED_HUNGRY : 0;
    this.petData = {
      ...this.petData,
      hunger: newHunger,
      affection: Math.min(AFFECTION_MAX, this.petData.affection + affectionBonus),
      lastTickAt: Date.now(),
    };
    // Clamp-at-zero lives in the reward gate now — same Math.max(0, coins - 5) net effect.
    this.awardCoins('feed', -FEED_COST_COINS);
    this.currentAction = 'eat';
    setTimeout(() => {
      if (this.currentAction === 'eat') this.currentAction = 'idle';
    }, 3000);
  }

  applyHeadpat() {
    const today = todayStr();
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

  claimDailyGift() {
    const today = todayStr();
    if (this.petData.lastDailyGift === today) return false;
    // Both mutations complete synchronously before awardCoins' async persist runs, so
    // the saved snapshot always carries the claimed date AND the +50 together.
    this.petData = { ...this.petData, lastDailyGift: today };
    this.awardCoins('daily_gift', DAILY_GIFT_COINS, { reason: today });
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
    if (result.awards.length > 0) this.persistRewards();
    return result.awards;
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
    // Hunger/affection/headpat stay session-ephemeral (no pet-behavior change in P1-C);
    // only the coin slice persists. restoreRewardState() backfills anything missing.
    this.loadPetData({ ...defaultPetData(), coins, lastDailyGift });
    this.rewards = restoreRewardState({
      totals,
      recent,
      lifetimeInputCount,
      lastAwardedMilestone,
    });
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
    await store.save();
  }

  private startFlushTimer() {
    if (this.flushTimer) return;
    this.flushTimer = setInterval(() => {
      if (this.inputCountDirty) this.persistRewards();
      // Self-heal: re-assert input tracking (idempotent no-op while already active) in
      // case another owner's lifecycle disabled it mid-session.
      // TODO(settings phase): gate this on the input-capture setting once a user-facing
      // privacy toggle exists — a blind re-assert must never override an explicit OFF.
      tryInvoke('set_input_tracking', { active: true });
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
