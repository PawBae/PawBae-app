<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { _ } from 'svelte-i18n';
  import { agentStore } from '../stores/agents.svelte';
  import { petStore } from '../stores/pet.svelte';
  import { sessionStore } from '../stores/sessions.svelte';
  import { settingsStore } from '../stores/settings.svelte';
  import { skinsStore } from '../stores/skins.svelte';
  import { windowStore } from '../stores/window.svelte';
  import type { AppMode, UpdateModalInfo } from '../types';
  import type { CodexPet } from '../utils/codex-pet';
  import { loadDefaultCodexPet } from '../utils/codex-pet';
  import { tryInvoke } from '../utils/invoke';
  import { track } from '../utils/telemetry';
  import { classifyIntent } from '../utils/voice-intent';
  import MascotView from './MascotView.svelte';
  import Onboarding from './Onboarding.svelte';
  import Panel from './Panel.svelte';
  import SettingsPanel from './settings/SettingsPanel.svelte';
  import UpdateModal from './UpdateModal.svelte';

  let pet = $state<CodexPet | null>(null);

  // The skin workshop switches pets at runtime: re-resolve on a new id or a store
  // refresh (a re-imported skin keeps its id, so the id alone would never re-fire).
  $effect(() => {
    const next = skinsStore.resolve(settingsStore.miniPetId);
    if (next) pet = next;
  });

  // OBS 直播舞台: settings own the choice, this effect owns the window — including
  // the startup restore when loadSettings flips the flag. Closing a window that
  // never opened is a no-op, so the pre-load `false` pass is harmless.
  $effect(() => {
    void tryInvoke(settingsStore.streamStageEnabled ? 'open_stage_window' : 'close_stage_window');
  });
  let showOnboarding = $state(false);

  let voiceRecording = $state(false);
  let voiceText = $state('');
  let voiceError = $state('');
  // Pet's reaction to a recognized final transcript (voice Phase C). `voiceNonce`
  // bumps on every final so MascotView re-plays the emotion even on a repeated intent.
  let voiceReply = $state('');
  let voiceEmotion = $state<string | null>(null);
  let voiceNonce = $state(0);
  let voiceReplyTimer: ReturnType<typeof setTimeout> | null = null;
  let voiceTextTimer: ReturnType<typeof setTimeout> | null = null;
  let voiceErrorTimer: ReturnType<typeof setTimeout> | null = null;
  const VOICE_REPLY_MS = 2600;

  let updateOpen = $state(false);
  let updatePhase = $state<'available' | 'downloading' | 'ready_to_restart'>('available');
  let updateInfo = $state<UpdateModalInfo | null>(null);
  let updateProgress = $state<number | null>(null);
  let updateProgressStage = $state('');
  let updateError = $state('');

  // Startup update prompt: check once shortly after launch, after the user has a
  // mode (never on top of onboarding). The manual flow in Settings → About stays
  // independent and is the fallback when this check fails quietly.
  const UPDATE_PROMPT_DELAY_MS = 8000;
  let updateCheckStarted = false;

  type UpdateProgressPayload = {
    stage: string;
    progress?: number | null;
    downloadedBytes?: number;
    totalBytes?: number | null;
    message?: string;
  };

  function resolveProgressText(stage?: string, fallbackMessage?: string): string {
    if (stage) {
      const key = `updateModal.progress.${stage}`;
      const localized = $_(key);
      if (localized !== key) return localized;
    }
    return fallbackMessage || '';
  }

  // 上次运行留下的崩溃报告：读一次未读计数（Rust 侧推进 marker），若有且用户
  // 开了遥测，发匿名聚合事件 —— 只有计数，无堆栈无路径（隐私红线见 crash.rs）。
  let crashCheckStarted = false;
  async function reportUnseenCrashes() {
    if (crashCheckStarted) return;
    crashCheckStarted = true;
    try {
      const res = (await invoke('take_unseen_crashes')) as {
        total: number;
        kinds: Record<string, number>;
      };
      if (res.total > 0) {
        track('app_crash_detected', {
          total: res.total,
          rust_panic: res.kinds['rust-panic'] ?? 0,
          webview: (res.kinds['webview-error'] ?? 0) + (res.kinds['webview-rejection'] ?? 0),
        });
      }
    } catch {
      // 命令缺失（旧构建）或读盘失败——静默，崩溃上报不能自己成为崩溃源
    }
  }

  async function checkUpdateOnStartup() {
    try {
      const info = (await invoke('check_for_update', { lang: settingsStore.language })) as {
        current: string;
        latest: string;
        hasUpdate: boolean;
        url: string;
        notes: string;
        signature?: string;
      };
      if (!info.hasUpdate || !info.url) return;
      if (info.latest === settingsStore.skippedVersion) return;
      updateInfo = info;
      updatePhase = 'available';
      updateOpen = true;
    } catch (e) {
      // Offline or manifest missing — stay quiet; Settings → About has manual check.
      console.warn('[update] startup check failed:', e);
    }
  }

  async function startUpdate() {
    if (!updateInfo?.url) return;
    updateError = '';
    updatePhase = 'downloading';
    updateProgress = 0;
    updateProgressStage = resolveProgressText('preparing', '');
    try {
      await invoke('run_update', { dmgUrl: updateInfo.url, signature: updateInfo.signature ?? null });
      // The installer helper is spawned and waiting — don't rely solely on the
      // ready_to_restart progress event racing the invoke resolution.
      updatePhase = 'ready_to_restart';
    } catch (e) {
      updatePhase = 'available';
      updateError = String(e);
    }
  }

  function skipThisVersion() {
    if (updateInfo) settingsStore.setSkippedVersion(updateInfo.latest);
    updateOpen = false;
  }

  $effect(() => {
    init();
    void reportUnseenCrashes();

    let disposed = false;
    const cleanups: (() => void)[] = [];

    function addListener<T>(event: string, handler: (e: { payload: T }) => void) {
      listen<T>(event, handler).then((u) => {
        if (disposed) u();
        else cleanups.push(u);
      });
    }

    addListener<{ recording: boolean; error?: string }>('voice-status', (e) => {
      voiceRecording = e.payload.recording;
      voiceError = e.payload.error || '';
      if (!e.payload.recording) {
        if (voiceTextTimer) clearTimeout(voiceTextTimer);
        voiceTextTimer = setTimeout(() => { voiceText = ''; voiceTextTimer = null; }, 2000);
      }
      if (e.payload.error) {
        if (voiceErrorTimer) clearTimeout(voiceErrorTimer);
        voiceErrorTimer = setTimeout(() => { voiceError = ''; voiceErrorTimer = null; }, 6000);
      }
    });

    addListener<{ text: string; is_final: boolean }>('voice-transcript', (e) => {
      // Privacy gate: if voice was turned off (e.g. mid-recording), drop any in-flight /
      // late transcript — no echo, no classification, no pet reply, no affection — and clear
      // any lingering bubble state so the pet stays quiet.
      if (!settingsStore.voiceEnabled) {
        voiceText = '';
        voiceReply = '';
        voiceEmotion = null;
        return;
      }
      // Live partial results keep echoing in the orange "heard" bubble; on the final
      // result we classify it, hand off to the pet's reply, and react.
      if (!e.payload.is_final) {
        voiceText = e.payload.text;
        return;
      }
      const r = classifyIntent(e.payload.text, {
        // She answers to her official name AND the user's nickname.
        petNames: pet ? [settingsStore.petNicknames[pet.id] ?? '', pet.displayName] : [],
      });
      petStore.applyVoiceAffection(r.affectionDelta);
      voiceText = ''; // swap the echo bubble out for the pet's reply
      voiceEmotion = r.emotion;
      voiceReply = $_(r.replyKey);
      voiceNonce += 1;
      if (voiceReplyTimer) clearTimeout(voiceReplyTimer);
      voiceReplyTimer = setTimeout(() => {
        voiceReply = '';
        voiceReplyTimer = null;
      }, VOICE_REPLY_MS);
    });

    // macOS tray "Stroll Mode" item. The frontend owns settings.json, so the
    // toggle is persisted here; MascotView's physics effect reacts to the
    // setting and flips the loop on/off.
    addListener<boolean>('stroll-mode-changed', (e) => {
      settingsStore.setStrollEnabled(e.payload);
    });

    addListener('tray-open-settings', () => {
      openSettings();
    });

    // The stage window is borderless — no close button of ours — but the OS can
    // still kill it (Cmd+W, forced close). Fall the settings toggle back so the
    // UI reflects reality; the lifecycle effect's close on the already-dead
    // window is a no-op.
    addListener('stage-closed', () => {
      if (settingsStore.streamStageEnabled) void settingsStore.setStreamStageEnabled(false);
    });

    addListener<UpdateProgressPayload>('update-progress', (e) => {
      const p = e.payload;
      updateProgress = typeof p.progress === 'number' ? p.progress : null;
      updateProgressStage = resolveProgressText(p.stage, p.message);
      if (p.stage === 'ready_to_restart') updatePhase = 'ready_to_restart';
    });

    // Reward model (P1-C): hydrate persisted coins/ledger, then listen for agent
    // completions and user input. App-wide on purpose — agent stops happen in coding
    // mode while the coin balance shows in pet mode.
    petStore.init().then((dispose) => {
      if (disposed) dispose();
      else cleanups.push(dispose);
    });

    return () => {
      disposed = true;
      for (const fn of cleanups) fn();
      agentStore.stopPolling();
      sessionStore.stopPolling();
    };
  });

  async function init() {
    try {
      await settingsStore.loadSettings();
    } catch (e) {
      console.warn('[init] settings load failed:', e);
    }

    // Retention heartbeat (installs / DAU / D1-D7-D30). No-op unless the user
    // opted in; first-run users fire theirs from handleModeSelect post-consent.
    if (settingsStore.appMode) track('app_started', { mode: settingsStore.appMode });

    try {
      await skinsStore.ensureLoaded();
      pet = skinsStore.resolve(settingsStore.miniPetId);
    } catch (e) {
      console.error('[init] pet loading failed:', e);
      pet = await loadDefaultCodexPet().catch(() => null);
    }

    // Dex migration (孵蛋与物种图鉴): an install already using a builtin neighbor when
    // the gate shipped keeps it — never confiscate the current pet. petStore.init() is
    // idempotent (shared promise); the reward $effect owns its dispose.
    petStore.init().then(() => petStore.noteCurrentSkinMet(settingsStore.miniPetId));

    if (!settingsStore.appMode) {
      showOnboarding = true;
      windowStore.setSettingsOpen(true);
      try {
        await invoke('set_mini_size', { restore: false });
      } catch (e) {
        console.warn('[onboarding] set_mini_size failed:', e);
      }
    }
  }

  function startModePolling() {
    if (settingsStore.appMode === 'coding') {
      agentStore.startPolling();
      sessionStore.startPolling();
    } else {
      agentStore.stopPolling();
      sessionStore.stopPolling();
    }
  }

  async function handleModeSelect(mode: AppMode, shareTelemetry: boolean) {
    showOnboarding = false;
    windowStore.setSettingsOpen(false);
    if (shareTelemetry) {
      await settingsStore.setTelemetryEnabled(true);
      track('app_started', { mode });
    }
    await settingsStore.setAppMode(mode);
    try {
      await invoke('set_mini_size', { restore: true, mascotScale: settingsStore.mascotScale });
    } catch (e) {
      console.warn('[onboarding] restore size failed:', e);
    }
  }

  $effect(() => {
    if (settingsStore.appMode) startModePolling();
  });

  // Recognition language: 'auto' resolves to the default single recognizer (Chinese) in the
  // Rust `recognizer_locales`. macOS can't reliably run two recognizers at once, so there is
  // no concurrent bilingual auto-detect (no-op off macOS).
  $effect(() => {
    tryInvoke('voice_set_locale', { locale: 'auto' });
  });

  // Master voice on/off (Settings → Privacy): keep the backend gate in sync so a disabled
  // setting means the shortcut opens no microphone.
  $effect(() => {
    tryInvoke('voice_set_enabled', { enabled: settingsStore.voiceEnabled });
  });

  $effect(() => {
    if (!settingsStore.appMode || showOnboarding || updateCheckStarted) return;
    updateCheckStarted = true;
    const timer = setTimeout(checkUpdateOnStartup, UPDATE_PROMPT_DELAY_MS);
    return () => clearTimeout(timer);
  });

  async function openSettings() {
    if (windowStore.settingsOpen) return;
    windowStore.setSettingsOpen(true);
    try {
      await invoke('set_mini_size', { restore: false });
    } catch (e) {
      console.warn('[settings] set_mini_size failed:', e);
    }
  }

  async function closeSettings() {
    if (!windowStore.settingsOpen) return;
    windowStore.setSettingsOpen(false);
    try {
      await invoke('set_mini_size', { restore: true, mascotScale: settingsStore.mascotScale });
    } catch (e) {
      console.warn('[settings] restore size failed:', e);
    }
  }
</script>

<div class="root" data-tauri-drag-region={windowStore.settingsOpen ? undefined : ''}>
  <MascotView
    {pet}
    {voiceRecording}
    {voiceText}
    {voiceError}
    {voiceReply}
    {voiceEmotion}
    {voiceNonce}
  />
  <Panel />

  <Onboarding open={showOnboarding} onSelect={handleModeSelect} />

  <SettingsPanel open={windowStore.settingsOpen} onClose={closeSettings} />

  <UpdateModal
    open={updateOpen}
    phase={updatePhase}
    info={updateInfo}
    progress={updateProgress}
    progressStage={updateProgressStage}
    errorMsg={updateError}
    onLater={() => { updateOpen = false; }}
    onSkipVersion={skipThisVersion}
    onUpdateNow={startUpdate}
    onRestartNow={() => { tryInvoke('exit_app'); }}
  />
</div>

<style>
  .root {
    width: 100vw;
    height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    background: transparent;
    overflow: hidden;
  }
</style>
