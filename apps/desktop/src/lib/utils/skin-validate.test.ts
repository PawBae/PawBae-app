import { describe, expect, it } from 'vitest';
import { isSafeSkinId, validateSkin } from './skin-validate';

const STANDARD_IMG = { width: 1536, height: 1872 }; // default atlas: 192×8, 208×9

function keys(issues: { key: string }[]): string[] {
  return issues.map((i) => i.key);
}

describe('isSafeSkinId', () => {
  it('accepts plain and unicode ids', () => {
    expect(isSafeSkinId('mimi-2')).toBe(true);
    expect(isSafeSkinId('doro.codex-pet')).toBe(true);
    expect(isSafeSkinId('云朵小猫')).toBe(true);
  });

  it('rejects traversal and separator shapes', () => {
    expect(isSafeSkinId('')).toBe(false);
    expect(isSafeSkinId('..')).toBe(false);
    expect(isSafeSkinId('../evil')).toBe(false);
    expect(isSafeSkinId('a/b')).toBe(false);
    expect(isSafeSkinId('a\\b')).toBe(false);
    expect(isSafeSkinId('C:evil')).toBe(false);
    expect(isSafeSkinId('.hidden')).toBe(false);
    expect(isSafeSkinId('a'.repeat(65))).toBe(false);
  });
});

describe('validateSkin', () => {
  it('rejects a non-object manifest outright', () => {
    expect(keys(validateSkin(null, STANDARD_IMG).errors)).toEqual(['notObject']);
    expect(keys(validateSkin([], STANDARD_IMG).errors)).toEqual(['notObject']);
  });

  it('passes a minimal standard-atlas skin cleanly', () => {
    const v = validateSkin({ id: 'mimi', displayName: 'Mimi' }, STANDARD_IMG);
    expect(v.errors).toEqual([]);
    expect(v.warnings).toEqual([]);
  });

  it('passes the generated single-image shape with only the simple-skin warning', () => {
    const v = validateSkin(
      {
        id: 'photo',
        atlas: { cellW: 800, cellH: 600, cols: 1, rows: 1 },
        animations: { idle: { row: 0, frames: 1 } },
      },
      { width: 800, height: 600 },
    );
    expect(v.errors).toEqual([]);
    expect(keys(v.warnings)).toEqual(['missingStandardRows']);
  });

  it('flags path-traversal ids and sheet paths as errors', () => {
    const v = validateSkin({ id: '../evil', spritesheetPath: '../../secret.png' }, STANDARD_IMG);
    expect(keys(v.errors)).toContain('invalidId');
    expect(keys(v.errors)).toContain('unsafeSheetPath');
  });

  it('flags non-positive or fractional atlas fields', () => {
    const v = validateSkin({ atlas: { cellW: 0, cols: 1.5 } }, STANDARD_IMG);
    expect(keys(v.errors)).toEqual(expect.arrayContaining(['atlasField', 'atlasField']));
  });

  it('errors when the atlas exceeds the image, warns on a remainder', () => {
    const wide = validateSkin(
      {
        atlas: { cellW: 100, cellH: 100, cols: 5, rows: 1 },
        animations: { idle: { row: 0, frames: 1 } },
      },
      { width: 499, height: 100 },
    );
    expect(keys(wide.errors)).toContain('atlasWiderThanImage');

    const remainder = validateSkin(
      {
        atlas: { cellW: 100, cellH: 100, cols: 4, rows: 1 },
        animations: { idle: { row: 0, frames: 1 } },
      },
      { width: 499, height: 100 },
    );
    expect(keys(remainder.errors)).toEqual([]);
    expect(keys(remainder.warnings)).toContain('atlasRemainder');
  });

  it('errors when the inherited standard rows cannot fit the declared atlas', () => {
    const v = validateSkin(
      { atlas: { cellW: 100, cellH: 100, cols: 1, rows: 1 } },
      { width: 100, height: 100 },
    );
    expect(keys(v.errors)).toContain('inheritedRowsExceedAtlas');
  });

  it('requires idle when animations are declared', () => {
    const v = validateSkin(
      {
        atlas: { cellW: 10, cellH: 10, cols: 2, rows: 2 },
        animations: { waving: { row: 0, frames: 2 } },
      },
      { width: 20, height: 20 },
    );
    expect(keys(v.errors)).toContain('missingIdle');
  });

  it('bounds animation rows and frames against the atlas', () => {
    const v = validateSkin(
      {
        atlas: { cellW: 10, cellH: 10, cols: 4, rows: 2 },
        animations: {
          idle: { row: 5, frames: 2 },
          waving: { row: 0, frames: 0 },
          dance: { row: 1, frames: 3, offsetCol: 2 },
        },
      },
      { width: 40, height: 20 },
    );
    expect(keys(v.errors)).toEqual(
      expect.arrayContaining(['rowOutOfRange', 'badFrames', 'framesOverflow']),
    );
  });

  it('rejects invalid offsetCol shapes instead of skipping the overflow check', () => {
    for (const bad of [-1, 1.5, '1', Number.NaN]) {
      const v = validateSkin(
        {
          atlas: { cellW: 10, cellH: 10, cols: 4, rows: 2 },
          animations: { idle: { row: 0, frames: 2, offsetCol: bad } },
        },
        { width: 40, height: 20 },
      );
      expect(keys(v.errors)).toContain('badOffsetCol');
    }
  });

  it('warns on typo-looking names, high fps, and dangling stateMap targets', () => {
    const v = validateSkin(
      {
        atlas: { cellW: 10, cellH: 10, cols: 8, rows: 9 },
        animations: { idle: { row: 0, frames: 2 }, idel: { row: 1, frames: 2, fps: 90 } },
        stateMap: { working: 'sprint' },
      },
      { width: 80, height: 90 },
    );
    expect(keys(v.warnings)).toEqual(
      expect.arrayContaining([
        'unknownAnimation',
        'highFps',
        'unknownStateMapTarget',
        'missingStandardRows',
      ]),
    );
    expect(v.errors).toEqual([]);
  });

  it('warns on huge images', () => {
    const v = validateSkin(
      {
        atlas: { cellW: 5000, cellH: 100, cols: 1, rows: 1 },
        animations: { idle: { row: 0, frames: 1 } },
      },
      { width: 5000, height: 100 },
    );
    expect(keys(v.warnings)).toContain('hugeImage');
  });
});
