// Sprite-pet asset model.
//
// Originally tied to the openai/skills hatch-pet output format
// (192×208 cells, 8 cols × 9 rows, fixed 9-state row layout). The schema
// has since been extended (schemaVersion: 2) so a pet.json can declare
// its own atlas dimensions, animation rows/frame counts, state mapping
// and physics flags. Pets that omit the new fields still load via
// `resolvePet` which fills in the legacy hatch-pet defaults.

// Strict union of canonical hatch-pet states. Used internally when we
// need exhaustiveness against the legacy 9-state layout (e.g. fallback
// tables). External consumers should treat the runtime state as
// `CodexPetState = string`, since extended pets may declare arbitrary
// animation keys.
export type CodexStandardState =
  | 'idle'
  | 'run-right'
  | 'run-left'
  | 'waving'
  | 'jumping'
  | 'failed'
  | 'waiting'
  | 'running'
  | 'review'

// Runtime state — any string a pet.json declares in its `animations`
// map. Falls back to STANDARD_ANIMATION_ROWS keys when not declared.
export type CodexPetState = string

export interface AtlasSpec {
  cellW: number
  cellH: number
  cols: number
  rows: number
}

export interface AnimationRow {
  row: number
  frames: number
  // Optional per-state fps override (otherwise falls back to STATE_FPS
  // table or SPRITE_FPS).
  fps?: number
  // Optional ms to hold the last frame before restarting the loop.
  loopRestMs?: number
  // Horizontally flip the rendered cell — used to derive run-left from
  // run-right when the pack only contains right-facing frames.
  flipX?: boolean
  // Optional column offset within the row, useful when several short
  // animations are packed into a single row (e.g. shimeji-bola row 7
  // bundles falling/jumping/bouncing).
  offsetCol?: number
}

export interface PhysicsSpec {
  enabled?: boolean
}

export type MiniPetSourceState = 'idle' | 'working' | 'compacting' | 'waiting'

export interface CodexPet {
  id: string
  displayName: string
  description: string
  // Resolved absolute URL ready to use as a CSS background-image source.
  spritesheetUrl: string
  schemaVersion?: number
  atlas: AtlasSpec
  animations: Record<string, AnimationRow>
  // Maps Mini.tsx's high-level pet state → animation key declared in
  // `animations`. Walking direction (run-right/run-left) is layered on
  // top of this by the wrapper component.
  stateMap: Record<MiniPetSourceState, string>
  // Animation keys that should play once and hold the last frame
  // instead of looping. Default (`['jumping']`) matches the hatch-pet
  // contract.
  oneShot: Set<string>
  physics?: PhysicsSpec
}

// Default hatch-pet atlas (legacy 192×208 cells, 8 cols × 9 rows). Used
// as a fallback when pet.json doesn't declare its own atlas.
export const DEFAULT_ATLAS: AtlasSpec = {
  cellW: 192,
  cellH: 208,
  cols: 8,
  rows: 9,
} as const

// Standard hatch-pet row layout. Frame counts come from the canonical
// references/animation-rows.md contract used by the curated skill. Used
// as a fallback when pet.json doesn't declare its own `animations`.
export const STANDARD_ANIMATION_ROWS: Record<CodexStandardState, AnimationRow> = {
  'idle':      { row: 0, frames: 6 },
  'run-right': { row: 1, frames: 8 },
  'run-left':  { row: 2, frames: 8 },
  'waving':    { row: 3, frames: 4 },
  'jumping':   { row: 4, frames: 5 },
  'failed':    { row: 5, frames: 8 },
  'waiting':   { row: 6, frames: 6 },
  'running':   { row: 7, frames: 6 },
  'review':    { row: 8, frames: 6 },
}

export const DEFAULT_STATE_MAP: Record<MiniPetSourceState, string> = {
  idle: 'idle',
  working: 'running',
  compacting: 'running',
  waiting: 'waiting',
}

export const DEFAULT_ONE_SHOT_STATES: ReadonlySet<string> = new Set(['jumping'])

export const SPRITE_FPS = 12

// Per-state fps overrides for hatch-pet rows. States not listed fall
// back to SPRITE_FPS. Idle is intentionally slow (subtle breathing-style
// loop). Jumping plays slower than the default 12fps so the 5-frame
// animation reads clearly during the brief one-shot. Run/walk-style
// states are also softened so the dragging mascot doesn't feel jittery.
export const STATE_FPS: Partial<Record<CodexStandardState, number>> = {
  idle: 2,
  jumping: 6,
  running: 6,
  waiting: 6,
  'run-left': 8,
  'run-right': 8,
}

// Per-state inter-cycle rest (ms) for legacy hatch-pet states. After
// completing a cycle, SpritePet holds the last frame for this duration
// before restarting from frame 0. Lets passive states like `waiting`
// read as repeated bursts with stillness in between (matching the jump
// cadence) instead of a continuous animation that feels too busy.
export const STATE_LOOP_REST_MS: Partial<Record<CodexStandardState, number>> = {
  waiting: 600,
}

// fps lookup that's pet-aware.
//   1) pet.animations[state].fps if declared
//   2) STATE_FPS[state] for legacy hatch-pet rows
//   3) SPRITE_FPS default
export function fpsFor(pet: CodexPet, state: CodexPetState): number {
  const declared = pet.animations[state]?.fps
  if (typeof declared === 'number' && declared > 0) return declared
  return STATE_FPS[state as CodexStandardState] ?? SPRITE_FPS
}

// Loop-rest lookup that's pet-aware.
export function loopRestMsFor(pet: CodexPet, state: CodexPetState): number {
  const declared = pet.animations[state]?.loopRestMs
  if (typeof declared === 'number' && declared >= 0) return declared
  return STATE_LOOP_REST_MS[state as CodexStandardState] ?? 0
}

// Map Mini.tsx's PetState → an animation key on this pet. Falls back
// to the legacy default mapping when the pet doesn't declare a custom
// stateMap, or when no pet is loaded yet (lets consumers compute a
// state eagerly without a null guard).
export function petStateToCodexState(
  pet: CodexPet | null | undefined,
  state: MiniPetSourceState,
): CodexPetState {
  if (!pet) return DEFAULT_STATE_MAP[state]
  const mapped = pet.stateMap[state]
  if (mapped && pet.animations[mapped]) return mapped
  return DEFAULT_STATE_MAP[state]
}

// Lookup an animation row by name on a pet, returning undefined if the
// pet doesn't declare that animation. Used by MiniPetMascot to gate the
// hover-jump fallback timer (pets without a `jumping` row simply
// disable that interaction).
export function animationFor(pet: CodexPet, state: CodexPetState): AnimationRow | undefined {
  return pet.animations[state]
}

export const DEFAULT_PET_ID = 'phoebe'

// Default agent rotation queue used when the user hasn't customised one.
// Phoebe leads (matches the onboarding hero) and the rest follow the
// manifest order, capped at 10 entries.
export const DEFAULT_PET_QUEUE_IDS: string[] = [
  'phoebe',
  'doro',
  'elaina',
  'homie',
  'linnea',
  'mambo',
  'naruto',
  'nezuko',
  'skirk',
  'taffy',
]

const BUILTIN_BASE = '/assets/builtin'
const MANIFEST_URL = `${BUILTIN_BASE}/pets-manifest.json`

interface RawPetMeta {
  id?: string
  displayName?: string
  description?: string
  spritesheetPath?: string
  schemaVersion?: number
  atlas?: Partial<AtlasSpec>
  animations?: Record<string, Partial<AnimationRow>>
  stateMap?: Partial<Record<MiniPetSourceState, string>>
  oneShot?: string[]
  physics?: PhysicsSpec
}

interface PetsManifest {
  pets: string[]
}

let cachedPets: Promise<CodexPet[]> | null = null

// Fill in legacy hatch-pet defaults for any field the pet.json omits.
// Keeps every previously-shipped pet (built-in + custom) loadable
// without modification.
function resolvePet(meta: RawPetMeta, fallbackId: string, spritesheetUrl: string): CodexPet {
  const atlas: AtlasSpec = {
    cellW: meta.atlas?.cellW ?? DEFAULT_ATLAS.cellW,
    cellH: meta.atlas?.cellH ?? DEFAULT_ATLAS.cellH,
    cols: meta.atlas?.cols ?? DEFAULT_ATLAS.cols,
    rows: meta.atlas?.rows ?? DEFAULT_ATLAS.rows,
  }

  // Animations: shallow-merge declared rows over the standard hatch-pet
  // table when no custom animations field is provided. When the pet
  // declares animations explicitly, that map is authoritative (don't
  // mix in standard rows that may not exist in the custom atlas).
  let animations: Record<string, AnimationRow>
  if (meta.animations && Object.keys(meta.animations).length > 0) {
    animations = {}
    for (const [k, v] of Object.entries(meta.animations)) {
      animations[k] = {
        row: v.row ?? 0,
        frames: v.frames ?? 1,
        fps: v.fps,
        loopRestMs: v.loopRestMs,
        flipX: v.flipX,
        offsetCol: v.offsetCol,
      }
    }
  } else {
    animations = { ...STANDARD_ANIMATION_ROWS }
  }

  const stateMap: Record<MiniPetSourceState, string> = {
    idle: meta.stateMap?.idle ?? DEFAULT_STATE_MAP.idle,
    working: meta.stateMap?.working ?? DEFAULT_STATE_MAP.working,
    compacting: meta.stateMap?.compacting ?? DEFAULT_STATE_MAP.compacting,
    waiting: meta.stateMap?.waiting ?? DEFAULT_STATE_MAP.waiting,
  }

  const oneShot = new Set<string>(
    Array.isArray(meta.oneShot) ? meta.oneShot : Array.from(DEFAULT_ONE_SHOT_STATES),
  )

  return {
    id: meta.id || fallbackId,
    displayName: meta.displayName || fallbackId,
    description: meta.description || '',
    spritesheetUrl,
    schemaVersion: meta.schemaVersion ?? 1,
    atlas,
    animations,
    stateMap,
    oneShot,
    physics: meta.physics,
  }
}

export function loadCodexPets(): Promise<CodexPet[]> {
  if (!cachedPets) {
    cachedPets = (async () => {
      const manifestRes = await fetch(MANIFEST_URL)
      if (!manifestRes.ok) {
        throw new Error(`pets-manifest.json fetch failed: ${manifestRes.status}`)
      }
      const manifest = (await manifestRes.json()) as PetsManifest
      const ids = Array.isArray(manifest.pets) ? manifest.pets : []

      const results = await Promise.all(
        ids.map(async (id): Promise<CodexPet | null> => {
          try {
            const res = await fetch(`${BUILTIN_BASE}/${id}/pet.json`)
            if (!res.ok) return null
            const meta = (await res.json()) as RawPetMeta
            const sheet = meta.spritesheetPath ?? 'spritesheet.webp'
            return resolvePet(meta, id, `${BUILTIN_BASE}/${id}/${sheet}`)
          } catch {
            return null
          }
        }),
      )
      return results.filter((p): p is CodexPet => p !== null)
    })()
  }
  return cachedPets
}

export async function loadCodexPetById(id: string): Promise<CodexPet | null> {
  const builtins = await loadCodexPets()
  const hit = builtins.find((p) => p.id === id)
  if (hit) return hit
  const customs = await loadCustomCodexPets()
  return customs.find((p) => p.id === id) ?? null
}

export async function loadDefaultCodexPet(): Promise<CodexPet | null> {
  const pets = await loadCodexPets()
  if (pets.length === 0) return null
  return pets.find((p) => p.id === DEFAULT_PET_ID) ?? pets[0]
}

// Reset the manifest cache so the next call rescans `pets-manifest.json`.
// Used by the picker's refresh button after the user drops in new assets.
export function clearCodexPetCache(): void {
  cachedPets = null
}

// Pets dropped into `~/.codex/pets/` by the user. Loaded via the Rust
// `list_custom_codex_pets` command which serves them through the asset
// protocol so they render outside the bundled `public/` tree. Custom
// pets that ship a richer pet.json (with custom atlas/animations) are
// supported transparently — the Rust side returns the URL to their
// pet.json which we re-fetch here to get the full schema.
export async function loadCustomCodexPets(): Promise<CodexPet[]> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const raw = (await invoke('list_custom_codex_pets')) as Array<{
      id: string
      displayName: string
      description: string
      spritesheetUrl: string
      petJsonUrl?: string
    }>
    const out: CodexPet[] = []
    for (const m of raw) {
      let meta: RawPetMeta = {}
      if (m.petJsonUrl) {
        try {
          const res = await fetch(m.petJsonUrl)
          if (res.ok) meta = (await res.json()) as RawPetMeta
        } catch {
          /* fall through to defaults */
        }
      }
      // Rust-supplied fields win for identity/url; pet.json contributes
      // schema (atlas, animations, stateMap, oneShot, physics).
      out.push(
        resolvePet(
          {
            ...meta,
            id: m.id,
            displayName: m.displayName,
            description: m.description,
          },
          m.id,
          m.spritesheetUrl,
        ),
      )
    }
    return out
  } catch (e) {
    console.warn('[codexPet] loadCustomCodexPets failed:', e)
    return []
  }
}
