import { describe, expect, it } from 'vitest';
import {
  buildStageSnapshot,
  STAGE_BG_COLORS,
  STAGE_BGS,
  sanitizeStageBg,
  snapshotsEqual,
} from './stage-bridge';

describe('sanitizeStageBg', () => {
  it('passes every preset through', () => {
    for (const bg of STAGE_BGS) expect(sanitizeStageBg(bg)).toBe(bg);
  });

  it('falls back to green on garbage', () => {
    expect(sanitizeStageBg('lime')).toBe('green');
    expect(sanitizeStageBg(undefined)).toBe('green');
    expect(sanitizeStageBg(null)).toBe('green');
    expect(sanitizeStageBg(42)).toBe('green');
    expect(sanitizeStageBg({ bg: 'blue' })).toBe('green');
  });

  it('every preset maps to a pure chroma color', () => {
    expect(STAGE_BG_COLORS.green).toBe('#00ff00');
    expect(STAGE_BG_COLORS.blue).toBe('#0000ff');
    expect(STAGE_BG_COLORS.magenta).toBe('#ff00ff');
  });
});

describe('buildStageSnapshot', () => {
  it('fills defaults for everything optional', () => {
    const snap = buildStageSnapshot({ petId: 'yoonie', spriteState: 'idle' });
    expect(snap).toEqual({
      petId: 'yoonie',
      spriteState: 'idle',
      overlaySprite: null,
      away: false,
      celebration: null,
      activity: { waiting: 0, compacting: 0, working: 0 },
      locale: 'en',
      bg: 'green',
    });
  });

  it('passes explicit fields through and sanitizes bg', () => {
    const snap = buildStageSnapshot({
      petId: 'cat',
      spriteState: 'working',
      overlaySprite: 'eat',
      away: true,
      celebration: { kind: 'perfect_day' },
      activity: { waiting: 1, compacting: 0, working: 2 },
      locale: 'zh-CN',
      bg: 'magenta',
    });
    expect(snap.overlaySprite).toBe('eat');
    expect(snap.away).toBe(true);
    expect(snap.celebration).toEqual({ kind: 'perfect_day' });
    expect(snap.activity).toEqual({ waiting: 1, compacting: 0, working: 2 });
    expect(snap.locale).toBe('zh-CN');
    expect(snap.bg).toBe('magenta');
    expect(buildStageSnapshot({ petId: 'cat', spriteState: 'idle', bg: 'nope' }).bg).toBe('green');
  });
});

describe('snapshotsEqual', () => {
  const base = () =>
    buildStageSnapshot({
      petId: 'yoonie',
      spriteState: 'idle',
      activity: { waiting: 0, compacting: 0, working: 1 },
      locale: 'en',
      bg: 'green',
    });

  it('same content compares equal', () => {
    expect(snapshotsEqual(base(), base())).toBe(true);
  });

  it('any field change breaks equality', () => {
    for (const patch of [
      { petId: 'cat' },
      { spriteState: 'working' },
      { overlaySprite: 'eat' },
      { away: true },
      { celebration: { kind: 'perfect_day' } as const },
      { activity: { waiting: 1, compacting: 0, working: 1 } },
      { locale: 'zh' },
      { bg: 'blue' as const },
    ]) {
      expect(snapshotsEqual(base(), { ...base(), ...patch })).toBe(false);
    }
  });
});
