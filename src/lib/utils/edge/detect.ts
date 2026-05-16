import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { spritePadFor } from './sprite-measure';
import type { EdgeState, MascotRect, MonitorRect, PetFloorInfo } from './types';

export {
  loadImage,
  measureSpriteAnchorsCSS,
  measureSpriteBottomPadCSS,
  resetRuntimeSpritePadCSS,
  scanCellBBox,
  setRuntimeSpritePadCSS,
  spritePadFor,
} from './sprite-measure';

const EDGE_TOLERANCE = 4;

let cachedMonitor: { value: MonitorRect; expiresAt: number } | null = null;
const MONITOR_CACHE_MS = 300;

let cachedFloor: { value: PetFloorInfo; expiresAt: number } | null = null;
const FLOOR_CACHE_MS = 300;

interface AppWindowInfoRaw {
  window_id: number;
  owner_name: string;
  owner_pid: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

let cachedActiveWindow: { value: AppWindowInfoRaw | null; expiresAt: number } | null = null;
const ACTIVE_WINDOW_CACHE_MS = 50;

const isMacOS =
  typeof navigator !== 'undefined' && /Mac|Macintosh|Mac OS/.test(navigator.userAgent);

export async function getFloorInfo(force = false): Promise<PetFloorInfo> {
  const now = performance.now();
  if (!force && cachedFloor && cachedFloor.expiresAt > now) {
    return cachedFloor.value;
  }
  const raw = await invoke<{
    on_dock_y: number;
    off_dock_y: number;
    dock_x_range: [number, number] | null;
  }>('get_pet_floor_info');
  const value: PetFloorInfo = {
    onDockY: raw.on_dock_y,
    offDockY: raw.off_dock_y,
    dockXRange: raw.dock_x_range,
  };
  cachedFloor = { value, expiresAt: now + FLOOR_CACHE_MS };
  return value;
}

export function invalidateFloorCache() {
  cachedFloor = null;
}

export async function getMonitorRect(force = false): Promise<MonitorRect> {
  const now = performance.now();
  if (!force && cachedMonitor && cachedMonitor.expiresAt > now) {
    return cachedMonitor.value;
  }
  const tuple = (await invoke('get_mini_monitor_rect')) as [number, number, number, number];
  const value: MonitorRect = { x: tuple[0], y: tuple[1], width: tuple[2], height: tuple[3] };
  cachedMonitor = { value, expiresAt: now + MONITOR_CACHE_MS };
  return value;
}

export function invalidateMonitorCache() {
  cachedMonitor = null;
}

export async function getActiveAppWindow(force = false): Promise<AppWindowInfoRaw | null> {
  const now = performance.now();
  if (!force && cachedActiveWindow && cachedActiveWindow.expiresAt > now) {
    return cachedActiveWindow.value;
  }
  if (!isMacOS) {
    cachedActiveWindow = { value: null, expiresAt: now + ACTIVE_WINDOW_CACHE_MS };
    return null;
  }
  try {
    const raw = await invoke<AppWindowInfoRaw | null>('get_frontmost_app_window');
    cachedActiveWindow = { value: raw, expiresAt: now + ACTIVE_WINDOW_CACHE_MS };
    return raw;
  } catch {
    cachedActiveWindow = { value: null, expiresAt: now + ACTIVE_WINDOW_CACHE_MS };
    return null;
  }
}

export function invalidateActiveWindowCache() {
  cachedActiveWindow = null;
}

export async function getMascotRect(): Promise<MascotRect> {
  const win = getCurrentWebviewWindow();
  const [origin, outerSize, scale] = await Promise.all([
    invoke('get_mini_origin') as Promise<[number, number]>,
    win.outerSize(),
    win.scaleFactor(),
  ]);
  const width = outerSize.width / scale;
  const height = outerSize.height / scale;
  return { x: origin[0], y: origin[1], width, height };
}

export async function detectEdges(forceMonitorRefresh = false): Promise<EdgeState> {
  const [monitor, mascot, floor, activeWindowRaw] = await Promise.all([
    getMonitorRect(forceMonitorRefresh),
    getMascotRect(),
    isMacOS ? getFloorInfo(forceMonitorRefresh) : Promise.resolve(null as PetFloorInfo | null),
    isMacOS
      ? getActiveAppWindow(forceMonitorRefresh)
      : Promise.resolve(null as AppWindowInfoRaw | null),
  ]);

  const pad = spritePadFor(mascot.width, mascot.height);
  const exX = monitor.x - pad.left;
  const exW = monitor.width + pad.left + pad.right;

  const onLeft = mascot.x - exX <= EDGE_TOLERANCE;
  const onRight = exX + exW - (mascot.x + mascot.width) <= EDGE_TOLERANCE;

  let onBottom: boolean;
  let onTop: boolean;
  if (isMacOS) {
    const centerX = mascot.x + mascot.width / 2;
    let floorYNs = monitor.y;
    if (floor) {
      const overDock =
        floor.dockXRange === null ||
        (centerX >= floor.dockXRange[0] && centerX <= floor.dockXRange[1]);
      floorYNs = overDock ? floor.onDockY : floor.offDockY;
    }
    const exBottomY = floorYNs - pad.bottom;
    const exTopY = monitor.y + monitor.height + pad.top - mascot.height;
    onBottom = mascot.y - exBottomY <= EDGE_TOLERANCE;
    onTop = exTopY - mascot.y <= EDGE_TOLERANCE;
  } else {
    const exTopY = monitor.y - pad.top;
    onTop = mascot.y - exTopY <= EDGE_TOLERANCE;
    onBottom =
      exTopY + monitor.height + pad.top + pad.bottom - (mascot.y + mascot.height) <= EDGE_TOLERANCE;
  }

  let activeWindow: EdgeState['activeWindow'] = null;
  if (isMacOS && activeWindowRaw) {
    const w = activeWindowRaw;
    const winLeft = w.x;
    const winRight = w.x + w.width;
    const winBottom = w.y;
    const winTop = w.y + w.height;

    const spriteFootY = mascot.y + pad.bottom;
    const spriteHeadY = mascot.y + mascot.height - pad.top;
    const spriteLeftX = mascot.x + pad.left;
    const spriteRightX = mascot.x + mascot.width - pad.right;

    const onTopOfWindow = Math.abs(spriteFootY - winTop) <= EDGE_TOLERANCE;
    const withinHorizontalRange = spriteRightX >= winLeft && spriteLeftX <= winRight;
    const withinVerticalRange = spriteHeadY >= winBottom && spriteFootY <= winTop;
    const onLeftOfWindow = Math.abs(spriteRightX - winLeft) <= EDGE_TOLERANCE;
    const onRightOfWindow = Math.abs(spriteLeftX - winRight) <= EDGE_TOLERANCE;
    const onBottomOfWindow = Math.abs(spriteHeadY - winBottom) <= EDGE_TOLERANCE;

    const menuBarBottomY = monitor.y + monitor.height;
    const clipsMenuBar = winTop > menuBarBottomY - EDGE_TOLERANCE;

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
      };
    }
  }

  return { monitor, mascot, onLeft, onRight, onTop, onBottom, activeWindow };
}
