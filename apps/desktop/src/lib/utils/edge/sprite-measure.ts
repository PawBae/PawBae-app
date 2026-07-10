import { animationFor, type CodexPet, type CodexPetState } from '../codex-pet';
import type { SpriteAnchorsCSS, SpritePad } from './types';

const SPRITE_PAD_BOTTOM_FRAC = 0.3;
const SPRITE_PAD_TOP_FRAC = 0.4;
const SPRITE_PAD_LEFT_FRAC = 0.45;
const SPRITE_PAD_RIGHT_FRAC = 0.45;

interface RuntimeSpritePad {
  topPx: number | null;
  rightPx: number | null;
  bottomPx: number | null;
  leftPx: number | null;
}

const runtimeSpritePad: RuntimeSpritePad = {
  topPx: null,
  rightPx: null,
  bottomPx: null,
  leftPx: null,
};

export function setRuntimeSpritePadCSS(values: Partial<SpriteAnchorsCSS>) {
  const apply = (key: keyof RuntimeSpritePad, v: number | null | undefined) => {
    if (v === undefined) return;
    if (v === null) {
      runtimeSpritePad[key] = null;
      return;
    }
    if (!Number.isFinite(v) || v < 0 || v > 1000) return;
    runtimeSpritePad[key] = v;
  };
  apply('topPx', values.topPx);
  apply('rightPx', values.rightPx);
  apply('bottomPx', values.bottomPx);
  apply('leftPx', values.leftPx);
}

export function resetRuntimeSpritePadCSS() {
  runtimeSpritePad.topPx = null;
  runtimeSpritePad.rightPx = null;
  runtimeSpritePad.bottomPx = null;
  runtimeSpritePad.leftPx = null;
}

export function spritePadFor(mascotW: number, mascotH: number): SpritePad {
  return {
    top: runtimeSpritePad.topPx ?? mascotH * SPRITE_PAD_TOP_FRAC,
    right: runtimeSpritePad.rightPx ?? mascotW * SPRITE_PAD_RIGHT_FRAC,
    bottom: runtimeSpritePad.bottomPx ?? mascotH * SPRITE_PAD_BOTTOM_FRAC,
    left: runtimeSpritePad.leftPx ?? mascotW * SPRITE_PAD_LEFT_FRAC,
  };
}

interface CellBBox {
  top: number;
  right: number;
  bottom: number;
  left: number;
  contactLeft: number;
  contactRight: number;
}
const cellBBoxCache = new Map<string, CellBBox | null>();

const ON_FLOOR_ANIM_KEYS = [
  'idle',
  'running',
  'run-right',
  'run-left',
  'waiting',
  'review',
] as const;
const ON_CEILING_ANIM_KEYS = ['climb-ceiling', 'climb-ceiling-flipped'] as const;
const ON_WALL_ANIM_KEYS = [
  'grab-wall',
  'grab-wall-flipped',
  'climb-wall',
  'climb-wall-flipped',
] as const;

export function scanCellBBox(
  img: HTMLImageElement,
  cellW: number,
  cellH: number,
  row: number,
  frameCount: number,
  offsetCol = 0,
): CellBBox | null {
  const canvas = document.createElement('canvas');
  canvas.width = cellW;
  canvas.height = cellH;
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;
  const ALPHA_THRESHOLD = 16;
  const SIDE_CONTACT_COVERAGE_RATIO = 0.2;
  const frames = Math.max(1, frameCount | 0);
  let aggTop = -1,
    aggBottom = -1,
    aggLeft = cellW,
    aggRight = -1;
  let anyOpaque = false;
  const columnCoverage = new Array<number>(cellW).fill(0);
  for (let frame = 0; frame < frames; frame++) {
    ctx.clearRect(0, 0, cellW, cellH);
    ctx.drawImage(img, (offsetCol + frame) * cellW, row * cellH, cellW, cellH, 0, 0, cellW, cellH);
    let data: Uint8ClampedArray;
    try {
      data = ctx.getImageData(0, 0, cellW, cellH).data;
    } catch {
      return null;
    }
    for (let y = 0; y < cellH; y++) {
      const rowStart = y * cellW * 4;
      for (let x = 0; x < cellW; x++) {
        if (data[rowStart + x * 4 + 3] >= ALPHA_THRESHOLD) {
          anyOpaque = true;
          columnCoverage[x] += 1;
          if (aggTop < 0 || y < aggTop) aggTop = y;
          if (y > aggBottom) aggBottom = y;
          if (x < aggLeft) aggLeft = x;
          if (x > aggRight) aggRight = x;
        }
      }
    }
  }
  if (!anyOpaque) return null;
  const maxCoverage = Math.max(...columnCoverage);
  const minContactCoverage = Math.max(1, maxCoverage * SIDE_CONTACT_COVERAGE_RATIO);
  const contactLeft = columnCoverage.findIndex((count) => count >= minContactCoverage);
  const contactRightFromEnd = [...columnCoverage]
    .reverse()
    .findIndex((count) => count >= minContactCoverage);
  return {
    top: aggTop,
    right: aggRight,
    bottom: aggBottom,
    left: aggLeft,
    contactLeft: contactLeft >= 0 ? contactLeft : aggLeft,
    contactRight: contactRightFromEnd >= 0 ? cellW - 1 - contactRightFromEnd : aggRight,
  };
}

export function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error(`Image load failed: ${url}`));
    img.src = url;
  });
}

export async function measureSpriteAnchorsCSS(pet: CodexPet): Promise<SpriteAnchorsCSS | null> {
  const anchor = document.querySelector('[data-physics-anchor]') as HTMLElement | null;
  if (!anchor) return null;
  const rect = anchor.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return null;

  const winInnerW = window.innerWidth;
  const winInnerH = window.innerHeight;
  if (winInnerW <= 0 || winInnerH <= 0) return null;

  const cellW = pet.atlas.cellW;
  const cellH = pet.atlas.cellH;
  if (cellW <= 0 || cellH <= 0) return null;

  const xScale = rect.width / cellW;
  const yScale = rect.height / cellH;
  const gapTop = rect.top;
  const gapBottom = winInnerH - rect.bottom;
  const gapLeft = rect.left;
  const gapRight = winInnerW - rect.right;
  if (!Number.isFinite(gapBottom) || !Number.isFinite(gapRight)) return null;

  const rowsFor = (keys: readonly string[]): Set<number> => {
    const out = new Set<number>();
    for (const k of keys) {
      const a = pet.animations[k];
      if (a) out.add(a.row);
    }
    return out;
  };
  const floorRows = rowsFor(ON_FLOOR_ANIM_KEYS);
  if (floorRows.size === 0) floorRows.add(0);
  const ceilingRows = rowsFor(ON_CEILING_ANIM_KEYS);
  const wallRows = rowsFor(ON_WALL_ANIM_KEYS);

  const rowFrameCount = new Map<number, number>();
  for (const a of Object.values(pet.animations)) {
    const prev = rowFrameCount.get(a.row) ?? 0;
    if (a.frames > prev) rowFrameCount.set(a.row, a.frames);
  }

  let img: HTMLImageElement | null = null;
  const getBBox = async (row: number): Promise<CellBBox | null> => {
    const cacheKey = `${pet.spritesheetUrl}#${row}`;
    if (cellBBoxCache.has(cacheKey)) return cellBBoxCache.get(cacheKey) ?? null;
    if (img === null) {
      try {
        img = await loadImage(pet.spritesheetUrl);
      } catch {
        return null;
      }
    }
    const frames = rowFrameCount.get(row) ?? 1;
    const bbox = scanCellBBox(img, cellW, cellH, row, frames);
    cellBBoxCache.set(cacheKey, bbox);
    return bbox;
  };

  let maxBottom = -1;
  for (const row of floorRows) {
    const bbox = await getBBox(row);
    if (bbox && bbox.bottom > maxBottom) maxBottom = bbox.bottom;
  }
  const bottomPx =
    maxBottom >= 0 ? Math.max(0, gapBottom + (cellH - 1 - maxBottom) * yScale) : null;

  let minTop = cellH;
  let anyCeilingScanned = false;
  for (const row of ceilingRows) {
    const bbox = await getBBox(row);
    if (bbox) {
      anyCeilingScanned = true;
      if (bbox.top < minTop) minTop = bbox.top;
    }
  }
  const topPx = anyCeilingScanned ? Math.max(0, gapTop + minTop * yScale) : null;

  let minSideGap = cellW;
  let anyWallScanned = false;
  for (const row of wallRows) {
    const bbox = await getBBox(row);
    if (!bbox) continue;
    anyWallScanned = true;
    minSideGap = Math.min(minSideGap, bbox.left, cellW - 1 - bbox.right);
  }
  const sideCellPad = anyWallScanned ? Math.max(0, minSideGap) : -1;
  const sideCSS = sideCellPad >= 0 ? sideCellPad * xScale : -1;
  const leftPx = anyWallScanned ? Math.max(0, gapLeft + sideCSS) : null;
  const rightPx = anyWallScanned ? Math.max(0, gapRight + sideCSS) : null;

  return { topPx, rightPx, bottomPx, leftPx };
}

export async function measureSpriteBottomPadCSS(
  pet: CodexPet,
  state: CodexPetState,
  renderWidth: number,
  renderHeight?: number,
): Promise<number | null> {
  const row = animationFor(pet, state) ?? animationFor(pet, 'idle');
  if (!row) return null;

  const cellW = pet.atlas.cellW;
  const cellH = pet.atlas.cellH;
  if (cellW <= 0 || cellH <= 0 || renderWidth <= 0) return null;

  let img: HTMLImageElement;
  try {
    img = await loadImage(pet.spritesheetUrl);
  } catch {
    return null;
  }

  const bbox = scanCellBBox(img, cellW, cellH, row.row, row.frames, row.offsetCol ?? 0);
  if (!bbox) return null;

  const cssScaleY =
    (renderHeight && renderHeight > 0 ? renderHeight : renderWidth * (cellH / cellW)) / cellH;
  const rowScale = row.displayScale ?? 1;
  return Math.max(0, (cellH - 1 - bbox.bottom) * cssScaleY * rowScale);
}
