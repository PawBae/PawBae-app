// Skin workshop pure helpers. A "skin" is a full CodexPet character folder — the
// engine model is reused untouched; these helpers only glue the custom-pet pipeline
// (~/.codex/pets via the codexpet:// protocol) into the gallery UI.
// See docs/superpowers/specs/2026-07-09-skin-workshop-design.md.

import { type CodexPet, DEFAULT_PET_ID } from './codex-pet';

/**
 * Builtin skins pulled from the bundle before closed beta — copyrighted character
 * sprites (see docs/superpowers/specs/2026-07-10-pet-interaction-creation-design.md §7).
 * The ids stay listed here so installs that persisted one migrate to the default pet.
 */
export const REMOVED_BUILTIN_PET_IDS: ReadonlySet<string> = new Set([
  'doro.codex-pet',
  'elaina-2',
  'homie',
  'linnea-2',
  'mambo',
  'naruto',
  'nezuko',
  'phoebe.codex-pet',
  'skirk-2',
  'taffy',
]);

/**
 * Map a persisted mini_pet_id to a shippable one: removed builtins fall back to the
 * default pet (Yoonie — original character, no IP risk). Custom skin ids pass through
 * untouched — customs are user content and never in the removed set.
 */
export function migrateMiniPetId(id: string | null | undefined): string {
  if (!id) return DEFAULT_PET_ID;
  return REMOVED_BUILTIN_PET_IDS.has(id) ? DEFAULT_PET_ID : id;
}

/**
 * Derive the pet.json URL from the platform-correct spritesheet URL Rust returns
 * (`codexpet://localhost/<id>/...` on macOS/Linux, `http://codexpet.localhost/<id>/...`
 * on Windows). pet.json always sits at the skin folder root, so take the first path
 * segment after the host — NOT the sheet's directory, which may be a subfolder.
 * Reusing Rust's URL keeps the platform branching in one place.
 */
export function petJsonUrlFromSheetUrl(sheetUrl: string): string | null {
  const parts = sheetUrl.split('/');
  // scheme: + '' + host + <id> + at least one file segment
  if (parts.length < 5 || !parts[3]) return null;
  return `${parts.slice(0, 4).join('/')}/pet.json`;
}

/**
 * Builtins first, customs layered on top — an id collision means the custom skin
 * wins, mirroring import's overwrite-by-id upgrade semantics.
 */
export function mergeSkins(
  builtins: readonly CodexPet[],
  customs: readonly CodexPet[],
): CodexPet[] {
  const out = new Map<string, CodexPet>();
  for (const p of builtins) out.set(p.id, p);
  for (const p of customs) out.set(p.id, p);
  return [...out.values()];
}

/**
 * Static gallery-tile crop: the first frame of the idle row (or the first declared
 * animation), scaled so one cell is `tileW` wide. Same background math as
 * SpritePet.svelte, minus the animation loop — a grid of looping sprites would burn
 * a rAF per tile for no benefit.
 */
export function tileFrameStyle(pet: CodexPet, tileW: number): string {
  const anim = pet.animations.idle ?? Object.values(pet.animations)[0];
  const row = anim?.row ?? 0;
  const offsetCol = anim?.offsetCol ?? 0;
  const cellW = Math.max(1, Math.round(tileW));
  const cellH = Math.max(1, Math.round(cellW * (pet.atlas.cellH / pet.atlas.cellW)));
  return [
    `width:${cellW}px`,
    `height:${cellH}px`,
    `background-image:url("${pet.spritesheetUrl}")`,
    'background-repeat:no-repeat',
    `background-size:${cellW * pet.atlas.cols}px ${cellH * pet.atlas.rows}px`,
    `background-position:${-offsetCol * cellW}px ${-row * cellH}px`,
    `image-rendering:${pet.imageRendering}`,
  ].join(';');
}
