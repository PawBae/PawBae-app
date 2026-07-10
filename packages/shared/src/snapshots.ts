import {
  APPROVED_SKIN_IDS,
  PROJECTION_STATUSES,
  type PublicPetProjection,
} from './contracts';
import {
  assertBoolean,
  assertBoundedInteger,
  assertDisplayName,
  assertEnum,
  assertExactRecord,
  assertIsoTimestamp,
  assertSafeId,
} from './validation';

export const PET_SPRITE_STATES = Object.freeze([
  'idle',
  'walk',
  'run',
  'sleep',
  'work',
  'waiting',
  'compacting',
  'happy',
  'eat',
] as const);
export type PetSpriteState = (typeof PET_SPRITE_STATES)[number];

export const PET_MOODS = Object.freeze(['neutral', 'happy', 'sleepy', 'focused'] as const);
export type PetMood = (typeof PET_MOODS)[number];

export interface PrivatePetSnapshot {
  readonly petId: string;
  readonly spriteState: PetSpriteState;
  readonly mood: PetMood;
  readonly hunger: number;
  readonly level: number;
  readonly streak: number;
  readonly away: boolean;
}

const PRIVATE_SNAPSHOT_KEYS = Object.freeze([
  'petId',
  'spriteState',
  'mood',
  'hunger',
  'level',
  'streak',
  'away',
] as const);

const PUBLIC_PROJECTION_KEYS = Object.freeze([
  'v',
  'petId',
  'displayName',
  'skinId',
  'status',
  'updatedAt',
] as const);

export function sanitizePrivatePetSnapshot(input: unknown): PrivatePetSnapshot {
  const raw = assertExactRecord(input, PRIVATE_SNAPSHOT_KEYS, 'private pet snapshot');
  return Object.freeze({
    petId: assertSafeId(raw.petId, 'petId'),
    spriteState: assertEnum(raw.spriteState, PET_SPRITE_STATES, 'spriteState'),
    mood: assertEnum(raw.mood, PET_MOODS, 'mood'),
    hunger: assertBoundedInteger(raw.hunger, 0, 100, 'hunger'),
    level: assertBoundedInteger(raw.level, 1, 100, 'level'),
    streak: assertBoundedInteger(raw.streak, 0, 3650, 'streak'),
    away: assertBoolean(raw.away, 'away'),
  });
}

export function sanitizePublicPetProjection(input: unknown): PublicPetProjection {
  const raw = assertExactRecord(input, PUBLIC_PROJECTION_KEYS, 'public pet projection');
  if (raw.v !== 1) {
    throw new TypeError('public pet projection.v must equal 1');
  }
  return Object.freeze({
    v: 1,
    petId: assertSafeId(raw.petId, 'petId'),
    displayName: assertDisplayName(raw.displayName, 'displayName'),
    skinId: assertEnum(raw.skinId, APPROVED_SKIN_IDS, 'skinId'),
    status: assertEnum(raw.status, PROJECTION_STATUSES, 'status'),
    updatedAt: assertIsoTimestamp(raw.updatedAt, 'updatedAt'),
  });
}
