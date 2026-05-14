import { load } from '@tauri-apps/plugin-store';
import type { AppMode, OcConnection } from '../types';

let appMode = $state<AppMode | null>(null);
let soundEnabled = $state(true);
let codexSoundEnabled = $state(true);
let cursorSoundEnabled = $state(false);
let mascotScale = $state(1);
let largeMascotScale = $state(5);
let viewMode = $state<'efficiency' | 'island'>('efficiency');
let enableClaudeCode = $state(true);
let enableCodex = $state(true);
let enableCursor = $state(true);
let language = $state('en');
let strollEnabled = $state(true);
let panelMaxHeight = $state(400);
let notifySound = $state<'default' | 'manbo'>('default');
let waitingSound = $state(true);
let autoCloseCompletion = $state(false);
let autoExpandOnTask = $state(true);
let hoverDelay = $state(0.3);
let petSfxEnabled = $state(true);
let petIdleIntervalMin = $state(2);

let miniPetId = $state<string>('yoonie');
let petQueue = $state<string[]>([]);

let ocConnections = $state<OcConnection[]>([{ id: 'local', type: 'local' }]);

let storeInstance: Awaited<ReturnType<typeof load>> | null = null;

async function getStore() {
  if (!storeInstance) {
    storeInstance = await load('settings.json', { defaults: {}, autoSave: true });
  }
  return storeInstance;
}

async function loadSettings() {
  const store = await getStore();

  appMode = ((await store.get('app_mode')) as AppMode) || null;
  soundEnabled = ((await store.get('sound_enabled')) as boolean) ?? true;
  codexSoundEnabled = ((await store.get('codex_sound_enabled')) as boolean) ?? true;
  cursorSoundEnabled = ((await store.get('cursor_sound_enabled')) as boolean) ?? false;
  mascotScale = ((await store.get('mascot_scale')) as number) ?? 1;
  largeMascotScale = ((await store.get('large_mascot_scale')) as number) ?? 5;
  viewMode = ((await store.get('view_mode')) as 'efficiency' | 'island') || 'efficiency';
  enableClaudeCode = ((await store.get('enable_claude_code')) as boolean) ?? true;
  enableCodex = ((await store.get('enable_codex')) as boolean) ?? true;
  enableCursor = ((await store.get('enable_cursor')) as boolean) ?? true;
  language = ((await store.get('pawbae-lang')) as string) || 'en';
  strollEnabled = ((await store.get('stroll_mode_enabled')) as boolean) ?? true;
  panelMaxHeight = ((await store.get('panel_max_height')) as number) ?? 400;
  notifySound = ((await store.get('notify_sound')) as 'default' | 'manbo') || 'default';
  waitingSound = ((await store.get('waiting_sound')) as boolean) ?? true;
  autoCloseCompletion = ((await store.get('auto_close_completion')) as boolean) ?? false;
  autoExpandOnTask = ((await store.get('auto_expand_on_task')) as boolean) ?? true;
  hoverDelay = ((await store.get('hover_delay')) as number) ?? 0.3;
  petSfxEnabled = ((await store.get('pet_sfx_enabled')) as boolean) ?? true;
  petIdleIntervalMin = ((await store.get('pet_idle_interval_min')) as number) ?? 2;
  miniPetId = ((await store.get('mini_pet_id')) as string) || 'yoonie';
  petQueue = ((await store.get('pet_queue')) as string[]) || [];
  ocConnections = ((await store.get('oc_connections')) as OcConnection[]) || [{ id: 'local', type: 'local' }];
}

async function saveSetting(key: string, value: unknown) {
  const store = await getStore();
  await store.set(key, value);
  await store.save();
}

async function setAppMode(mode: AppMode) {
  appMode = mode;
  await saveSetting('app_mode', mode);
}

async function setSoundEnabled(v: boolean) {
  soundEnabled = v;
  await saveSetting('sound_enabled', v);
}

async function setCodexSoundEnabled(v: boolean) {
  codexSoundEnabled = v;
  await saveSetting('codex_sound_enabled', v);
}

async function setCursorSoundEnabled(v: boolean) {
  cursorSoundEnabled = v;
  await saveSetting('cursor_sound_enabled', v);
}

async function setMascotScale(v: number) {
  mascotScale = v;
  await saveSetting('mascot_scale', v);
}

async function setLargeMascotScale(v: number) {
  largeMascotScale = v;
  await saveSetting('large_mascot_scale', v);
}

async function setViewMode(v: 'efficiency' | 'island') {
  viewMode = v;
  await saveSetting('view_mode', v);
}

async function setEnableClaudeCode(v: boolean) {
  enableClaudeCode = v;
  await saveSetting('enable_claude_code', v);
}

async function setEnableCodex(v: boolean) {
  enableCodex = v;
  await saveSetting('enable_codex', v);
}

async function setEnableCursor(v: boolean) {
  enableCursor = v;
  await saveSetting('enable_cursor', v);
}

async function setLanguage(v: string) {
  language = v;
  await saveSetting('pawbae-lang', v);
}

async function setStrollEnabled(v: boolean) {
  strollEnabled = v;
  await saveSetting('stroll_mode_enabled', v);
}

async function setPanelMaxHeight(v: number) {
  panelMaxHeight = v;
  await saveSetting('panel_max_height', v);
}

async function setNotifySound(v: 'default' | 'manbo') {
  notifySound = v;
  await saveSetting('notify_sound', v);
}

async function setWaitingSound(v: boolean) {
  waitingSound = v;
  await saveSetting('waiting_sound', v);
}

async function setAutoCloseCompletion(v: boolean) {
  autoCloseCompletion = v;
  await saveSetting('auto_close_completion', v);
}

async function setAutoExpandOnTask(v: boolean) {
  autoExpandOnTask = v;
  await saveSetting('auto_expand_on_task', v);
}

async function setHoverDelay(v: number) {
  hoverDelay = v;
  await saveSetting('hover_delay', v);
}

async function setPetSfxEnabled(v: boolean) {
  petSfxEnabled = v;
  await saveSetting('pet_sfx_enabled', v);
}

async function setPetIdleIntervalMin(v: number) {
  petIdleIntervalMin = v;
  await saveSetting('pet_idle_interval_min', v);
}

async function setMiniPetId(v: string) {
  miniPetId = v;
  await saveSetting('mini_pet_id', v);
}

async function setPetQueue(v: string[]) {
  petQueue = v;
  await saveSetting('pet_queue', v);
}

async function setOcConnections(v: OcConnection[]) {
  ocConnections = v;
  await saveSetting('oc_connections', v);
}

export const settingsStore = {
  get appMode() { return appMode; },
  get soundEnabled() { return soundEnabled; },
  get codexSoundEnabled() { return codexSoundEnabled; },
  get cursorSoundEnabled() { return cursorSoundEnabled; },
  get mascotScale() { return mascotScale; },
  get largeMascotScale() { return largeMascotScale; },
  get viewMode() { return viewMode; },
  get enableClaudeCode() { return enableClaudeCode; },
  get enableCodex() { return enableCodex; },
  get enableCursor() { return enableCursor; },
  get language() { return language; },
  get strollEnabled() { return strollEnabled; },
  get panelMaxHeight() { return panelMaxHeight; },
  get notifySound() { return notifySound; },
  get waitingSound() { return waitingSound; },
  get autoCloseCompletion() { return autoCloseCompletion; },
  get autoExpandOnTask() { return autoExpandOnTask; },
  get hoverDelay() { return hoverDelay; },
  get petSfxEnabled() { return petSfxEnabled; },
  get petIdleIntervalMin() { return petIdleIntervalMin; },
  get miniPetId() { return miniPetId; },
  get petQueue() { return petQueue; },
  get ocConnections() { return ocConnections; },
  loadSettings,
  setAppMode,
  setSoundEnabled,
  setCodexSoundEnabled,
  setCursorSoundEnabled,
  setMascotScale,
  setLargeMascotScale,
  setViewMode,
  setEnableClaudeCode,
  setEnableCodex,
  setEnableCursor,
  setLanguage,
  setStrollEnabled,
  setPanelMaxHeight,
  setNotifySound,
  setWaitingSound,
  setAutoCloseCompletion,
  setAutoExpandOnTask,
  setHoverDelay,
  setPetSfxEnabled,
  setPetIdleIntervalMin,
  setMiniPetId,
  setPetQueue,
  setOcConnections,
  getStore,
};
