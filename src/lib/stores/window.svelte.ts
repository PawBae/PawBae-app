import { invoke } from '@tauri-apps/api/core';

let expanded = $state(false);
let mascotHover = $state(false);
let moveMode = $state(false);
let strollActive = $state(false);
let pinned = $state(false);
let settingsOpen = $state(false);

async function setExpanded(v: boolean, mascotScale: number = 1) {
  expanded = v;
  try {
    await invoke('set_mini_expanded', { expanded: v, mascotScale });
  } catch (e) {
    console.warn('[window] set_mini_expanded failed:', e);
  }
}

async function moveBy(dx: number, dy: number) {
  try {
    await invoke('move_mini_by', { dx, dy });
  } catch {
    // ignore
  }
}

async function setOrigin(x: number, y: number) {
  try {
    await invoke('set_mini_origin', { x, y });
  } catch {
    // ignore
  }
}

async function getOrigin(): Promise<{ x: number; y: number } | null> {
  try {
    return (await invoke('get_mini_origin')) as { x: number; y: number };
  } catch {
    return null;
  }
}

async function getMonitorRect(): Promise<{ x: number; y: number; w: number; h: number } | null> {
  try {
    return (await invoke('get_mini_monitor_rect')) as { x: number; y: number; w: number; h: number };
  } catch {
    return null;
  }
}

async function openMini() {
  try {
    await invoke('open_mini');
  } catch {
    // ignore
  }
}

async function closeMini() {
  try {
    await invoke('close_mini');
  } catch {
    // ignore
  }
}

async function reassertFloating() {
  try {
    await invoke('reassert_floating');
  } catch {
    // ignore
  }
}

function setMascotHover(v: boolean) {
  mascotHover = v;
}

function setMoveMode(v: boolean) {
  moveMode = v;
}

function setStrollActive(v: boolean) {
  strollActive = v;
}

function setPinned(v: boolean) {
  pinned = v;
}

function setSettingsOpen(v: boolean) {
  settingsOpen = v;
}

function toggle() {
  setExpanded(!expanded);
}

export const windowStore = {
  get expanded() { return expanded; },
  get mascotHover() { return mascotHover; },
  get moveMode() { return moveMode; },
  get strollActive() { return strollActive; },
  get pinned() { return pinned; },
  get settingsOpen() { return settingsOpen; },
  setExpanded,
  moveBy,
  setOrigin,
  getOrigin,
  getMonitorRect,
  openMini,
  closeMini,
  reassertFloating,
  setMascotHover,
  setMoveMode,
  setStrollActive,
  setPinned,
  setSettingsOpen,
  toggle,
};
