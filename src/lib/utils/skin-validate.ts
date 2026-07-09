// Import-time validation for user-supplied skins ("宽进严出"): errors block the
// import (and roll it back), warnings ride along as gallery badges. Every issue is
// an i18n key + params so the report is bilingual for free.
//
// Threat model: skins will be downloaded from strangers. The id doubles as the
// destination folder name under ~/.codex/pets, so path-traversal shapes are errors
// here AND rejected again in Rust (defense in depth).
// See docs/superpowers/specs/2026-07-09-skin-workshop-design.md §4-5.

import { DEFAULT_ATLAS, STANDARD_ANIMATION_ROWS } from './codex-pet';

export interface SkinIssue {
  /** i18n key suffix under `skin.issue.*` */
  key: string;
  params?: Record<string, string | number>;
}

export interface SkinValidation {
  errors: SkinIssue[];
  warnings: SkinIssue[];
}

export interface ImageDims {
  width: number;
  height: number;
}

/**
 * Every animation-row name some runtime consumer actually asks for (standard 9,
 * physics, meal beat, input reactions, idle micro-actions, voice emotions, yoonie
 * one-shots). Anything else is probably a typo — worth a warning, never an error.
 */
export const KNOWN_ANIMATION_NAMES: ReadonlySet<string> = new Set([
  ...Object.keys(STANDARD_ANIMATION_ROWS),
  'falling',
  'bouncing',
  'grab-wall',
  'grab-wall-flipped',
  'climb-wall',
  'climb-wall-flipped',
  'climb-ceiling',
  'climb-ceiling-flipped',
  'eat',
  'happy',
  'angry',
  'react-keyboard',
  'react-mouse',
  'blink',
  'thinking',
  'pounce',
  'yawn',
  'sleep',
  'rest',
  'dance',
  'spin',
  'peek',
  'done-success',
  'done-fail',
]);

/** Names the largest image either axis may reach before we warn about perf. */
export const HUGE_IMAGE_PX = 4096;

/**
 * The id becomes a folder name under ~/.codex/pets — reject anything that could
 * escape it. Unicode is fine (小红书 creators name skins in Chinese); separators,
 * dot-tricks, drive colons, and control chars are not. Mirrored in Rust.
 */
export function isSafeSkinId(id: string): boolean {
  if (id.length === 0 || id.length > 64) return false;
  if (id.startsWith('.') || id.includes('..')) return false;
  for (const ch of id) {
    if (ch === '/' || ch === '\\' || ch === ':') return false;
    const code = ch.codePointAt(0) ?? 0;
    if (code < 0x20 || code === 0x7f) return false;
  }
  return true;
}

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function positiveInt(v: unknown, fallback: number): number | null {
  const n = v === undefined ? fallback : v;
  return typeof n === 'number' && Number.isInteger(n) && n > 0 ? n : null;
}

export function validateSkin(raw: unknown, img: ImageDims): SkinValidation {
  const errors: SkinIssue[] = [];
  const warnings: SkinIssue[] = [];
  if (!isPlainObject(raw)) {
    return { errors: [{ key: 'notObject' }], warnings };
  }

  if (raw.id !== undefined && (typeof raw.id !== 'string' || !isSafeSkinId(raw.id))) {
    errors.push({ key: 'invalidId', params: { id: String(raw.id) } });
  }

  const sheet = raw.spritesheetPath;
  if (
    sheet !== undefined &&
    (typeof sheet !== 'string' ||
      sheet.includes('..') ||
      sheet.startsWith('/') ||
      sheet.includes('\\'))
  ) {
    errors.push({ key: 'unsafeSheetPath', params: { path: String(sheet) } });
  }

  // Effective atlas mirrors resolvePet(): absent fields inherit the defaults.
  const rawAtlas = isPlainObject(raw.atlas) ? raw.atlas : {};
  const cellW = positiveInt(rawAtlas.cellW, DEFAULT_ATLAS.cellW);
  const cellH = positiveInt(rawAtlas.cellH, DEFAULT_ATLAS.cellH);
  const cols = positiveInt(rawAtlas.cols, DEFAULT_ATLAS.cols);
  const rows = positiveInt(rawAtlas.rows, DEFAULT_ATLAS.rows);
  for (const [field, v] of [
    ['cellW', cellW],
    ['cellH', cellH],
    ['cols', cols],
    ['rows', rows],
  ] as const) {
    if (v === null) errors.push({ key: 'atlasField', params: { field } });
  }

  if (cellW !== null && cellH !== null && cols !== null && rows !== null) {
    if (cellW * cols > img.width) {
      errors.push({ key: 'atlasWiderThanImage', params: { need: cellW * cols, have: img.width } });
    } else if (cellW * cols !== img.width) {
      warnings.push({
        key: 'atlasRemainder',
        params: { axis: 'x', used: cellW * cols, actual: img.width },
      });
    }
    if (cellH * rows > img.height) {
      errors.push({
        key: 'atlasTallerThanImage',
        params: { need: cellH * rows, have: img.height },
      });
    } else if (cellH * rows !== img.height) {
      warnings.push({
        key: 'atlasRemainder',
        params: { axis: 'y', used: cellH * rows, actual: img.height },
      });
    }

    const anims = isPlainObject(raw.animations) ? raw.animations : null;
    const declaredNames = anims ? Object.keys(anims) : [];
    if (anims && declaredNames.length > 0) {
      if (!anims.idle) errors.push({ key: 'missingIdle' });
      for (const name of declaredNames) {
        const a = anims[name];
        if (!isPlainObject(a)) {
          errors.push({ key: 'badAnimation', params: { name } });
          continue;
        }
        // resolvePet defaults: row 0, frames 1, offsetCol 0.
        const row = a.row === undefined ? 0 : a.row;
        const frames = a.frames === undefined ? 1 : a.frames;
        const offsetCol = a.offsetCol === undefined ? 0 : a.offsetCol;
        if (typeof row !== 'number' || !Number.isInteger(row) || row < 0 || row >= rows) {
          errors.push({ key: 'rowOutOfRange', params: { name, row: String(row), rows } });
        }
        if (typeof frames !== 'number' || !Number.isInteger(frames) || frames < 1) {
          errors.push({ key: 'badFrames', params: { name, frames: String(frames) } });
        } else if (
          typeof offsetCol === 'number' &&
          Number.isInteger(offsetCol) &&
          offsetCol >= 0 &&
          offsetCol + frames > cols
        ) {
          errors.push({ key: 'framesOverflow', params: { name, need: offsetCol + frames, cols } });
        }
        if (typeof a.fps === 'number' && a.fps > 60) {
          warnings.push({ key: 'highFps', params: { name, fps: a.fps } });
        }
        if (!KNOWN_ANIMATION_NAMES.has(name)) {
          warnings.push({ key: 'unknownAnimation', params: { name } });
        }
      }
      const missing = Object.keys(STANDARD_ANIMATION_ROWS).filter((n) => !anims[n]);
      if (missing.length > 0) {
        warnings.push({ key: 'missingStandardRows', params: { rows: missing.join(', ') } });
      }
    } else if (rows < 9 || cols < 8) {
      // No animations declared → the standard 9×8 rows are inherited wholesale;
      // an atlas too small for them would render garbage at runtime.
      errors.push({ key: 'inheritedRowsExceedAtlas', params: { rows, cols } });
    }

    const stateMap = isPlainObject(raw.stateMap) ? raw.stateMap : null;
    if (stateMap && anims && declaredNames.length > 0) {
      for (const [state, target] of Object.entries(stateMap)) {
        if (typeof target === 'string' && !anims[target]) {
          warnings.push({ key: 'unknownStateMapTarget', params: { state, target } });
        }
      }
    }
  }

  if (img.width > HUGE_IMAGE_PX || img.height > HUGE_IMAGE_PX) {
    warnings.push({ key: 'hugeImage', params: { w: img.width, h: img.height } });
  }

  return { errors, warnings };
}
