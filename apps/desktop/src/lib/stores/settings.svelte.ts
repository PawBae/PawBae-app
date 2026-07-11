import { load } from '@tauri-apps/plugin-store';
import type { AppMode, OcConnection } from '../types';
import { sanitizeNickname, sanitizeNicknames } from '../utils/pet-name';
import { migrateMiniPetId } from '../utils/skins';
import { type StageBg, sanitizeStageBg } from '../utils/stage-bridge';

class SettingsStore {
  appMode = $state<AppMode | null>(null);
  soundEnabled = $state(true);
  codexSoundEnabled = $state(true);
  cursorSoundEnabled = $state(false);
  mascotScale = $state(1);
  largeMascotScale = $state(5);
  viewMode = $state<'efficiency' | 'island'>('efficiency');
  enableClaudeCode = $state(true);
  enableCodex = $state(true);
  enableCursor = $state(true);
  language = $state('en');
  strollEnabled = $state(true);
  panelMaxHeight = $state(400);
  notifySound = $state<'default' | 'manbo'>('default');
  waitingSound = $state(true);
  autoCloseCompletion = $state(false);
  autoExpandOnTask = $state(true);
  hoverDelay = $state(0.3);
  petSfxEnabled = $state(true);
  petIdleIntervalMin = $state(2);
  miniPetId = $state<string>('yoonie');
  // User nicknames per pet id (角色 IP: Yoonie stays the official name; this is the
  // user's address for THEIR pet). Absent key = official name. utils/pet-name.ts
  // owns the sanitize/fallback rules.
  petNicknames = $state.raw<Record<string, string>>({});
  petQueue = $state.raw<string[]>([]);
  skippedVersion = $state('');
  inputTrackingEnabled = $state(true);
  // Anonymous usage telemetry is OPT-IN: default off, and utils/telemetry.ts
  // gates every event on this flag.
  telemetryEnabled = $state(false);
  voiceEnabled = $state(false);
  musicReactionEnabled = $state(true);
  // OBS 直播舞台: the store only persists the choice — window open/close and the
  // snapshot feed react to these in Main/MascotView (stores never invoke here).
  streamStageEnabled = $state(false);
  streamStageBg = $state<StageBg>('green');
  ocConnections = $state.raw<OcConnection[]>([{ id: 'local', type: 'local' }]);
  // 平台 opt-in（B 线 W3，全部默认关）：主开关 = 「连接你的 agent」——
  // 关 = 心跳/投影/事件一律不出本机；分项 = 逐类事件上传（主开关关时无效）。
  platformConnectEnabled = $state(false);
  uploadRewardsEnabled = $state(false);
  uploadEggsEnabled = $state(false);
  uploadSouvenirsEnabled = $state(false);
  uploadStreaksEnabled = $state(false);

  private storeInstance: Awaited<ReturnType<typeof load>> | null = null;

  async getStore() {
    if (!this.storeInstance) {
      this.storeInstance = await load('settings.json', { defaults: {}, autoSave: true });
    }
    return this.storeInstance;
  }

  async loadSettings() {
    const store = await this.getStore();

    this.appMode = ((await store.get('app_mode')) as AppMode) || null;
    this.soundEnabled = ((await store.get('sound_enabled')) as boolean) ?? true;
    this.codexSoundEnabled = ((await store.get('codex_sound_enabled')) as boolean) ?? true;
    this.cursorSoundEnabled = ((await store.get('cursor_sound_enabled')) as boolean) ?? false;
    this.mascotScale = ((await store.get('mascot_scale')) as number) ?? 1;
    this.largeMascotScale = ((await store.get('large_mascot_scale')) as number) ?? 5;
    this.viewMode = ((await store.get('view_mode')) as 'efficiency' | 'island') || 'efficiency';
    this.enableClaudeCode = ((await store.get('enable_claude_code')) as boolean) ?? true;
    this.enableCodex = ((await store.get('enable_codex')) as boolean) ?? true;
    this.enableCursor = ((await store.get('enable_cursor')) as boolean) ?? true;
    this.language = ((await store.get('pawbae-lang')) as string) || 'en';
    this.strollEnabled = ((await store.get('stroll_mode_enabled')) as boolean) ?? true;
    this.panelMaxHeight = ((await store.get('panel_max_height')) as number) ?? 400;
    this.notifySound = ((await store.get('notify_sound')) as 'default' | 'manbo') || 'default';
    this.waitingSound = ((await store.get('waiting_sound')) as boolean) ?? true;
    this.autoCloseCompletion = ((await store.get('auto_close_completion')) as boolean) ?? false;
    this.autoExpandOnTask = ((await store.get('auto_expand_on_task')) as boolean) ?? true;
    this.hoverDelay = ((await store.get('hover_delay')) as number) ?? 0.3;
    this.petSfxEnabled = ((await store.get('pet_sfx_enabled')) as boolean) ?? true;
    this.petIdleIntervalMin = ((await store.get('pet_idle_interval_min')) as number) ?? 2;
    // Copyright cleanup migration: a persisted id of a removed builtin skin falls
    // back to the default pet (skinsStore.resolve also guards at render time, but
    // rewriting here keeps nickname lookups and the gallery active-tile in sync).
    const storedPetId = (await store.get('mini_pet_id')) as string | undefined;
    this.miniPetId = migrateMiniPetId(storedPetId);
    if (storedPetId && storedPetId !== this.miniPetId) {
      await store.set('mini_pet_id', this.miniPetId);
    }
    this.petNicknames = sanitizeNicknames(await store.get('pet_nicknames'));
    this.petQueue = ((await store.get('pet_queue')) as string[]) || [];
    this.skippedVersion = ((await store.get('skipped_version')) as string) || '';
    this.inputTrackingEnabled = ((await store.get('input_tracking_enabled')) as boolean) ?? true;
    this.telemetryEnabled = ((await store.get('telemetry_enabled')) as boolean) ?? false;
    this.voiceEnabled = ((await store.get('voice_enabled')) as boolean) ?? false;
    this.musicReactionEnabled = ((await store.get('music_reaction_enabled')) as boolean) ?? true;
    this.streamStageEnabled = ((await store.get('stream_stage_enabled')) as boolean) ?? false;
    this.streamStageBg = sanitizeStageBg(await store.get('stream_stage_bg'));
    this.ocConnections = ((await store.get('oc_connections')) as OcConnection[]) || [
      { id: 'local', type: 'local' },
    ];
    this.platformConnectEnabled =
      ((await store.get('platform_connect_enabled')) as boolean) ?? false;
    this.uploadRewardsEnabled = ((await store.get('upload_rewards_enabled')) as boolean) ?? false;
    this.uploadEggsEnabled = ((await store.get('upload_eggs_enabled')) as boolean) ?? false;
    this.uploadSouvenirsEnabled =
      ((await store.get('upload_souvenirs_enabled')) as boolean) ?? false;
    this.uploadStreaksEnabled = ((await store.get('upload_streaks_enabled')) as boolean) ?? false;
  }

  private async saveSetting(key: string, value: unknown) {
    const store = await this.getStore();
    await store.set(key, value);
    await store.save();
  }

  async setAppMode(mode: AppMode) {
    this.appMode = mode;
    await this.saveSetting('app_mode', mode);
  }

  async setSoundEnabled(v: boolean) {
    this.soundEnabled = v;
    await this.saveSetting('sound_enabled', v);
  }

  async setCodexSoundEnabled(v: boolean) {
    this.codexSoundEnabled = v;
    await this.saveSetting('codex_sound_enabled', v);
  }

  async setCursorSoundEnabled(v: boolean) {
    this.cursorSoundEnabled = v;
    await this.saveSetting('cursor_sound_enabled', v);
  }

  async setMascotScale(v: number) {
    this.mascotScale = v;
    await this.saveSetting('mascot_scale', v);
  }

  async setLargeMascotScale(v: number) {
    this.largeMascotScale = v;
    await this.saveSetting('large_mascot_scale', v);
  }

  async setViewMode(v: 'efficiency' | 'island') {
    this.viewMode = v;
    await this.saveSetting('view_mode', v);
  }

  async setEnableClaudeCode(v: boolean) {
    this.enableClaudeCode = v;
    await this.saveSetting('enable_claude_code', v);
  }

  async setEnableCodex(v: boolean) {
    this.enableCodex = v;
    await this.saveSetting('enable_codex', v);
  }

  async setEnableCursor(v: boolean) {
    this.enableCursor = v;
    await this.saveSetting('enable_cursor', v);
  }

  async setLanguage(v: string) {
    this.language = v;
    await this.saveSetting('pawbae-lang', v);
  }

  async setStrollEnabled(v: boolean) {
    this.strollEnabled = v;
    await this.saveSetting('stroll_mode_enabled', v);
  }

  async setPanelMaxHeight(v: number) {
    this.panelMaxHeight = v;
    await this.saveSetting('panel_max_height', v);
  }

  async setNotifySound(v: 'default' | 'manbo') {
    this.notifySound = v;
    await this.saveSetting('notify_sound', v);
  }

  async setWaitingSound(v: boolean) {
    this.waitingSound = v;
    await this.saveSetting('waiting_sound', v);
  }

  async setAutoCloseCompletion(v: boolean) {
    this.autoCloseCompletion = v;
    await this.saveSetting('auto_close_completion', v);
  }

  async setAutoExpandOnTask(v: boolean) {
    this.autoExpandOnTask = v;
    await this.saveSetting('auto_expand_on_task', v);
  }

  async setHoverDelay(v: number) {
    this.hoverDelay = v;
    await this.saveSetting('hover_delay', v);
  }

  async setPetSfxEnabled(v: boolean) {
    this.petSfxEnabled = v;
    await this.saveSetting('pet_sfx_enabled', v);
  }

  async setPetIdleIntervalMin(v: number) {
    this.petIdleIntervalMin = v;
    await this.saveSetting('pet_idle_interval_min', v);
  }

  async setMiniPetId(v: string) {
    this.miniPetId = v;
    await this.saveSetting('mini_pet_id', v);
  }

  /** Empty (post-sanitize) removes the nickname — the pet answers to her official
   *  name again. Returns whether anything changed; the pet_renamed telemetry call
   *  lives at the UI layer (telemetry.ts imports this store — no cycles). */
  async setPetNickname(petId: string, raw: string): Promise<boolean> {
    const nick = sanitizeNickname(raw);
    const prev = this.petNicknames[petId] ?? '';
    if (nick === prev) return false;
    const next: Record<string, string> = Object.create(null);
    Object.assign(next, this.petNicknames);
    if (nick) next[petId] = nick;
    else delete next[petId];
    this.petNicknames = next;
    await this.saveSetting('pet_nicknames', { ...next });
    return true;
  }

  async setPetQueue(v: string[]) {
    this.petQueue = v;
    await this.saveSetting('pet_queue', v);
  }

  async setSkippedVersion(v: string) {
    this.skippedVersion = v;
    await this.saveSetting('skipped_version', v);
  }

  async setInputTrackingEnabled(v: boolean) {
    this.inputTrackingEnabled = v;
    await this.saveSetting('input_tracking_enabled', v);
  }

  async setTelemetryEnabled(v: boolean) {
    this.telemetryEnabled = v;
    await this.saveSetting('telemetry_enabled', v);
  }

  async setVoiceEnabled(v: boolean) {
    this.voiceEnabled = v;
    await this.saveSetting('voice_enabled', v);
  }

  async setMusicReactionEnabled(v: boolean) {
    this.musicReactionEnabled = v;
    await this.saveSetting('music_reaction_enabled', v);
  }

  async setStreamStageEnabled(v: boolean) {
    this.streamStageEnabled = v;
    await this.saveSetting('stream_stage_enabled', v);
  }

  async setStreamStageBg(v: StageBg) {
    this.streamStageBg = v;
    await this.saveSetting('stream_stage_bg', v);
  }

  async setOcConnections(v: OcConnection[]) {
    this.ocConnections = v;
    await this.saveSetting('oc_connections', v);
  }

  async setPlatformConnectEnabled(v: boolean) {
    this.platformConnectEnabled = v;
    await this.saveSetting('platform_connect_enabled', v);
  }

  async setUploadRewardsEnabled(v: boolean) {
    this.uploadRewardsEnabled = v;
    await this.saveSetting('upload_rewards_enabled', v);
  }

  async setUploadEggsEnabled(v: boolean) {
    this.uploadEggsEnabled = v;
    await this.saveSetting('upload_eggs_enabled', v);
  }

  async setUploadSouvenirsEnabled(v: boolean) {
    this.uploadSouvenirsEnabled = v;
    await this.saveSetting('upload_souvenirs_enabled', v);
  }

  async setUploadStreaksEnabled(v: boolean) {
    this.uploadStreaksEnabled = v;
    await this.saveSetting('upload_streaks_enabled', v);
  }
}

export const settingsStore = new SettingsStore();
