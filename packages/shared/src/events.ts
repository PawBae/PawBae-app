import { assertBoundedInteger, assertEnum, assertExactRecord } from './validation';

export const AGENT_SOURCES = Object.freeze(['cc', 'codex', 'cursor'] as const);
export type AgentSource = (typeof AGENT_SOURCES)[number];

export const EVENT_RARITIES = Object.freeze(['common', 'rare', 'legendary'] as const);
export type EventRarity = (typeof EVENT_RARITIES)[number];

export const EVENT_KINDS = Object.freeze([
  'task_completed',
  'egg_hatched',
  'souvenir_found',
  'streak_milestone',
] as const);
export type EventKind = (typeof EVENT_KINDS)[number];

export interface TaskCompletedEvent {
  readonly kind: 'task_completed';
  readonly params: Readonly<{ source: AgentSource }>;
}

export interface EggHatchedEvent {
  readonly kind: 'egg_hatched';
  readonly params: Readonly<{ rarity: EventRarity }>;
}

export interface SouvenirFoundEvent {
  readonly kind: 'souvenir_found';
  readonly params: Readonly<{ rarity: EventRarity }>;
}

export interface StreakMilestoneEvent {
  readonly kind: 'streak_milestone';
  readonly params: Readonly<{ days: number }>;
}

export type PawBaeEvent =
  | TaskCompletedEvent
  | EggHatchedEvent
  | SouvenirFoundEvent
  | StreakMilestoneEvent;

export function createTaskCompletedEvent(input: unknown): TaskCompletedEvent {
  const raw = assertExactRecord(input, ['source'], 'task_completed params');
  const params = Object.freeze({ source: assertEnum(raw.source, AGENT_SOURCES, 'source') });
  return Object.freeze({ kind: 'task_completed', params });
}

export function createEggHatchedEvent(input: unknown): EggHatchedEvent {
  const raw = assertExactRecord(input, ['rarity'], 'egg_hatched params');
  const params = Object.freeze({ rarity: assertEnum(raw.rarity, EVENT_RARITIES, 'rarity') });
  return Object.freeze({ kind: 'egg_hatched', params });
}

export function createSouvenirFoundEvent(input: unknown): SouvenirFoundEvent {
  const raw = assertExactRecord(input, ['rarity'], 'souvenir_found params');
  const params = Object.freeze({ rarity: assertEnum(raw.rarity, EVENT_RARITIES, 'rarity') });
  return Object.freeze({ kind: 'souvenir_found', params });
}

export function createStreakMilestoneEvent(input: unknown): StreakMilestoneEvent {
  const raw = assertExactRecord(input, ['days'], 'streak_milestone params');
  const params = Object.freeze({ days: assertBoundedInteger(raw.days, 1, 3650, 'days') });
  return Object.freeze({ kind: 'streak_milestone', params });
}

export function createEvent(kind: unknown, params: unknown): PawBaeEvent {
  if (typeof kind !== 'string') {
    throw new TypeError('event kind must be a string');
  }
  if (!EVENT_KINDS.includes(kind as EventKind)) {
    throw new TypeError('unknown event kind');
  }
  switch (kind as EventKind) {
    case 'task_completed':
      return createTaskCompletedEvent(params);
    case 'egg_hatched':
      return createEggHatchedEvent(params);
    case 'souvenir_found':
      return createSouvenirFoundEvent(params);
    case 'streak_milestone':
      return createStreakMilestoneEvent(params);
  }
}
