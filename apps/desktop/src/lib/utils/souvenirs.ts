// Souvenir catalog + drop rules (Phase 1 Agent 冒险). Pure logic, zero Svelte/Tauri
// imports. Names and flavor text live in i18n (`souvenir.<id>.name/.flavor`); the
// catalog is the single source for ids, emoji and rarity. All 24 items are canonized
// on the cloud-visitor lore — see docs/lore/yoonie.md item 9 and the full bilingual
// table in docs/superpowers/specs/2026-07-08-agent-adventure-design.md.

export type SouvenirRarity = 'common' | 'rare' | 'legendary';

export interface SouvenirDef {
  id: string;
  emoji: string;
  rarity: SouvenirRarity;
}

/** Trips at least this long roll on the better drop table (期待感变现). */
export const LONG_TRIP_MS = 600_000;

// Order is display order on the shelf. IDs are persisted in pet.json — never rename.
export const SOUVENIR_CATALOG: readonly SouvenirDef[] = [
  // Common ×12.
  { id: 'cloud_fluff', emoji: '🌫️', rarity: 'common' },
  { id: 'ding_echo', emoji: '🔔', rarity: 'common' },
  { id: 'rounded_pebble', emoji: '🪨', rarity: 'common' },
  { id: 'steam_candy', emoji: '🍬', rarity: 'common' },
  { id: 'mist_ribbon', emoji: '🎀', rarity: 'common' },
  { id: 'star_crumbs', emoji: '✨', rarity: 'common' },
  { id: 'rain_seed', emoji: '💧', rarity: 'common' },
  { id: 'stray_feather', emoji: '🪶', rarity: 'common' },
  { id: 'tiny_ladder', emoji: '🪜', rarity: 'common' },
  { id: 'washed_note', emoji: '📃', rarity: 'common' },
  { id: 'wind_knot', emoji: '🌀', rarity: 'common' },
  { id: 'warm_button', emoji: '🔘', rarity: 'common' },
  // Rare ×8.
  { id: 'rain_smell_jar', emoji: '🫙', rarity: 'rare' },
  { id: 'moon_shaving', emoji: '🌙', rarity: 'rare' },
  { id: 'dud_thunder', emoji: '⚡', rarity: 'rare' },
  { id: 'cloud_bell', emoji: '🛎️', rarity: 'rare' },
  { id: 'fog_marble', emoji: '🔮', rarity: 'rare' },
  { id: 'sky_postcard', emoji: '💌', rarity: 'rare' },
  { id: 'dream_cotton', emoji: '☁️', rarity: 'rare' },
  { id: 'ding_whistle', emoji: '🎐', rarity: 'rare' },
  // Legendary ×4.
  { id: 'whole_kiwi', emoji: '🥝', rarity: 'legendary' },
  { id: 'first_ding', emoji: '🏮', rarity: 'legendary' },
  { id: 'cloud_key', emoji: '🗝️', rarity: 'legendary' },
  { id: 'bottled_aurora', emoji: '🌈', rarity: 'legendary' },
] as const;

// Cumulative rarity thresholds: a uniform roll below the first bound is common,
// below the second rare, else legendary. Longer trips shift odds toward the top.
const BASE_TABLE: readonly [number, number] = [0.78, 0.97]; // 78 / 19 / 3
const LONG_TABLE: readonly [number, number] = [0.6, 0.92]; // 60 / 32 / 8

/**
 * Roll one souvenir for a trip of `elapsedMs`. `rand` supplies uniform [0,1)
 * numbers (two draws: rarity, then item within the rarity) so tests are
 * deterministic and the caller owns the entropy.
 */
export function rollSouvenir(elapsedMs: number, rand: () => number): SouvenirDef {
  const table = Number.isFinite(elapsedMs) && elapsedMs >= LONG_TRIP_MS ? LONG_TABLE : BASE_TABLE;
  const r = rand();
  const rarity: SouvenirRarity = r < table[0] ? 'common' : r < table[1] ? 'rare' : 'legendary';
  const pool = SOUVENIR_CATALOG.filter((d) => d.rarity === rarity);
  const idx = Math.min(pool.length - 1, Math.max(0, Math.floor(rand() * pool.length)));
  return pool[idx];
}

export interface SouvenirOwned {
  count: number;
  /** Epoch ms of the first find — the shelf's "collected on" moment. */
  firstAt: number;
}

/**
 * Record a find: a repeat bumps the ×N count and keeps the original firstAt.
 * Returns a fresh null-prototype record (persisted-map discipline — a hostile
 * "__proto__" key stays an ordinary own key).
 */
export function addSouvenir(
  owned: Record<string, SouvenirOwned>,
  id: string,
  at: number,
): Record<string, SouvenirOwned> {
  const out: Record<string, SouvenirOwned> = Object.create(null);
  Object.assign(out, owned);
  const prev = out[id];
  out[id] = prev
    ? { count: prev.count + 1, firstAt: prev.firstAt }
    : { count: 1, firstAt: Number.isFinite(at) && at > 0 ? at : 0 };
  return out;
}

/**
 * Coerce a persisted shelf back to a safe shape: unknown ids are kept (forward
 * compat with newer builds), counts become positive integers, firstAt a finite
 * non-negative timestamp; anything else is dropped.
 */
export function sanitizeSouvenirs(raw: unknown): Record<string, SouvenirOwned> {
  const out: Record<string, SouvenirOwned> = Object.create(null);
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) return out;
  for (const [k, v] of Object.entries(raw as Record<string, unknown>)) {
    if (typeof v !== 'object' || v === null) continue;
    const count = Math.floor(Number((v as { count?: unknown }).count));
    const firstAt = Number((v as { firstAt?: unknown }).firstAt);
    if (!Number.isFinite(count) || count < 1) continue;
    out[k] = { count, firstAt: Number.isFinite(firstAt) && firstAt > 0 ? firstAt : 0 };
  }
  return out;
}
