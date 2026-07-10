import { describe, expect, it } from 'vitest';
import { type CodexPet, DEFAULT_PET_ID } from './codex-pet';
import {
  mergeSkins,
  migrateMiniPetId,
  petJsonUrlFromSheetUrl,
  REMOVED_BUILTIN_PET_IDS,
  tileFrameStyle,
} from './skins';

function fakePet(id: string, overrides: Partial<CodexPet> = {}): CodexPet {
  return {
    id,
    displayName: id,
    description: '',
    spritesheetUrl: `/assets/builtin/${id}/spritesheet.webp`,
    atlas: { cellW: 192, cellH: 208, cols: 8, rows: 9 },
    animations: { idle: { row: 0, frames: 6 } },
    stateMap: { idle: 'idle', working: 'running', compacting: 'running', waiting: 'waiting' },
    oneShot: new Set(['jumping']),
    imageRendering: 'auto',
    ...overrides,
  };
}

describe('petJsonUrlFromSheetUrl', () => {
  it('maps the macOS/Linux protocol form to the folder-root pet.json', () => {
    expect(petJsonUrlFromSheetUrl('codexpet://localhost/mimi/spritesheet.webp')).toBe(
      'codexpet://localhost/mimi/pet.json',
    );
  });

  it('maps the Windows http form', () => {
    expect(petJsonUrlFromSheetUrl('http://codexpet.localhost/mimi/spritesheet.png')).toBe(
      'http://codexpet.localhost/mimi/pet.json',
    );
  });

  it('stays at the folder root even when the sheet lives in a subfolder', () => {
    expect(petJsonUrlFromSheetUrl('codexpet://localhost/mimi/sprites/sheet.png')).toBe(
      'codexpet://localhost/mimi/pet.json',
    );
  });

  it('rejects URLs without an id or file segment', () => {
    expect(petJsonUrlFromSheetUrl('codexpet://localhost/mimi')).toBeNull();
    expect(petJsonUrlFromSheetUrl('codexpet://localhost//sheet.png')).toBeNull();
    expect(petJsonUrlFromSheetUrl('')).toBeNull();
  });
});

describe('mergeSkins', () => {
  it('lets a custom skin override a builtin with the same id', () => {
    const builtin = fakePet('yoonie');
    const custom = fakePet('yoonie', { displayName: 'Custom Yoonie' });
    const merged = mergeSkins([builtin, fakePet('wukong')], [custom]);
    expect(merged).toHaveLength(2);
    expect(merged.find((p) => p.id === 'yoonie')?.displayName).toBe('Custom Yoonie');
  });

  it('appends non-colliding customs after builtins', () => {
    const merged = mergeSkins([fakePet('a')], [fakePet('b')]);
    expect(merged.map((p) => p.id)).toEqual(['a', 'b']);
  });
});

describe('migrateMiniPetId', () => {
  it('maps every removed builtin id to the default pet', () => {
    for (const id of REMOVED_BUILTIN_PET_IDS) {
      expect(migrateMiniPetId(id)).toBe(DEFAULT_PET_ID);
    }
  });

  it('passes surviving builtins and custom skin ids through', () => {
    expect(migrateMiniPetId('shimeji-bola')).toBe('shimeji-bola');
    expect(migrateMiniPetId('wukong')).toBe('wukong');
    expect(migrateMiniPetId('my-custom.codex-pet')).toBe('my-custom.codex-pet');
  });

  it('falls back to the default pet on empty input', () => {
    expect(migrateMiniPetId(null)).toBe(DEFAULT_PET_ID);
    expect(migrateMiniPetId(undefined)).toBe(DEFAULT_PET_ID);
    expect(migrateMiniPetId('')).toBe(DEFAULT_PET_ID);
  });
});

describe('tileFrameStyle', () => {
  it('crops the first idle frame at tile scale', () => {
    const style = tileFrameStyle(fakePet('a', { animations: { idle: { row: 3, frames: 6 } } }), 96);
    expect(style).toContain('width:96px');
    // cellH/cellW = 208/192 → 96 * 1.083… ≈ 104
    expect(style).toContain('height:104px');
    expect(style).toContain('background-size:768px 936px');
    expect(style).toContain('background-position:0px -312px');
  });

  it('respects offsetCol and falls back to the first animation without idle', () => {
    const style = tileFrameStyle(
      fakePet('a', { animations: { waving: { row: 1, frames: 4, offsetCol: 2 } } }),
      96,
    );
    expect(style).toContain('background-position:-192px -104px');
  });
});
