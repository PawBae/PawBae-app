import { describe, expect, it } from 'vitest';
import {
  PET_MOODS,
  PET_SPRITE_STATES,
  sanitizePrivatePetSnapshot,
  sanitizePublicPetProjection,
} from './snapshots';

describe('sanitizePrivatePetSnapshot', () => {
  const valid = {
    petId: 'yoonie',
    spriteState: 'work',
    mood: 'focused',
    hunger: 73,
    level: 4,
    streak: 18,
    away: false,
  };

  it('returns a frozen snapshot in canonical key order regardless of input order', () => {
    const snapshot = sanitizePrivatePetSnapshot({
      away: false,
      streak: 18,
      level: 4,
      hunger: 73,
      mood: 'focused',
      spriteState: 'work',
      petId: 'yoonie',
    });

    expect(snapshot).toEqual(valid);
    expect(Object.isFrozen(snapshot)).toBe(true);
    expect(JSON.stringify(snapshot)).toBe(
      '{"petId":"yoonie","spriteState":"work","mood":"focused","hunger":73,"level":4,"streak":18,"away":false}',
    );
  });

  it('rejects unknown fields rather than risking private-data upload', () => {
    expect(() => sanitizePrivatePetSnapshot({ ...valid, prompt: 'do not upload' })).toThrowError(
      /private pet snapshot.*unknown key.*prompt/i,
    );
  });

  it('enforces safe ids and all frozen enum dictionaries', () => {
    expect(PET_SPRITE_STATES).toEqual([
      'idle',
      'walk',
      'run',
      'sleep',
      'work',
      'waiting',
      'compacting',
      'happy',
      'eat',
    ]);
    expect(PET_MOODS).toEqual(['neutral', 'happy', 'sleepy', 'focused']);
    expect(Object.isFrozen(PET_SPRITE_STATES)).toBe(true);
    expect(Object.isFrozen(PET_MOODS)).toBe(true);
    for (const petId of ['', '../secret', 'Yoonie', '-leading', 'a'.repeat(65)]) {
      expect(() => sanitizePrivatePetSnapshot({ ...valid, petId })).toThrowError(/petId/i);
    }
    expect(() => sanitizePrivatePetSnapshot({ ...valid, spriteState: 'typing' })).toThrowError(
      /spriteState/i,
    );
    expect(() => sanitizePrivatePetSnapshot({ ...valid, mood: 'angry' })).toThrowError(/mood/i);
  });

  it('requires exact primitive types and bounded integers', () => {
    const invalid: ReadonlyArray<[string, unknown]> = [
      ['hunger', -1],
      ['hunger', 101],
      ['hunger', 1.5],
      ['level', 0],
      ['level', 101],
      ['streak', -1],
      ['streak', 3651],
      ['away', 0],
    ];
    for (const [key, value] of invalid) {
      expect(() => sanitizePrivatePetSnapshot({ ...valid, [key]: value })).toThrow(TypeError);
    }
    expect(() => sanitizePrivatePetSnapshot(null)).toThrow(TypeError);
    expect(() => sanitizePrivatePetSnapshot({ ...valid, mood: undefined })).toThrow(TypeError);
  });
});

describe('sanitizePublicPetProjection', () => {
  const valid = {
    v: 1,
    petId: 'pet_01',
    displayName: 'Yoonie 云云',
    skinId: 'yoonie',
    status: 'working',
    updatedAt: '2026-07-10T12:34:56.000Z',
  };

  it('returns only the canonical public projection as a frozen object', () => {
    const projection = sanitizePublicPetProjection(valid);
    expect(projection).toEqual(valid);
    expect(Object.isFrozen(projection)).toBe(true);
    expect(Object.keys(projection)).toEqual([
      'v',
      'petId',
      'displayName',
      'skinId',
      'status',
      'updatedAt',
    ]);
  });

  it('rejects unknown or private fields instead of forwarding them', () => {
    for (const key of ['source', 'hunger', 'snapshot', 'prompt']) {
      expect(() => sanitizePublicPetProjection({ ...valid, [key]: 'private' })).toThrowError(
        new RegExp(`public pet projection.*unknown key.*${key}`, 'i'),
      );
    }
  });

  it('requires schema version one, safe ids, approved skins, and projection statuses', () => {
    expect(() => sanitizePublicPetProjection({ ...valid, v: 2 })).toThrowError(/v/i);
    expect(() => sanitizePublicPetProjection({ ...valid, petId: '../pet' })).toThrowError(/petId/i);
    expect(() => sanitizePublicPetProjection({ ...valid, skinId: 'custom-secret' })).toThrowError(
      /skinId/i,
    );
    expect(() => sanitizePublicPetProjection({ ...valid, status: 'celebrating' })).toThrowError(
      /status/i,
    );
  });

  it('allows bounded display-name prose but rejects control characters and invalid timestamps', () => {
    expect(sanitizePublicPetProjection({ ...valid, displayName: '  Momo  ' }).displayName).toBe(
      'Momo',
    );
    for (const displayName of [
      42,
      '',
      '   ',
      'a'.repeat(65),
      'Momo\nsecret',
      'Momo\u202esecret',
    ]) {
      expect(() => sanitizePublicPetProjection({ ...valid, displayName })).toThrowError(
        /displayName/i,
      );
    }
    for (const updatedAt of [
      '',
      'today',
      1,
      '2026',
      '2026-07-10',
      '2026-07-10T12:34:56',
      '2026-13-40T00:00:00Z',
    ]) {
      expect(() => sanitizePublicPetProjection({ ...valid, updatedAt })).toThrowError(/updatedAt/i);
    }
    expect(
      sanitizePublicPetProjection({ ...valid, updatedAt: '2026-07-10T12:34:56Z' }).updatedAt,
    ).toBe('2026-07-10T12:34:56.000Z');
  });

  it('accepts Postgres microseconds and rejects normalized impossible calendar dates', () => {
    expect(
      sanitizePublicPetProjection({
        ...valid,
        updatedAt: '2026-07-10T12:34:56.123456Z',
      }).updatedAt,
    ).toBe('2026-07-10T12:34:56.123Z');

    expect(() =>
      sanitizePublicPetProjection({
        ...valid,
        updatedAt: '2026-02-30T12:34:56Z',
      }),
    ).toThrowError(/updatedAt/i);
  });
});
