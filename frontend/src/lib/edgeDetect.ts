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
}

const EDGE_TOLERANCE = 4

// Cache the monitor rect to avoid an IPC round-trip every physics
// tick (30 ms ticks × 1 IPC = ~33 calls/sec). The monitor rect changes
// rarely (display reconnect / window dragged across screens) so a
// short TTL is fine.
let cachedMonitor: { value: MonitorRect; expiresAt: number } | null = null
const MONITOR_CACHE_MS = 300

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
  const [monitor, mascot] = await Promise.all([
    getMonitorRect(forceMonitorRefresh),
    getMascotRect(),
  ])

  const onLeft = mascot.x - monitor.x <= EDGE_TOLERANCE
  const onRight =
    monitor.x + monitor.width - (mascot.x + mascot.width) <= EDGE_TOLERANCE

  // Vertical edges differ by platform.
  // macOS (bottom-up): bottom of screen is at monitor.y, so the visual
  //   bottom of the window is touching the floor when mascot.y is
  //   close to monitor.y. The visual top is touching when
  //   mascot.y + mascot.height >= monitor.y + monitor.height.
  // Windows (top-down): bottom is at monitor.y + monitor.height; top
  //   is at monitor.y.
  let onBottom: boolean
  let onTop: boolean
  if (isMacOS) {
    onBottom = mascot.y - monitor.y <= EDGE_TOLERANCE
    onTop =
      monitor.y + monitor.height - (mascot.y + mascot.height) <= EDGE_TOLERANCE
  } else {
    onTop = mascot.y - monitor.y <= EDGE_TOLERANCE
    onBottom =
      monitor.y + monitor.height - (mascot.y + mascot.height) <= EDGE_TOLERANCE
  }

  return { monitor, mascot, onLeft, onRight, onTop, onBottom }
}
