import type { PetData, PetAction, PomodoroState } from '../types';

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

let petData = $state<PetData>(defaultPetData());
let currentAction = $state<PetAction>('idle');
let pomodoro = $state<PomodoroState | null>(null);
let pomodoroInterval: ReturnType<typeof setInterval> | null = null;

function applyDecay() {
  const now = Date.now();
  const hours = (now - petData.lastTickAt) / 3_600_000;
  if (hours < 0.01) return;

  const hungerDecay = HUNGER_DECAY_PER_HOUR * hours;
  const newHunger = Math.max(HUNGER_OFFLINE_FLOOR, petData.hunger - hungerDecay);

  let affectionDecay = (AFFECTION_DECAY_PER_DAY / 24) * hours;
  if (newHunger < 30) {
    affectionDecay += AFFECTION_HUNGRY_DECAY_PER_HOUR * hours;
  }
  const newAffection = Math.max(AFFECTION_OFFLINE_FLOOR, petData.affection - affectionDecay);

  petData = {
    ...petData,
    hunger: Math.round(newHunger * 10) / 10,
    affection: Math.round(newAffection * 10) / 10,
    lastTickAt: now,
  };
}

function applyFeed(amount: number = 20) {
  const wasHungry = petData.hunger < 30;
  const newHunger = Math.min(HUNGER_MAX, petData.hunger + amount);
  const affectionBonus = wasHungry ? AFFECTION_FEED_HUNGRY : 0;
  petData = {
    ...petData,
    hunger: newHunger,
    affection: Math.min(AFFECTION_MAX, petData.affection + affectionBonus),
    coins: Math.max(0, petData.coins - 5),
    lastTickAt: Date.now(),
  };
  currentAction = 'eat';
  setTimeout(() => {
    if (currentAction === 'eat') currentAction = 'idle';
  }, 3000);
}

function applyHeadpat() {
  const today = todayStr();
  let count = petData.headpatDate === today ? petData.headpatToday : 0;
  if (count >= AFFECTION_HEADPAT_DAILY_LIMIT) return;
  count++;
  petData = {
    ...petData,
    affection: Math.min(AFFECTION_MAX, petData.affection + AFFECTION_HEADPAT),
    headpatToday: count,
    headpatDate: today,
  };
  currentAction = 'headpat';
  setTimeout(() => {
    if (currentAction === 'headpat') currentAction = 'idle';
  }, 2000);
}

function claimDailyGift() {
  const today = todayStr();
  if (petData.lastDailyGift === today) return false;
  petData = {
    ...petData,
    coins: petData.coins + 50,
    lastDailyGift: today,
  };
  return true;
}

function startPomodoro(durationMin: number = 25) {
  const duration = durationMin * 60;
  pomodoro = {
    active: true,
    duration,
    remaining: duration,
    startedAt: Date.now(),
  };
  petData = { ...petData, pomodoroCoins: 0 };
  currentAction = 'work';

  pomodoroInterval = setInterval(() => {
    if (!pomodoro || !pomodoro.active) return;
    const elapsed = Math.floor((Date.now() - pomodoro.startedAt) / 1000);
    const remaining = Math.max(0, pomodoro.duration - elapsed);
    const earnedCoins = Math.floor(elapsed / 60) * POMODORO_COINS_PER_MIN;

    pomodoro = { ...pomodoro, remaining };
    petData = { ...petData, pomodoroCoins: earnedCoins };

    if (remaining <= 0) {
      stopPomodoro();
    }
  }, 1000);
}

function stopPomodoro() {
  if (pomodoroInterval) {
    clearInterval(pomodoroInterval);
    pomodoroInterval = null;
  }
  if (pomodoro) {
    petData = {
      ...petData,
      coins: petData.coins + petData.pomodoroCoins,
      pomodoroCoins: 0,
    };
  }
  pomodoro = null;
  currentAction = 'idle';
}

function setAction(action: PetAction) {
  currentAction = action;
}

function loadPetData(data: PetData) {
  petData = data;
  applyDecay();
}

export const petStore = {
  get petData() { return petData; },
  get currentAction() { return currentAction; },
  get pomodoro() { return pomodoro; },
  applyFeed,
  applyHeadpat,
  applyDecay,
  claimDailyGift,
  startPomodoro,
  stopPomodoro,
  setAction,
  loadPetData,
  defaultPetData,
};
