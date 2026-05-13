// Sprite-pet asset model.
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
  fps?: number
  loopRestMs?: number
  flipX?: boolean
  offsetCol?: number
  displayScale?: number
}

export interface PhysicsSpec {
  enabled?: boolean
}

export type SpriteImageRendering = 'auto' | 'pixelated' | 'crisp-edges'

export type MiniPetSourceState = 'idle' | 'working' | 'compacting' | 'waiting'

export interface CodexPet {
  id: string
  displayName: string
  description: string
  spritesheetUrl: string
  schemaVersion?: number
  atlas: AtlasSpec
  animations: Record<string, AnimationRow>
  stateMap: Record<MiniPetSourceState, string>
  oneShot: Set<string>
  physics?: PhysicsSpec
  displayScale?: number
  imageRendering: SpriteImageRendering
}

export const DEFAULT_ATLAS: AtlasSpec = {
  cellW: 192,
  cellH: 208,
  cols: 8,
  rows: 9,
} as const

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

export const STATE_FPS: Partial<Record<CodexStandardState, number>> = {
  idle: 2,
  jumping: 6,
  running: 6,
  waiting: 6,
  'run-left': 8,
  'run-right': 8,
}

export const STATE_LOOP_REST_MS: Partial<Record<CodexStandardState, number>> = {
  waiting: 600,
}

export function fpsFor(pet: CodexPet, state: CodexPetState): number {
  const declared = pet.animations[state]?.fps
  if (typeof declared === 'number' && declared > 0) return declared
  return STATE_FPS[state as CodexStandardState] ?? SPRITE_FPS
}

export function loopRestMsFor(pet: CodexPet, state: CodexPetState): number {
  const declared = pet.animations[state]?.loopRestMs
  if (typeof declared === 'number' && declared >= 0) return declared
  return STATE_LOOP_REST_MS[state as CodexStandardState] ?? 0
}

export function petStateToCodexState(
  pet: CodexPet | null | undefined,
  state: MiniPetSourceState,
): CodexPetState {
  if (!pet) return DEFAULT_STATE_MAP[state]
  const mapped = pet.stateMap[state]
  if (mapped && pet.animations[mapped]) return mapped
  return DEFAULT_STATE_MAP[state]
}

export function animationFor(pet: CodexPet, state: CodexPetState): AnimationRow | undefined {
  return pet.animations[state]
}

export const DEFAULT_PET_ID = 'phoebe'

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
  displayScale?: number
  imageRendering?: SpriteImageRendering
}

interface PetsManifest {
  pets: string[]
}

let cachedPets: Promise<CodexPet[]> | null = null

function resolvePet(meta: RawPetMeta, fallbackId: string, spritesheetUrl: string): CodexPet {
  const atlas: AtlasSpec = {
    cellW: meta.atlas?.cellW ?? DEFAULT_ATLAS.cellW,
    cellH: meta.atlas?.cellH ?? DEFAULT_ATLAS.cellH,
    cols: meta.atlas?.cols ?? DEFAULT_ATLAS.cols,
    rows: meta.atlas?.rows ?? DEFAULT_ATLAS.rows,
  }

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
        displayScale: v.displayScale,
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
    displayScale: meta.displayScale,
    imageRendering: meta.imageRendering ?? 'auto',
  }
}

export function loadCodexPets(): Promise<CodexPet[]> {
  if (!cachedPets) {
    cachedPets = (async () => {
      try {
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
      } catch (e) {
        console.error('[codexPet] loadCodexPets failed:', e)
        return []
      }
    })()
  }
  return cachedPets
}

export async function loadDefaultCodexPet(): Promise<CodexPet | null> {
  const pets = await loadCodexPets()
  if (pets.length === 0) return null
  return pets.find((p) => p.id === DEFAULT_PET_ID) ?? pets[0]
}
