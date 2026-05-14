import type { PetAction, PetData, PomodoroState } from '../types';

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
  private pomodoroInterval: ReturnType<typeof setInterval> | null = null;

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
      coins: Math.max(0, this.petData.coins - 5),
      lastTickAt: Date.now(),
    };
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
    this.petData = {
      ...this.petData,
      coins: this.petData.coins + 50,
      lastDailyGift: today,
    };
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
      this.petData = {
        ...this.petData,
        coins: this.petData.coins + this.petData.pomodoroCoins,
        pomodoroCoins: 0,
      };
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

  defaultPetData = defaultPetData;
}

export const petStore = new PetStore();
