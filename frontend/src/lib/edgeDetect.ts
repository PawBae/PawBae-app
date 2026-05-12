// Detect which screen edges the mini window is currently touching.
// Used by the pet physics loop to drive transitions between
// on_floor / on_wall / on_ceiling / falling.
//
// The Rust commands `get_mini_origin` and `get_mini_monitor_rect` both
// report coordinates in the OS-native frame:
//   - macOS: bottom-up Cocoa coordinates. `origin.y` is the bottom edge
//     of the window; `screen.origin.y` is the bottom of the screen.
//     Moving "down" visually means *decreasing* `origin.y`.
//   - Windows: top-down. (Stroll mode is gated to macOS only, but we
//     keep the API symmetric so the helper isn't macOS-specific.)
//
// The physics loop talks to `move_mini_by(dx, dy)` where `dy > 0` means
// "move down on screen" regardless of platform — the Rust side flips
// the sign for Cocoa internally. So we stay consistent here: edge
// detection works in OS-native coords, but physics velocity vectors
// are always top-down.

import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { animationFor, type CodexPet, type CodexPetState } from './codexPet'

export interface MonitorRect {
  // Origin (bottom-left on macOS, top-left on Windows).
  x: number
  y: number
  width: number
  height: number
}

export interface MascotRect {
  // Window origin in OS-native frame (bottom-left on macOS).
  x: number
  y: number
  width: number
  height: number
}

export interface EdgeState {
  monitor: MonitorRect
  mascot: MascotRect
  // Each of these is "touching or past" the edge with a ≤ EDGE_TOLERANCE
  // gap. on_top means the window's *upper* edge is at the top of the
  // screen — this is the visual "ceiling" regardless of coordinate
  // system, because we always offset from the right side of the height
  // axis appropriate to the platform.
  onLeft: boolean
  onRight: boolean
  onTop: boolean
  onBottom: boolean
  // Frontmost app window (Finder, Safari, …) the pet can interact with
  // as a second world. `null` when no qualifying window is on-screen
  // (everything minimized, only utility panels visible, …) or when the
  // physics loop is on a non-macOS platform.
  activeWindow: ActiveWindowEdge | null
}

/// Frontmost-window surface info — mirror of the screen-edge fields,
/// but anchored to a single app window's rect. The geometry treats the
/// window's top edge as a *floor* (the pet stands on the title bar),
/// the window's left/right verticals as *walls* (the pet grabs them
/// like screen edges), and the window's bottom edge as a *ceiling* (the
/// pet hangs upside-down from underneath).
export interface ActiveWindowEdge {
  /// Cocoa bottom-left window rect, in the same coord frame as
  /// `EdgeState.mascot`.
  rect: { x: number; y: number; width: number; height: number }
  /// Stable CGWindowID; the physics loop watches this to detect
  /// "the window I was sitting on disappeared".
  windowId: number
  /// e.g. "Finder", "Safari" — purely informational for now.
  ownerName: string
  ownerPid: number
  /// Visible sprite foot is resting on the window's top edge from above.
  /// This is the "sit on title bar" condition.
  onTopOfWindow: boolean
  /// Visible sprite right side is flush with the window's left vertical
  /// (pet sits to the left of the window, body touching its left edge).
  onLeftOfWindow: boolean
  /// Visible sprite left side is flush with the window's right vertical.
  onRightOfWindow: boolean
  /// Visible sprite head is resting against the window's bottom edge
  /// from below (pet hangs upside-down from underneath).
  onBottomOfWindow: boolean
  /// Visible body's horizontal span overlaps the window's horizontal
  /// range. Required to validate "stand on top" — if the pet has walked
  /// off the side of the title bar, this goes false even when
  /// `onTopOfWindow` is true at the y level, and the pet should fall.
  withinHorizontalRange: boolean
  /// Visible body's vertical span overlaps the window's vertical range.
  /// Required to validate side-grab — if the pet is too far above or
  /// below the window, side proximity alone doesn't mean it's gripping
  /// a wall (it would be floating in air).
  withinVerticalRange: boolean
}

const EDGE_TOLERANCE = 4

// Sprite content padding within the (transparent) window box, as
// fractions of window size. Lets the *visible* character sit flush
// with screen edges by allowing the window to overshoot visibleFrame
// by the corresponding transparent gap. Tuned for shimeji-bola's
// 128×128 atlas (window 96 px, cell→window scale 0.75) where the cat
// occupies roughly the top-left ~75×80 of each idle cell:
//   - idle pose has ~28.5 px of empty space below the cat in a 96 px
//     window (cell bottom of sprite at y≈90 → 67.5 px from window top).
//   - climbing pose head at cell-y≈5 → ~3.75 px from window top.
//   - left/right edges of sprite at cell-x≈10/118 → ~7.5 px inset.
// These values are intentionally per-physics-pet defaults; once a
// second physics pet ships, lift them into pet.json.physics.bbox.
const SPRITE_PAD_BOTTOM_FRAC = 0.30
// Top pad — pushed to 0.40. The climb-ceiling sprite anchors the
// cat's head much deeper into the cell than estimated; the gap was
// still visible at 0.18 per user. If 0.40 now makes the cat clip
// past the menu bar (head disappears behind it), dial back toward
// 0.30. If still a gap, push to 0.50.
const SPRITE_PAD_TOP_FRAC = 0.40
// Side pads — symmetric at 0.45 for shimeji-bola: the grab-wall /
// climb-wall body is significantly inset on whichever side the cat
// is rendered. With the per-side flipped variants the rendering is
// mirror-symmetric, so left and right need the same overshoot to
// reach the screen edge. If the cat now clips past either edge,
// dial both back toward 0.40.
const SPRITE_PAD_LEFT_FRAC = 0.45
const SPRITE_PAD_RIGHT_FRAC = 0.45

export interface SpritePad {
  top: number
  right: number
  bottom: number
  left: number
}

// Runtime-measured absolute pad overrides, in CSS pixels — one per
// edge. Each answers exactly "how many CSS pixels lie between the
// visible edge of the sprite (on this side) and the corresponding
// window edge?". When the physics loop subtracts these from the
// visibleFrame edges, the visible sprite touches the screen edge
// flush by construction.
//
// `null` for any field means "no measurement yet" → `spritePadFor`
// falls back to the per-pet hardcoded fraction for that side. The
// fallback only matters before the first DOM frame; once
// `measureSpriteAnchorsCSS` has run, all four are populated for any
// pet that declares the conventional on-floor / on-wall / on-ceiling
// animation keys.
interface RuntimeSpritePad {
  topPx: number | null
  rightPx: number | null
  bottomPx: number | null
  leftPx: number | null
}

const runtimeSpritePad: RuntimeSpritePad = {
  topPx: null,
  rightPx: null,
  bottomPx: null,
  leftPx: null,
}

export interface SpriteAnchorsCSS {
  topPx: number | null
  rightPx: number | null
  bottomPx: number | null
  leftPx: number | null
}

export function setRuntimeSpritePadCSS(values: Partial<SpriteAnchorsCSS>) {
  // Sanity-clamp. Any pad must be >= 0 (can't push the sprite past
  // the window) and shouldn't exceed 1000 px (covers the largest
  // collapsed mascot window we'd ever ship). Silently reject bad
  // values so a buggy measurement can't push the cat off-screen.
  const apply = (key: keyof RuntimeSpritePad, v: number | null | undefined) => {
    if (v === undefined) return
    if (v === null) { runtimeSpritePad[key] = null; return }
    if (!Number.isFinite(v) || v < 0 || v > 1000) return
    runtimeSpritePad[key] = v
  }
  apply('topPx', values.topPx)
  apply('rightPx', values.rightPx)
  apply('bottomPx', values.bottomPx)
  apply('leftPx', values.leftPx)
}

// Clear every measured pad. Called when physics re-enables for a new
// pet so the next tick uses the fraction fallback until the new
// measurement lands, rather than the stale measurement from whatever
// pet was selected before.
export function resetRuntimeSpritePadCSS() {
  runtimeSpritePad.topPx = null
  runtimeSpritePad.rightPx = null
  runtimeSpritePad.bottomPx = null
  runtimeSpritePad.leftPx = null
}

// Resolve the sprite content padding in *window pixels* for a mascot of
// the given outer size. Each side prefers its runtime-measured
// absolute CSS-pixel value when available, otherwise falls back to
// the per-pet default fraction × the relevant window dimension.
export function spritePadFor(mascotW: number, mascotH: number): SpritePad {
  return {
    top: runtimeSpritePad.topPx ?? (mascotH * SPRITE_PAD_TOP_FRAC),
    right: runtimeSpritePad.rightPx ?? (mascotW * SPRITE_PAD_RIGHT_FRAC),
    bottom: runtimeSpritePad.bottomPx ?? (mascotH * SPRITE_PAD_BOTTOM_FRAC),
    left: runtimeSpritePad.leftPx ?? (mascotW * SPRITE_PAD_LEFT_FRAC),
  }
}

// Cache of the opaque bounding box per (spritesheetUrl, row). Atlas
// pixel geometry is immutable per pet, so this is a permanent
// session-level cache.
interface CellBBox {
  top: number      // 0-indexed Y of topmost opaque pixel
  right: number    // 0-indexed X of rightmost opaque pixel
  bottom: number   // 0-indexed Y of lowest opaque pixel
  left: number     // 0-indexed X of leftmost opaque pixel
  // Side-contact columns ignore wispy extremities such as ears, tails,
  // shadows, and antialiased pixels. Window-wall detection uses these
  // instead of raw left/right so the mascot grips the visible window
  // border with its body/paws rather than a decorative outlier pixel.
  contactLeft: number
  contactRight: number
}
const cellBBoxCache = new Map<string, CellBBox | null>()

// Animation-key conventions for each edge. Different pets declare
// different rows for the same role, so we look them up by name and
// scan whichever rows the pet actually has. A pet that omits a role
// (e.g. no climb-ceiling animation) returns `null` for that edge and
// `spritePadFor` keeps using the fraction fallback for that side.
const ON_FLOOR_ANIM_KEYS = [
  'idle', 'running', 'run-right', 'run-left', 'waiting', 'review',
] as const
const ON_CEILING_ANIM_KEYS = [
  'climb-ceiling', 'climb-ceiling-flipped',
] as const
const ON_WALL_ANIM_KEYS = [
  'grab-wall', 'grab-wall-flipped', 'climb-wall', 'climb-wall-flipped',
] as const

// Measure the absolute CSS-pixel offset between the rendered sprite's
// visible edges and the surrounding window edges, for all four sides.
// The physics loop subtracts each from the corresponding visibleFrame
// edge so the visible character sits flush with the screen edges.
//
// Two measurements combine for every edge:
//   1. Alpha scan of every relevant animation row → the cell-coord
//      bounding box of opaque pixels for each pose.
//   2. DOM read of the rendered sprite's bounding rect (via
//      `[data-physics-anchor]`) for the actual sprite layout inside
//      the window — captures any centering offset that the
//      fraction-of-window-size formula would miss.
//
// Per-edge logic:
//   - bottom: max lowestOpaqueY across on-floor rows. The pose with
//     the lowest-hanging foot lands exactly on the Dock; other poses
//     float slightly above (never sink into the Dock).
//   - top: min topmostOpaqueY across on-ceiling rows. The pose with
//     the topmost head reaches the menubar; others stay slightly
//     below (never clip past it).
//   - left / right: wall-pose contact column, not the raw bbox side.
//     Raw bbox sides include decorative extremities, which makes the
//     pet visually hover away from screen/window borders. The contact
//     column requires enough vertical alpha coverage to behave like
//     the body/paw edge that actually grips the wall.
//
// Returns `null` when the DOM anchor isn't in the tree yet — caller
// should retry. Individual edge fields may be `null` if the pet
// doesn't declare any animations for that role (e.g. a floor-only
// pet has no climb rows → `topPx` and side fields stay null).
export async function measureSpriteAnchorsCSS(
  pet: CodexPet,
): Promise<SpriteAnchorsCSS | null> {
  const anchor = document.querySelector(
    '[data-physics-anchor]',
  ) as HTMLElement | null
  if (!anchor) return null
  const rect = anchor.getBoundingClientRect()
  if (rect.width <= 0 || rect.height <= 0) return null

  const winInnerW = window.innerWidth
  const winInnerH = window.innerHeight
  if (winInnerW <= 0 || winInnerH <= 0) return null

  const cellW = pet.atlas.cellW
  const cellH = pet.atlas.cellH
  if (cellW <= 0 || cellH <= 0) return null

  const xScale = rect.width / cellW
  const yScale = rect.height / cellH
  const gapTop = rect.top
  const gapBottom = winInnerH - rect.bottom
  const gapLeft = rect.left
  const gapRight = winInnerW - rect.right
  if (!Number.isFinite(gapBottom) || !Number.isFinite(gapRight)) return null

  // Collect unique row indices for each role.
  const rowsFor = (keys: readonly string[]): Set<number> => {
    const out = new Set<number>()
    for (const k of keys) {
      const a = pet.animations[k]
      if (a) out.add(a.row)
    }
    return out
  }
  const floorRows = rowsFor(ON_FLOOR_ANIM_KEYS)
  if (floorRows.size === 0) floorRows.add(0) // fallback so bottom always tries row 0
  const ceilingRows = rowsFor(ON_CEILING_ANIM_KEYS)
  const wallRows = rowsFor(ON_WALL_ANIM_KEYS)

  // Frame count per row — the max declared `frames` of any animation
  // that points at this row. Climb animations (4 frames) can extend
  // paws further on some frames than frame 0 alone shows, so the
  // bounding box must be unioned across every frame to capture the
  // tightest fit.
  const rowFrameCount = new Map<number, number>()
  for (const a of Object.values(pet.animations)) {
    const prev = rowFrameCount.get(a.row) ?? 0
    if (a.frames > prev) rowFrameCount.set(a.row, a.frames)
  }

  // Lazy-load the atlas once and cache the bounding box per row.
  let img: HTMLImageElement | null = null
  const getBBox = async (row: number): Promise<CellBBox | null> => {
    const cacheKey = `${pet.spritesheetUrl}#${row}`
    if (cellBBoxCache.has(cacheKey)) return cellBBoxCache.get(cacheKey) ?? null
    if (img === null) {
      try { img = await loadImage(pet.spritesheetUrl) }
      catch { return null }
    }
    const frames = rowFrameCount.get(row) ?? 1
    const bbox = scanCellBBox(img, cellW, cellH, row, frames)
    cellBBoxCache.set(cacheKey, bbox)
    return bbox
  }

  // Bottom — max lowestOpaqueY across on-floor rows.
  let maxBottom = -1
  for (const row of floorRows) {
    const bbox = await getBBox(row)
    if (bbox && bbox.bottom > maxBottom) maxBottom = bbox.bottom
  }
  const bottomPx = maxBottom >= 0
    ? Math.max(0, gapBottom + (cellH - 1 - maxBottom) * yScale)
    : null

  // Top — min topmostOpaqueY across on-ceiling rows. If the pet has no
  // ceiling animations, leave topPx null and let `spritePadFor` use
  // the fraction fallback (irrelevant for floor-only pets anyway).
  let minTop = cellH
  let anyCeilingScanned = false
  for (const row of ceilingRows) {
    const bbox = await getBBox(row)
    if (bbox) { anyCeilingScanned = true; if (bbox.top < minTop) minTop = bbox.top }
  }
  const topPx = anyCeilingScanned
    ? Math.max(0, gapTop + minTop * yScale)
    : null

  // Sides — visible silhouette across wall rows. Wall sprites can be
  // rendered either unflipped or CSS-flipped depending on whether the
  // pet is clinging to a screen edge or a window side. Runtime edge
  // detection stores one left/right pad for the pet, not per-state
  // flip-aware pads, so use the smallest transparent horizontal gap
  // across both sides of the unflipped cell. That is conservative for
  // both the native and mirrored renders: the pet may sit a little
  // farther from the edge, but no head/ear pixels disappear.
  //
  // Earlier versions used a body/paw "contact column" so the paws could
  // sit exactly on the border, but Yoonie's long ears extend past that
  // contact column. The native window is clipped by screen edges, so
  // contact-column padding pushed the ear outside the visible frame on
  // walls. Full-silhouette padding keeps the whole character visible.
  let minSideGap = cellW
  let anyWallScanned = false
  for (const row of wallRows) {
    const bbox = await getBBox(row)
    if (!bbox) continue
    anyWallScanned = true
    minSideGap = Math.min(minSideGap, bbox.left, cellW - 1 - bbox.right)
  }
  const sideCellPad = anyWallScanned ? Math.max(0, minSideGap) : -1
  const sideCSS = sideCellPad >= 0 ? sideCellPad * xScale : -1
  const leftPx = anyWallScanned ? Math.max(0, gapLeft + sideCSS) : null
  const rightPx = anyWallScanned ? Math.max(0, gapRight + sideCSS) : null

  return { topPx, rightPx, bottomPx, leftPx }
}

// Measure how much transparent atlas padding sits below the visible pixels
// for a specific rendered animation. The coding-mode mini mascot uses this
// to visually anchor user-selected sprite pets to the native window bottom:
// CSS layout anchors the full atlas cell, but many pets intentionally leave
// transparent room below their feet inside each cell.
export async function measureSpriteBottomPadCSS(
  pet: CodexPet,
  state: CodexPetState,
  renderWidth: number,
  renderHeight?: number,
): Promise<number | null> {
  const row = animationFor(pet, state) ?? animationFor(pet, 'idle')
  if (!row) return null

  const cellW = pet.atlas.cellW
  const cellH = pet.atlas.cellH
  if (cellW <= 0 || cellH <= 0 || renderWidth <= 0) return null

  let img: HTMLImageElement
  try {
    img = await loadImage(pet.spritesheetUrl)
  } catch {
    return null
  }

  const bbox = scanCellBBox(img, cellW, cellH, row.row, row.frames, row.offsetCol ?? 0)
  if (!bbox) return null

  const cssScaleY = (renderHeight && renderHeight > 0 ? renderHeight : renderWidth * (cellH / cellW)) / cellH
  const rowScale = row.displayScale ?? 1
  return Math.max(0, (cellH - 1 - bbox.bottom) * cssScaleY * rowScale)
}

// Scan every frame of a row and return the unioned bounding box of
// pixels with alpha >= 16/255 (~6% opacity) across all frames. The
// threshold rejects antialiased edge halos and shadow pixels that
// would otherwise puff the apparent reach past the artist-intended
// silhouette. Returns null when every frame is fully transparent.
//
// Unioning across frames matters for animated poses where a limb
// extends further on some frames than others (climb-wall's paw
// reaching during a stride, climb-ceiling's tail flicking, etc.).
// Scanning frame 0 alone could under-pad and leave a gap on the
// extending frames.
function scanCellBBox(
  img: HTMLImageElement,
  cellW: number,
  cellH: number,
  row: number,
  frameCount: number,
  offsetCol = 0,
): CellBBox | null {
  const canvas = document.createElement('canvas')
  canvas.width = cellW
  canvas.height = cellH
  const ctx = canvas.getContext('2d')
  if (!ctx) return null
  const ALPHA_THRESHOLD = 16
  const SIDE_CONTACT_COVERAGE_RATIO = 0.2
  const frames = Math.max(1, frameCount | 0)
  let aggTop = -1, aggBottom = -1, aggLeft = cellW, aggRight = -1
  let anyOpaque = false
  const columnCoverage = new Array<number>(cellW).fill(0)
  for (let frame = 0; frame < frames; frame++) {
    ctx.clearRect(0, 0, cellW, cellH)
    ctx.drawImage(
      img,
      (offsetCol + frame) * cellW, row * cellH, cellW, cellH,
      0, 0, cellW, cellH,
    )
    let data: Uint8ClampedArray
    try {
      data = ctx.getImageData(0, 0, cellW, cellH).data
    } catch {
      return null
    }
    for (let y = 0; y < cellH; y++) {
      const rowStart = y * cellW * 4
      for (let x = 0; x < cellW; x++) {
        if (data[rowStart + x * 4 + 3] >= ALPHA_THRESHOLD) {
          anyOpaque = true
          columnCoverage[x] += 1
          if (aggTop < 0 || y < aggTop) aggTop = y
          if (y > aggBottom) aggBottom = y
          if (x < aggLeft) aggLeft = x
          if (x > aggRight) aggRight = x
        }
      }
    }
  }
  if (!anyOpaque) return null
  const maxCoverage = Math.max(...columnCoverage)
  const minContactCoverage = Math.max(1, maxCoverage * SIDE_CONTACT_COVERAGE_RATIO)
  const contactLeft = columnCoverage.findIndex((count) => count >= minContactCoverage)
  const contactRightFromEnd = [...columnCoverage].reverse().findIndex((count) => count >= minContactCoverage)
  return {
    top: aggTop,
    right: aggRight,
    bottom: aggBottom,
    left: aggLeft,
    contactLeft: contactLeft >= 0 ? contactLeft : aggLeft,
    contactRight: contactRightFromEnd >= 0 ? cellW - 1 - contactRightFromEnd : aggRight,
  }
}

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    // crossOrigin: anonymous so getImageData doesn't taint the canvas
    // when the asset protocol serves cross-origin headers. Built-in
    // pets are same-origin under /assets, but custom pets come through
    // a custom asset:// scheme — set crossOrigin defensively.
    img.crossOrigin = 'anonymous'
    img.onload = () => resolve(img)
    img.onerror = () => reject(new Error(`Image load failed: ${url}`))
    img.src = url
  })
}

// Cache the monitor rect to avoid an IPC round-trip every physics
// tick (30 ms ticks × 1 IPC = ~33 calls/sec). The monitor rect changes
// rarely (display reconnect / window dragged across screens) so a
// short TTL is fine.
let cachedMonitor: { value: MonitorRect; expiresAt: number } | null = null
const MONITOR_CACHE_MS = 300

// Pet-floor info from Rust. The pet has a piecewise floor: on top of
// the Dock where the Dock exists, falling through to the actual screen
// bottom past either end of the Dock's horizontal extent.
export interface PetFloorInfo {
  // ns/bottom-up logical y of the floor when over the Dock (== top of
  // Dock).
  onDockY: number
  // ns/bottom-up logical y of the floor when off the Dock (== actual
  // screen bottom).
  offDockY: number
  // Horizontal extent of the Dock window (left x, right x) in screen
  // coords, or null when no Dock is detected (auto-hide engaged or
  // Dock missing). When null the entire screen width is "off-Dock" so
  // the pet falls to the screen bottom.
  dockXRange: [number, number] | null
}

let cachedFloor: { value: PetFloorInfo; expiresAt: number } | null = null
const FLOOR_CACHE_MS = 300

export async function getFloorInfo(force = false): Promise<PetFloorInfo> {
  const now = performance.now()
  if (!force && cachedFloor && cachedFloor.expiresAt > now) {
    return cachedFloor.value
  }
  const raw = await invoke<{
    on_dock_y: number
    off_dock_y: number
    dock_x_range: [number, number] | null
  }>('get_pet_floor_info')
  const value: PetFloorInfo = {
    onDockY: raw.on_dock_y,
    offDockY: raw.off_dock_y,
    dockXRange: raw.dock_x_range,
  }
  cachedFloor = { value, expiresAt: now + FLOOR_CACHE_MS }
  return value
}

export function invalidateFloorCache() {
  cachedFloor = null
}

export async function getMonitorRect(force = false): Promise<MonitorRect> {
  const now = performance.now()
  if (!force && cachedMonitor && cachedMonitor.expiresAt > now) {
    return cachedMonitor.value
  }
  const tuple = (await invoke('get_mini_monitor_rect')) as [number, number, number, number]
  const value: MonitorRect = { x: tuple[0], y: tuple[1], width: tuple[2], height: tuple[3] }
  cachedMonitor = { value, expiresAt: now + MONITOR_CACHE_MS }
  return value
}

export function invalidateMonitorCache() {
  cachedMonitor = null
}

// Active foreground app window — used to give the pet a second world to
// interact with (Shimeji-style). The Rust side caches at 50 ms; we
// mirror that here so two ticks within the same TTL share a single
// decode. `null` means no qualifying window right now (everything
// minimized, only utility windows visible) or non-macOS platform.
interface AppWindowInfoRaw {
  window_id: number
  owner_name: string
  owner_pid: number
  x: number
  y: number
  width: number
  height: number
}

let cachedActiveWindow: { value: AppWindowInfoRaw | null; expiresAt: number } | null = null
const ACTIVE_WINDOW_CACHE_MS = 50

export async function getActiveAppWindow(force = false): Promise<AppWindowInfoRaw | null> {
  const now = performance.now()
  if (!force && cachedActiveWindow && cachedActiveWindow.expiresAt > now) {
    return cachedActiveWindow.value
  }
  if (!isMacOS) {
    cachedActiveWindow = { value: null, expiresAt: now + ACTIVE_WINDOW_CACHE_MS }
    return null
  }
  try {
    const raw = await invoke<AppWindowInfoRaw | null>('get_frontmost_app_window')
    cachedActiveWindow = { value: raw, expiresAt: now + ACTIVE_WINDOW_CACHE_MS }
    return raw
  } catch {
    // IPC error during shutdown / window-not-ready — treat as "no window".
    cachedActiveWindow = { value: null, expiresAt: now + ACTIVE_WINDOW_CACHE_MS }
    return null
  }
}

export function invalidateActiveWindowCache() {
  cachedActiveWindow = null
}

// Read the mascot's current screen-frame origin and outer size.
export async function getMascotRect(): Promise<MascotRect> {
  const win = getCurrentWebviewWindow()
  const [origin, outerSize, scale] = await Promise.all([
    invoke('get_mini_origin') as Promise<[number, number]>,
    win.outerSize(),
    win.scaleFactor(),
  ])
  // outerSize is PhysicalSize on every platform; convert to logical
  // pixels because get_mini_origin already returns logical coords.
  const width = outerSize.width / scale
  const height = outerSize.height / scale
  return { x: origin[0], y: origin[1], width, height }
}

const isMacOS =
  typeof navigator !== 'undefined'
  && /Mac|Macintosh|Mac OS/.test(navigator.userAgent)

export async function detectEdges(forceMonitorRefresh = false): Promise<EdgeState> {
  // On macOS we also pull the piecewise floor info so onBottom respects
  // the Dock's actual horizontal extent: the pet stands on top of the
  // Dock where it exists and falls to the actual screen bottom off
  // either end of the Dock.
  //
  // We also fetch the frontmost app window in parallel — used by the
  // physics loop to give the pet a second world to climb on. Rust caches
  // at 50 ms so most ticks hit the cache, but the IPC always happens
  // (the round-trip is cheap relative to the physics tick budget).
  const [monitor, mascot, floor, activeWindowRaw] = await Promise.all([
    getMonitorRect(forceMonitorRefresh),
    getMascotRect(),
    isMacOS ? getFloorInfo(forceMonitorRefresh) : Promise.resolve(null as PetFloorInfo | null),
    isMacOS ? getActiveAppWindow(forceMonitorRefresh) : Promise.resolve(null as AppWindowInfoRaw | null),
  ])

  // Virtually extend the monitor rect outward by the sprite-content
  // padding so edge flags fire when the *visible character* (not the
  // transparent window box) is at a screen edge. Bottom and right pads
  // use mascot width/height because the window is square but the
  // padding is per-axis. On macOS this also means the window can sit
  // physically below the floor by `pad.bottom` pixels — the overshoot
  // is invisible because it lands in the Dock area where the window
  // is transparent.
  const pad = spritePadFor(mascot.width, mascot.height)
  const exX = monitor.x - pad.left
  const exW = monitor.width + pad.left + pad.right

  const onLeft = mascot.x - exX <= EDGE_TOLERANCE
  const onRight =
    exX + exW - (mascot.x + mascot.width) <= EDGE_TOLERANCE

  let onBottom: boolean
  let onTop: boolean
  if (isMacOS) {
    // Piecewise floor: on top of the Dock where the Dock spans, else at
    // the actual screen bottom. Falls back to visibleFrame.y when no
    // Dock detected (auto-hide or no Dock).
    const centerX = mascot.x + mascot.width / 2
    let floorYNs = monitor.y // top of visibleFrame; default when no floor info
    if (floor) {
      // When Dock detection fails (dockXRange is null) we treat the
      // entire visibleFrame width as a platform — the pet still sits
      // on the Dock-top line instead of plummeting past it. Piecewise
      // "fall off the side" behavior only engages when a Dock x-range
      // was actually detected.
      const overDock =
        floor.dockXRange === null
        || (centerX >= floor.dockXRange[0] && centerX <= floor.dockXRange[1])
      floorYNs = overDock ? floor.onDockY : floor.offDockY
    }
    const exBottomY = floorYNs - pad.bottom
    const exTopY =
      monitor.y + monitor.height + pad.top - mascot.height
    onBottom = mascot.y - exBottomY <= EDGE_TOLERANCE
    onTop = exTopY - mascot.y <= EDGE_TOLERANCE
  } else {
    const exTopY = monitor.y - pad.top
    onTop = mascot.y - exTopY <= EDGE_TOLERANCE
    onBottom =
      exTopY + monitor.height + pad.top + pad.bottom
        - (mascot.y + mascot.height) <= EDGE_TOLERANCE
  }

  // Compute the active-window surface flags. The pet treats the window
  // as a second world: its top edge is a floor (pet sits on the title
  // bar), its sides are walls, and its bottom edge is a ceiling.
  //
  // Geometry mirrors the screen edges: each "on*OfWindow" answers "is
  // the *visible character* (after sprite padding) flush with this
  // edge of the window?". We reuse the same `pad` and `EDGE_TOLERANCE`
  // so a cat that sits flush on the Dock also sits flush on a Finder
  // title bar.
  //
  // Skip windows that are *behind* the screen Dock area or *under* the
  // menu bar (top edge inside the menu-bar zone): standing on them
  // would clip the cat through OS chrome. Use `monitor.height +
  // monitor.y` as the menu-bar lower boundary on macOS and skip if the
  // window's top edge is past that (i.e. clipped behind the menu bar).
  let activeWindow: ActiveWindowEdge | null = null
  if (isMacOS && activeWindowRaw) {
    const w = activeWindowRaw
    const winLeft = w.x
    const winRight = w.x + w.width
    const winBottom = w.y
    const winTop = w.y + w.height

    // Visible sprite edges in mascot-window-origin frame.
    const spriteFootY = mascot.y + pad.bottom               // cocoa-y of foot
    const spriteHeadY = mascot.y + mascot.height - pad.top  // cocoa-y of head
    const spriteLeftX = mascot.x + pad.left
    const spriteRightX = mascot.x + mascot.width - pad.right

    // "On top of window" — sprite foot is at window's top edge from above.
    // The pet must also be horizontally within the window's footprint;
    // otherwise it's hovering past the title bar and should fall.
    const onTopOfWindow = Math.abs(spriteFootY - winTop) <= EDGE_TOLERANCE
    // Permissive overlap (>= / <=, no EDGE_TOLERANCE buffer) so the
    // pet can stand on the title bar all the way to the corner. With a
    // strict buffer the moment the pet climbed up a side and hit the
    // corner it would immediately re-detach because its body was just
    // at the threshold, not 4 px inside.
    const withinHorizontalRange =
      spriteRightX >= winLeft
      && spriteLeftX <= winRight
    const withinVerticalRange =
      spriteHeadY >= winBottom
      && spriteFootY <= winTop

    // Pet on window's LEFT side: visible body's right edge touches the
    // window's left vertical from outside. Pet's body is *to the left*
    // of the window, facing right (toward the window's interior).
    const onLeftOfWindow = Math.abs(spriteRightX - winLeft) <= EDGE_TOLERANCE
    // Pet on window's RIGHT side: visible body's left edge touches the
    // window's right vertical. Pet sits to the *right* of the window.
    const onRightOfWindow = Math.abs(spriteLeftX - winRight) <= EDGE_TOLERANCE

    // Pet hanging upside-down from window's bottom: sprite head touches
    // window's bottom edge from below.
    const onBottomOfWindow = Math.abs(spriteHeadY - winBottom) <= EDGE_TOLERANCE

    // Drop windows whose top edge would clip the cat into the menu
    // bar. visibleFrame.y + visibleFrame.height = top of visibleFrame
    // (i.e. bottom of menu bar) on macOS. If the window's title bar is
    // above that, standing on it puts the cat behind the menu bar.
    const menuBarBottomY = monitor.y + monitor.height
    const clipsMenuBar = winTop > menuBarBottomY - EDGE_TOLERANCE

    if (!clipsMenuBar) {
      activeWindow = {
        rect: { x: w.x, y: w.y, width: w.width, height: w.height },
        windowId: w.window_id,
        ownerName: w.owner_name,
        ownerPid: w.owner_pid,
        onTopOfWindow,
        onLeftOfWindow,
        onRightOfWindow,
        onBottomOfWindow,
        withinHorizontalRange,
        withinVerticalRange,
      }
    }
  }

  return { monitor, mascot, onLeft, onRight, onTop, onBottom, activeWindow }
}
