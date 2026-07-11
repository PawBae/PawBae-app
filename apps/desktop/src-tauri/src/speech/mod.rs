use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;

use serde::Serialize;
use tauri::Emitter;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos::{do_start_recording, do_stop_recording, request_authorization, RecordingState};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows::{do_start_recording, do_stop_recording, request_authorization, RecordingState};

static VOICE_ACTIVE: AtomicBool = AtomicBool::new(false);
/// User-facing master switch (Settings → Privacy). When off, the shortcut starts no
/// recording at all, so the mic is never opened. Defaults OFF (fail-closed): the frontend
/// syncs the persisted setting right after launch, so a user who disabled voice can't have
/// the mic opened in the brief window before that sync lands.
static VOICE_ENABLED: AtomicBool = AtomicBool::new(false);
static SPEECH_TX: OnceLock<Mutex<mpsc::Sender<SpeechCommand>>> = OnceLock::new();

pub(crate) const MAX_RECORDING_SECS: u64 = 30;
pub(crate) const SILENCE_TIMEOUT_SECS: u64 = 8;

/// Whether recording should auto-stop. All times in milliseconds: `elapsed_ms` since the
/// recording started, `last_result_ms` the elapsed value at the most recent transcript
/// (0 = nothing recognized yet, so silence is the whole elapsed time). Computing in ms (not
/// truncated seconds) avoids stopping up to ~1s early.
fn should_autostop(elapsed_ms: u64, last_result_ms: u64, max_secs: u64, silence_secs: u64) -> bool {
    let silence_ms = if last_result_ms == 0 {
        elapsed_ms
    } else {
        elapsed_ms.saturating_sub(last_result_ms)
    };
    elapsed_ms >= max_secs * 1000 || silence_ms >= silence_secs * 1000
}

pub(crate) enum SpeechCommand {
    Start,
    Stop,
}

#[derive(Clone, Serialize)]
pub(crate) struct VoiceStatusPayload {
    pub recording: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
pub(crate) struct VoiceTranscriptPayload {
    pub text: String,
    pub is_final: bool,
}

pub fn is_recording() -> bool {
    VOICE_ACTIVE.load(Ordering::SeqCst)
}

/// Whether flipping the master switch should stop an in-flight recording: only when voice is
/// being turned OFF while a recording is active. Pure so the privacy-critical decision is
/// unit-tested without the native recorder.
fn should_stop_on_disable(enabled: bool, active: bool) -> bool {
    !enabled && active
}

/// Whether a queued Start should actually open the mic when the speech thread reaches it:
/// only if voice is still enabled and nothing is already recording. Re-checked on the thread
/// to close the race where voice is disabled after a Start was queued but before VOICE_ACTIVE
/// flipped true (so set_voice_enabled(false) saw active=false and sent no Stop).
fn should_accept_start(enabled: bool, active: bool) -> bool {
    enabled && !active
}

pub fn set_voice_enabled(enabled: bool) {
    VOICE_ENABLED.store(enabled, Ordering::SeqCst);
    if enabled {
        request_authorization();
    } else if should_stop_on_disable(enabled, VOICE_ACTIVE.load(Ordering::SeqCst)) {
        log::info!("[voice] disabled mid-recording — stopping to release the mic");
        let _ = stop_recording();
    }
}

pub fn start_recording() -> Result<(), String> {
    if !VOICE_ENABLED.load(Ordering::SeqCst) {
        log::info!("[voice] start ignored — voice interaction disabled in settings");
        return Ok(());
    }
    if VOICE_ACTIVE.load(Ordering::SeqCst) {
        return Ok(());
    }
    request_authorization();
    let tx = SPEECH_TX.get().ok_or("Speech thread not initialized")?;
    crate::state::lock_or_recover(tx)
        .send(SpeechCommand::Start)
        .map_err(|e| e.to_string())
}

pub fn stop_recording() -> Result<(), String> {
    if !VOICE_ACTIVE.load(Ordering::SeqCst) {
        return Ok(());
    }
    let tx = SPEECH_TX.get().ok_or("Speech thread not initialized")?;
    crate::state::lock_or_recover(tx)
        .send(SpeechCommand::Stop)
        .map_err(|e| e.to_string())
}

static VOICE_LOCALE: OnceLock<Mutex<String>> = OnceLock::new();

pub fn set_voice_locale(locale: String) {
    let cell = VOICE_LOCALE.get_or_init(|| Mutex::new(String::new()));
    if let Ok(mut g) = cell.lock() {
        *g = locale;
    }
}

pub(crate) fn recognizer_locales() -> Vec<String> {
    let pinned = VOICE_LOCALE
        .get()
        .and_then(|m| m.lock().ok())
        .map(|g| g.trim().to_string())
        .unwrap_or_default();
    if pinned.is_empty() || pinned.eq_ignore_ascii_case("auto") {
        vec!["zh-CN".to_string()]
    } else {
        vec![pinned]
    }
}

/// Final/partial/fallback state machine for the recognizer result handler.
pub(crate) struct Selection {
    pub pending_finals: usize,
    pub best_text: String,
    pub best_conf: f64,
    pub have_best: bool,
    pub emitted: bool,
    pub last_partial_len: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Emit {
    None,
    Partial(String),
    Final(String),
}

impl Selection {
    pub(crate) fn on_partial(&mut self, text: String) -> Emit {
        if !self.emitted && text.len() > self.last_partial_len {
            self.last_partial_len = text.len();
            Emit::Partial(text)
        } else {
            Emit::None
        }
    }

    pub(crate) fn on_done(&mut self, result: Option<(String, f64)>) -> Emit {
        if let Some((text, conf)) = result {
            if !text.trim().is_empty() && (!self.have_best || conf > self.best_conf) {
                self.best_text = text;
                self.best_conf = conf;
                self.have_best = true;
            }
        }
        self.pending_finals = self.pending_finals.saturating_sub(1);
        if self.pending_finals == 0 && !self.emitted && self.have_best {
            self.emitted = true;
            return Emit::Final(self.best_text.clone());
        }
        Emit::None
    }

    pub(crate) fn force_emit(&mut self) -> Emit {
        if !self.emitted && self.have_best {
            self.emitted = true;
            return Emit::Final(self.best_text.clone());
        }
        Emit::None
    }
}

/// Monotonic id of the current recording.
pub(crate) static RECORDING_GEN: AtomicU64 = AtomicU64::new(0);

pub(crate) fn emit_transcript_if_current(
    app: &tauri::AppHandle,
    generation: u64,
    text: String,
    is_final: bool,
) {
    if RECORDING_GEN.load(Ordering::SeqCst) == generation {
        let _ = app.emit(
            "voice-transcript",
            VoiceTranscriptPayload { text, is_final },
        );
    }
}

pub(crate) static LAST_RESULT_TICK: AtomicU64 = AtomicU64::new(0);

pub fn init_speech_thread(app: tauri::AppHandle) {
    let (tx, rx) = mpsc::channel::<SpeechCommand>();
    let _ = SPEECH_TX.set(Mutex::new(tx));

    std::thread::Builder::new()
        .name("speech".into())
        .spawn(move || {
            speech_thread_main(app, rx);
        })
        .expect("Failed to spawn speech thread");
}

fn speech_thread_main(app: tauri::AppHandle, rx: mpsc::Receiver<SpeechCommand>) {
    let mut recording: Option<RecordingState> = None;

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(SpeechCommand::Start) => {
                log::info!("[voice] received Start command");
                let enabled = VOICE_ENABLED.load(Ordering::SeqCst);
                if !should_accept_start(enabled, recording.is_some()) {
                    if !enabled {
                        log::info!("[voice] Start dropped — voice disabled before it ran");
                        let _ = app.emit(
                            "voice-status",
                            VoiceStatusPayload {
                                recording: false,
                                error: None,
                            },
                        );
                    }
                    continue;
                }
                match do_start_recording(&app) {
                    Ok(state) => {
                        if !VOICE_ENABLED.load(Ordering::SeqCst) {
                            log::info!(
                                "[voice] recording initialized but voice was disabled; stopping immediately"
                            );
                            do_stop_recording(state);
                            VOICE_ACTIVE.store(false, Ordering::SeqCst);
                            let _ = app.emit(
                                "voice-status",
                                VoiceStatusPayload {
                                    recording: false,
                                    error: None,
                                },
                            );
                            continue;
                        }
                        log::info!("[voice] recording started");
                        VOICE_ACTIVE.store(true, Ordering::SeqCst);
                        let _ = app.emit(
                            "voice-status",
                            VoiceStatusPayload {
                                recording: true,
                                error: None,
                            },
                        );
                        recording = Some(state);
                    }
                    Err(e) => {
                        log::warn!("[voice] start failed: {}", e);
                        let _ = app.emit(
                            "voice-status",
                            VoiceStatusPayload {
                                recording: false,
                                error: Some(e),
                            },
                        );
                    }
                }
            }
            Ok(SpeechCommand::Stop) => {
                if let Some(state) = recording.take() {
                    log::info!("[voice] stopping recording");
                    do_stop_recording(state);
                    VOICE_ACTIVE.store(false, Ordering::SeqCst);
                    let _ = app.emit(
                        "voice-status",
                        VoiceStatusPayload {
                            recording: false,
                            error: None,
                        },
                    );
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        // Auto-stop checks
        if let Some(ref state) = recording {
            let elapsed_ms = state.start_time().elapsed().as_millis() as u64;
            let last_result_ms = LAST_RESULT_TICK.load(Ordering::SeqCst);
            let should_stop = should_autostop(
                elapsed_ms,
                last_result_ms,
                MAX_RECORDING_SECS,
                SILENCE_TIMEOUT_SECS,
            );

            if should_stop {
                log::info!(
                    "[voice] auto-stop: elapsed={}s last_result={}ms",
                    elapsed_ms / 1000,
                    last_result_ms
                );
                if let Some(state) = recording.take() {
                    do_stop_recording(state);
                    VOICE_ACTIVE.store(false, Ordering::SeqCst);
                    let _ = app.emit(
                        "voice-status",
                        VoiceStatusPayload {
                            recording: false,
                            error: None,
                        },
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{should_accept_start, should_autostop, should_stop_on_disable, Emit, Selection};

    fn fresh(pending: usize) -> Selection {
        Selection {
            pending_finals: pending,
            best_text: String::new(),
            best_conf: 0.0,
            have_best: false,
            emitted: false,
            last_partial_len: 0,
        }
    }

    #[test]
    fn picks_highest_confidence_final_when_all_done() {
        let mut s = fresh(2);
        assert_eq!(s.on_done(Some(("摸摸头".into(), 0.9))), Emit::None);
        assert_eq!(
            s.on_done(Some(("garbage".into(), 0.3))),
            Emit::Final("摸摸头".into())
        );
    }

    #[test]
    fn error_counts_as_done_so_match_emits_immediately() {
        let mut s = fresh(2);
        assert_eq!(s.on_done(Some(("hello".into(), 0.8))), Emit::None);
        assert_eq!(s.on_done(None), Emit::Final("hello".into()));
    }

    #[test]
    fn single_recognizer_emits_its_own_final() {
        let mut s = fresh(1);
        assert_eq!(
            s.on_done(Some(("你好".into(), 0.95))),
            Emit::Final("你好".into())
        );
    }

    #[test]
    fn stops_only_when_disabled_mid_recording() {
        assert!(should_stop_on_disable(false, true));
        assert!(!should_stop_on_disable(false, false));
        assert!(!should_stop_on_disable(true, true));
        assert!(!should_stop_on_disable(true, false));
    }

    #[test]
    fn accepts_start_only_when_enabled_and_idle() {
        assert!(!should_accept_start(false, false));
        assert!(!should_accept_start(false, true));
        assert!(should_accept_start(true, false));
        assert!(!should_accept_start(true, true));
    }

    #[test]
    fn empty_final_never_becomes_the_winner() {
        let mut s = fresh(2);
        assert_eq!(s.on_done(Some(("   ".into(), 0.9))), Emit::None);
        assert_eq!(
            s.on_done(Some(("hi".into(), 0.4))),
            Emit::Final("hi".into())
        );
    }

    #[test]
    fn emits_final_only_once() {
        let mut s = fresh(1);
        assert_eq!(
            s.on_done(Some(("hi".into(), 0.5))),
            Emit::Final("hi".into())
        );
        assert_eq!(s.on_done(None), Emit::None);
        assert_eq!(s.force_emit(), Emit::None);
    }

    #[test]
    fn force_emit_is_the_fallback_for_a_stuck_recognizer() {
        let mut s = fresh(2);
        s.on_done(Some(("hi".into(), 0.5)));
        assert_eq!(s.force_emit(), Emit::Final("hi".into()));
        assert_eq!(s.force_emit(), Emit::None);
    }

    #[test]
    fn partials_echo_the_longest_so_far() {
        let mut s = fresh(2);
        assert_eq!(s.on_partial("ab".into()), Emit::Partial("ab".into()));
        assert_eq!(s.on_partial("a".into()), Emit::None);
        assert_eq!(s.on_partial("abcd".into()), Emit::Partial("abcd".into()));
        s.emitted = true;
        assert_eq!(s.on_partial("abcdef".into()), Emit::None);
    }

    #[test]
    fn autostop_on_max_duration() {
        assert!(should_autostop(30_000, 5_000, 30, 8));
        assert!(!should_autostop(29_000, 28_000, 30, 8));
    }

    #[test]
    fn autostop_on_silence() {
        assert!(should_autostop(8_000, 0, 30, 8));
        assert!(!should_autostop(7_000, 0, 30, 8));
        assert!(should_autostop(20_000, 12_000, 30, 8));
        assert!(!should_autostop(19_000, 12_000, 30, 8));
    }

    #[test]
    fn autostop_uses_ms_precision_no_off_by_one() {
        assert!(!should_autostop(8_000, 500, 30, 8));
    }

    #[test]
    fn recognizer_locales_default_is_zh_cn() {
        let locales = super::recognizer_locales();
        assert_eq!(locales, vec!["zh-CN".to_string()]);
    }
}
