use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject};
use serde::Serialize;
use tauri::Emitter;

#[link(name = "Speech", kind = "framework")]
unsafe extern "C" {}
#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {}

static VOICE_ACTIVE: AtomicBool = AtomicBool::new(false);
/// User-facing master switch (Settings → Privacy). When off, the shortcut starts no
/// recording at all, so the mic is never opened. Defaults OFF (fail-closed): the frontend
/// syncs the persisted setting right after launch, so a user who disabled voice can't have
/// the mic opened in the brief window before that sync lands.
static VOICE_ENABLED: AtomicBool = AtomicBool::new(false);
static SPEECH_TX: OnceLock<Mutex<mpsc::Sender<SpeechCommand>>> = OnceLock::new();

const MAX_RECORDING_SECS: u64 = 30;
const SILENCE_TIMEOUT_SECS: u64 = 8;

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

enum SpeechCommand {
    Start,
    Stop,
}

#[derive(Clone, Serialize)]
struct VoiceStatusPayload {
    recording: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Clone, Serialize)]
struct VoiceTranscriptPayload {
    text: String,
    is_final: bool,
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
        // Explicit opt-in: request authorization now (no-op if already resolved) so the
        // system prompt is tied to the user enabling voice, not to app launch.
        request_authorization();
    } else if should_stop_on_disable(enabled, VOICE_ACTIVE.load(Ordering::SeqCst)) {
        // Privacy: turning voice off mid-recording must close the mic immediately, not wait
        // for the manual/auto stop. stop_recording() sends Stop; the speech thread tears the
        // engine down and emits voice-status { recording: false } so the UI red dot clears.
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
    // First real recording (gate passed): make sure authorization is requested even if the
    // setting was enabled in a prior build where the prompt never fired. No-op once resolved.
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

/// Ask macOS for Speech Recognition permission. Called when the user opts in (enables voice)
/// or on their first recording — NOT at app launch, so a fresh opted-out user sees no prompt.
/// An earlier version only *read* `authorizationStatus`, so a notDetermined status never
/// advanced and the recognizer silently returned "No speech detected"; this triggers the
/// system prompt (requires `NSSpeechRecognitionUsageDescription` in the bundle Info.plist).
/// Calling it when already authorized/denied is a cheap no-op (no repeat prompt).
pub fn request_authorization() {
    unsafe {
        let Some(speech_cls) = AnyClass::get(c"SFSpeechRecognizer") else {
            log::warn!("[voice] SFSpeechRecognizer unavailable; cannot request authorization");
            return;
        };
        let current: i64 = msg_send![speech_cls, authorizationStatus];
        // Only prompt when the user hasn't decided yet. Once authorized/denied, re-calling
        // is a no-op for the user but pointless — skip it. (A permission reset across builds
        // is a code-signing artifact, not this call firing repeatedly.)
        if current != 0 {
            log::info!("[voice] speech authorization already resolved (status={current}), not re-requesting");
            return;
        }
        log::info!("[voice] requesting speech authorization (current status={current})");
        let handler = RcBlock::new(move |new_status: i64| {
            log::info!(
                "[voice] speech authorization result: status={new_status} (0=notDetermined, 1=denied, 2=restricted, 3=authorized)"
            );
        });
        let _: () = msg_send![speech_cls, requestAuthorization: &*handler];
        std::mem::forget(handler);
    }
}

pub fn init_speech_thread(app: tauri::AppHandle) {
    // NOTE: do NOT request Speech Recognition authorization here. Voice is opt-in (default
    // off), so a fresh user must not see the system prompt at launch. Authorization is
    // requested only when the user explicitly enables voice (set_voice_enabled(true)) or on
    // the first real recording (start_recording, after the enabled gate).
    let (tx, rx) = mpsc::channel::<SpeechCommand>();
    let _ = SPEECH_TX.set(Mutex::new(tx));

    std::thread::Builder::new()
        .name("speech".into())
        .spawn(move || {
            speech_thread_main(app, rx);
        })
        .expect("Failed to spawn speech thread");
}

unsafe fn nsstring_to_string(ns: *mut AnyObject) -> String {
    if ns.is_null() {
        return String::new();
    }
    let c_str: *const u8 = unsafe { msg_send![&*ns, UTF8String] };
    if c_str.is_null() {
        return String::new();
    }
    unsafe {
        std::ffi::CStr::from_ptr(c_str as *const _)
            .to_string_lossy()
            .into_owned()
    }
}

unsafe fn make_nsstring(s: &str) -> *mut AnyObject {
    let Some(cls) = AnyClass::get(c"NSString") else {
        return std::ptr::null_mut();
    };
    let Ok(c) = std::ffi::CString::new(s) else {
        return std::ptr::null_mut();
    };
    unsafe { msg_send![cls, stringWithUTF8String: c.as_ptr()] }
}

// Locale the single recognizer is built with (e.g. "zh-CN"). The frontend sets it via the
// `voice_set_locale` command; "auto"/empty resolves to the default in `recognizer_locales`.
static VOICE_LOCALE: OnceLock<Mutex<String>> = OnceLock::new();

pub fn set_voice_locale(locale: String) {
    let cell = VOICE_LOCALE.get_or_init(|| Mutex::new(String::new()));
    if let Ok(mut g) = cell.lock() {
        *g = locale;
    }
}

/// Locale the recognizer runs. Chinese-only for now: macOS won't run two SFSpeechRecognizers
/// at once (the second is reliably starved — "No speech detected" — whether on-device or via
/// the server), so bilingual auto-detect isn't possible with a single live engine. A specific
/// identifier via voice_set_locale still pins to that language.
fn recognizer_locales() -> Vec<String> {
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

/// Final/partial/fallback state machine for the recognizer result handler. The app runs a
/// SINGLE recognizer (see `recognizer_locales`); the multi-`pending_finals` support is a
/// historical/defensive remnant and does NOT mean concurrent multi-language recognition is
/// enabled. Partials echo live; the final is emitted exactly once.
struct Selection {
    pending_finals: usize,
    best_text: String,
    best_conf: f64,
    have_best: bool,
    emitted: bool,
    last_partial_len: usize,
}

#[derive(Debug, PartialEq, Eq)]
enum Emit {
    None,
    Partial(String),
    Final(String),
}

impl Selection {
    /// A live partial result. Echo the longest one so far (monotonic) so the displayed text
    /// doesn't jitter shorter between updates.
    fn on_partial(&mut self, text: String) -> Emit {
        if !self.emitted && text.len() > self.last_partial_len {
            self.last_partial_len = text.len();
            Emit::Partial(text)
        } else {
            Emit::None
        }
    }

    /// A recognizer reached a terminal state: `Some((text, conf))` for a final transcript,
    /// or `None` on error (it heard nothing). Decrements the pending count and emits the
    /// highest-confidence final once every recognizer is done. With a single recognizer this
    /// is just "emit its final, or nothing if it errored".
    fn on_done(&mut self, result: Option<(String, f64)>) -> Emit {
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

    /// Safety net: if a recognizer neither finalized nor errored, emit the best we have.
    fn force_emit(&mut self) -> Emit {
        if !self.emitted && self.have_best {
            self.emitted = true;
            return Emit::Final(self.best_text.clone());
        }
        Emit::None
    }
}

struct RecordingState {
    app: tauri::AppHandle,
    engine: *mut AnyObject,
    requests: Vec<*mut AnyObject>,
    tasks: Vec<*mut AnyObject>,
    selection: std::sync::Arc<Mutex<Selection>>,
    start_time: Instant,
    generation: u64,
}

unsafe impl Send for RecordingState {}

/// Monotonic id of the current recording. Bumped on every start so a late async final or
/// the 900ms fallback from a previous session can't emit a stale transcript into a newer
/// recording (push-to-talk released and re-pressed quickly).
static RECORDING_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn emit_transcript_if_current(
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

fn speech_thread_main(app: tauri::AppHandle, rx: mpsc::Receiver<SpeechCommand>) {
    let mut recording: Option<RecordingState> = None;

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(SpeechCommand::Start) => {
                log::info!("[voice] received Start command");
                let enabled = VOICE_ENABLED.load(Ordering::SeqCst);
                // Re-check enabled here (not just at enqueue) so a disable that landed after
                // this Start was queued still keeps the mic shut.
                if !should_accept_start(enabled, recording.is_some()) {
                    if !enabled {
                        log::info!("[voice] Start dropped — voice disabled before it ran");
                        // Keep the UI's recording state consistent; not an error.
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
                        // Final privacy gate: voice may have been disabled WHILE the engine /
                        // recognizer were initializing. At that moment VOICE_ACTIVE was still
                        // false, so set_voice_enabled(false) sent no Stop — close the mic here
                        // before ever exposing it as recording, instead of opening it.
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
            let elapsed_ms = state.start_time.elapsed().as_millis() as u64;
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

static LAST_RESULT_TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

static RECORDING_START_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Average per-segment confidence of a transcription (0..1). Final results carry real
/// confidences. The Selection uses it to break ties when it has multiple candidates; with a
/// single recognizer running today it is simply that recognizer's own confidence.
fn average_confidence(transcription: *mut AnyObject) -> f64 {
    if transcription.is_null() {
        return 0.0;
    }
    unsafe {
        let segments: *mut AnyObject = msg_send![&*transcription, segments];
        if segments.is_null() {
            return 0.0;
        }
        let count: usize = msg_send![&*segments, count];
        if count == 0 {
            return 0.0;
        }
        let mut sum = 0.0f64;
        for i in 0..count {
            let seg: *mut AnyObject = msg_send![&*segments, objectAtIndex: i];
            let c: f32 = msg_send![&*seg, confidence];
            sum += c as f64;
        }
        sum / count as f64
    }
}

fn do_start_recording(app: &tauri::AppHandle) -> Result<RecordingState, String> {
    unsafe {
        let speech_cls =
            AnyClass::get(c"SFSpeechRecognizer").ok_or("SFSpeechRecognizer not available")?;

        let auth_status: i64 = msg_send![speech_cls, authorizationStatus];
        log::info!("[voice] speech auth status = {} (0=notDetermined, 1=denied, 2=restricted, 3=authorized)", auth_status);
        if auth_status == 1 || auth_status == 2 {
            return Err("Speech recognition denied. Enable in System Settings → Privacy & Security → Speech Recognition.".into());
        }
        // For notDetermined (0), proceed anyway — macOS may still allow
        // recognition and will auto-prompt in signed production builds.

        let locales = recognizer_locales();
        log::info!("[voice] creating recognizers for locales {locales:?}");
        let locale_cls = AnyClass::get(c"NSLocale").ok_or("NSLocale not available")?;
        let req_cls = AnyClass::get(c"SFSpeechAudioBufferRecognitionRequest")
            .ok_or("SFSpeechAudioBufferRecognitionRequest not available")?;

        // One recognizer + request per locale. recognizer_locales() returns a single locale
        // (zh-CN) today, so exactly one recognizer runs. The loop is written generically, but
        // returning more than one WOULD create concurrent recognizers here — which macOS can't
        // run reliably (the second is starved) — so we deliberately keep it to one. A locale
        // unavailable on this device is skipped; if none come up, bail.
        let mut recognizers: Vec<(String, *mut AnyObject)> = Vec::new();
        let mut requests: Vec<*mut AnyObject> = Vec::new();
        for loc in &locales {
            let ns_loc = make_nsstring(loc);
            let locale_obj: *mut AnyObject =
                msg_send![locale_cls, localeWithLocaleIdentifier: ns_loc];
            let r: *mut AnyObject = msg_send![speech_cls, alloc];
            let r: *mut AnyObject = msg_send![r, initWithLocale: locale_obj];
            if r.is_null() {
                continue;
            }
            let available: bool = msg_send![&*r, isAvailable];
            if !available {
                log::warn!("[voice] recognizer for {loc} unavailable, skipping");
                let _: () = msg_send![&*r, release];
                continue;
            }
            let request: *mut AnyObject = msg_send![req_cls, alloc];
            let request: *mut AnyObject = msg_send![request, init];
            let _: () = msg_send![&*request, setShouldReportPartialResults: true];
            recognizers.push((loc.clone(), r));
            requests.push(request);
        }
        if recognizers.is_empty() {
            return Err("Speech recognition not available on this device.".into());
        }
        log::info!("[voice] {} recognizer(s) active", recognizers.len());

        let engine_cls = AnyClass::get(c"AVAudioEngine").ok_or("AVAudioEngine not available")?;
        let engine: *mut AnyObject = msg_send![engine_cls, alloc];
        let engine: *mut AnyObject = msg_send![engine, init];

        let input_node: *mut AnyObject = msg_send![&*engine, inputNode];
        let format: *mut AnyObject = msg_send![&*input_node, outputFormatForBus: 0u64];
        let sample_rate: f64 = msg_send![&*format, sampleRate];
        let channel_count: u32 = msg_send![&*format, channelCount];
        log::info!("[voice] audio format: sampleRate={sample_rate} channels={channel_count}");

        let start_ms = epoch_ms();
        RECORDING_START_MS.store(start_ms, Ordering::SeqCst);
        LAST_RESULT_TICK.store(0, Ordering::SeqCst);

        // Unique id for this recording; handlers and the fallback only emit while it's current.
        let generation = RECORDING_GEN.fetch_add(1, Ordering::SeqCst) + 1;

        let selection = std::sync::Arc::new(Mutex::new(Selection {
            pending_finals: recognizers.len(),
            best_text: String::new(),
            best_conf: 0.0,
            have_best: false,
            emitted: false,
            last_partial_len: 0,
        }));

        // One recognition task per recognizer (just one today); each handler is tagged with
        // its locale and funnels results through the shared Selection.
        let mut tasks: Vec<*mut AnyObject> = Vec::new();
        for (i, (loc, recognizer)) in recognizers.iter().enumerate() {
            let recog_ptr = *recognizer;
            let request = requests[i];
            let app_handle = app.clone();
            let sel = selection.clone();
            let loc_label = loc.clone();
            // Each recognizer contributes exactly one terminal event (final OR error); this
            // guards against double-counting toward the pending total.
            let done = std::sync::Arc::new(AtomicBool::new(false));
            let handler = RcBlock::new(move |result: *mut AnyObject, error: *mut AnyObject| {
                let mut to_emit = Emit::None;
                if !result.is_null() {
                    let best: *mut AnyObject = msg_send![&*result, bestTranscription];
                    let text_ns: *mut AnyObject = msg_send![&*best, formattedString];
                    let text = nsstring_to_string(text_ns);
                    let is_final: bool = msg_send![&*result, isFinal];
                    let conf = average_confidence(best);

                    let now = epoch_ms();
                    let elapsed = now.saturating_sub(RECORDING_START_MS.load(Ordering::SeqCst));
                    LAST_RESULT_TICK.store(elapsed, Ordering::SeqCst);

                    log::info!("[voice] [{loc_label}] '{text}' final={is_final} conf={conf:.2}");
                    if is_final {
                        if !done.swap(true, Ordering::SeqCst) {
                            if let Ok(mut s) = sel.lock() {
                                to_emit = s.on_done(Some((text, conf)));
                            }
                        }
                    } else if let Ok(mut s) = sel.lock() {
                        to_emit = s.on_partial(text);
                    }
                }
                if !error.is_null() {
                    let desc: *mut AnyObject = msg_send![&*error, localizedDescription];
                    log::warn!(
                        "[voice] [{loc_label}] recognition error: {}",
                        nsstring_to_string(desc)
                    );
                    // Treat an error as "this language is done with nothing" so the matching
                    // language can emit immediately instead of waiting on the grace timer.
                    if !done.swap(true, Ordering::SeqCst) {
                        if let Ok(mut s) = sel.lock() {
                            to_emit = s.on_done(None);
                        }
                    }
                }
                match to_emit {
                    Emit::Partial(t) => {
                        emit_transcript_if_current(&app_handle, generation, t, false)
                    }
                    Emit::Final(t) => emit_transcript_if_current(&app_handle, generation, t, true),
                    Emit::None => {}
                }
            });
            let task: *mut AnyObject = msg_send![
                &*recog_ptr,
                recognitionTaskWithRequest: &*request,
                resultHandler: &*handler
            ];
            let _: () = msg_send![&*recog_ptr, retain];
            std::mem::forget(handler);
            tasks.push(task);
        }

        log::info!("[voice] installing audio tap");
        let request_addrs: Vec<usize> = requests.iter().map(|&r| r as usize).collect();
        let buf_count = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let buf_count2 = buf_count.clone();
        let tap_block = RcBlock::new(move |buffer: *mut AnyObject, _time: *mut AnyObject| {
            // Feed the same audio to every language's request.
            for &addr in &request_addrs {
                let req = addr as *mut AnyObject;
                let _: () = msg_send![&*req, appendAudioPCMBuffer: buffer];
            }
            let c = buf_count2.fetch_add(1, Ordering::Relaxed);
            if c == 0 || c == 50 || c == 200 {
                log::info!("[voice] audio buffer #{}", c + 1);
            }
        });
        let _: () = msg_send![
            &*input_node,
            installTapOnBus: 0u64,
            bufferSize: 1024u32,
            format: format,
            block: &*tap_block
        ];
        log::info!("[voice] audio tap installed");

        // Start engine
        let mut err_obj: *mut AnyObject = std::ptr::null_mut();
        let started: bool = msg_send![&*engine, startAndReturnError: &mut err_obj];
        if !started {
            let _: () = msg_send![&*input_node, removeTapOnBus: 0u64];
            for &request in &requests {
                let _: () = msg_send![&*request, endAudio];
            }
            // The engine never started, so no audio flowed and no async final is pending —
            // cancel the just-created tasks and free the engine we own (+1 from alloc/init).
            for &task in &tasks {
                let _: () = msg_send![&*task, cancel];
            }
            let _: () = msg_send![&*engine, release];
            let err_msg = if !err_obj.is_null() {
                let desc: *mut AnyObject = msg_send![&*err_obj, localizedDescription];
                nsstring_to_string(desc)
            } else {
                "Unknown error starting audio engine".into()
            };
            return Err(err_msg);
        }
        log::info!("[voice] engine started");

        std::mem::forget(tap_block);

        Ok(RecordingState {
            app: app.clone(),
            engine,
            requests,
            tasks,
            selection,
            start_time: Instant::now(),
            generation,
        })
    }
}

fn do_stop_recording(state: RecordingState) {
    unsafe {
        let input_node: *mut AnyObject = msg_send![&*state.engine, inputNode];
        let _: () = msg_send![&*input_node, removeTapOnBus: 0u64];
        let _: () = msg_send![&*state.engine, stop];
        for &request in &state.requests {
            let _: () = msg_send![&*request, endAudio];
        }
        // `finish` (not `cancel`) so each recognizer delivers its final result — cancel
        // discards the in-flight transcription, leaving is_final text empty.
        for &task in &state.tasks {
            let _: () = msg_send![&*task, finish];
        }
        // Free the AVAudioEngine (+1 from alloc/init, the largest per-recording object): the
        // tap is removed and the engine is stopped, so nothing references it asynchronously.
        // NOTE: the recognizers/requests/handler blocks are still referenced by the tasks'
        // async final callbacks here; releasing them on a timer would risk a use-after-free,
        // so a fully ARC-correct release of those needs on-device leak/UAF verification.
        let _: () = msg_send![&*state.engine, release];
        log::info!("[voice] recording stopped, finishing recognition");
    }

    // Fallback: if a recognizer errored and never finalized, the pending counter never hits
    // zero. After a short grace period, emit the best final we collected so the pet still
    // reacts (no-op if a winner was already emitted).
    let sel = state.selection.clone();
    let app = state.app.clone();
    let generation = state.generation;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(900));
        let emit = match sel.lock() {
            Ok(mut s) => s.force_emit(),
            Err(_) => Emit::None,
        };
        if let Emit::Final(t) = emit {
            emit_transcript_if_current(&app, generation, t, true);
        }
    });
}

#[cfg(test)]
mod tests {
    // The Selection state machine supports several recognizers, but the app currently runs a
    // single recognizer (recognizer_locales → zh-CN). These tests exercise the state machine
    // with multiple pending terminals to cover the wait/pick/fallback paths — they do NOT
    // imply concurrent multi-language recognition is enabled.
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
        // First recognizer finalizes; nothing emitted yet (the other is still pending).
        assert_eq!(s.on_done(Some(("摸摸头".into(), 0.9))), Emit::None);
        // Second recognizer finalizes lower-confidence; the best one wins and emits once.
        assert_eq!(
            s.on_done(Some(("garbage".into(), 0.3))),
            Emit::Final("摸摸头".into())
        );
    }

    #[test]
    fn error_counts_as_done_so_match_emits_immediately() {
        let mut s = fresh(2);
        assert_eq!(s.on_done(Some(("hello".into(), 0.8))), Emit::None);
        // The other recognizer heard nothing (error) — emit the best right away, no waiting.
        assert_eq!(s.on_done(None), Emit::Final("hello".into()));
    }

    #[test]
    fn single_recognizer_emits_its_own_final() {
        // The real runtime case today: one recognizer.
        let mut s = fresh(1);
        assert_eq!(
            s.on_done(Some(("你好".into(), 0.95))),
            Emit::Final("你好".into())
        );
    }

    #[test]
    fn stops_only_when_disabled_mid_recording() {
        assert!(should_stop_on_disable(false, true)); // turned off while recording → stop
        assert!(!should_stop_on_disable(false, false)); // off but not recording → nothing
        assert!(!should_stop_on_disable(true, true)); // enabling never stops
        assert!(!should_stop_on_disable(true, false));
    }

    #[test]
    fn accepts_start_only_when_enabled_and_idle() {
        assert!(!should_accept_start(false, false)); // disabled, idle → ignore (mic stays shut)
        assert!(!should_accept_start(false, true)); // disabled mid-record → ignore
        assert!(should_accept_start(true, false)); // enabled, idle → open the mic
        assert!(!should_accept_start(true, true)); // enabled but already recording → idempotent
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
        // No second emission from a stray late terminal or the fallback.
        assert_eq!(s.on_done(None), Emit::None);
        assert_eq!(s.force_emit(), Emit::None);
    }

    #[test]
    fn force_emit_is_the_fallback_for_a_stuck_recognizer() {
        let mut s = fresh(2);
        s.on_done(Some(("hi".into(), 0.5))); // one done, one stuck → not emitted yet
        assert_eq!(s.force_emit(), Emit::Final("hi".into()));
        assert_eq!(s.force_emit(), Emit::None); // already emitted
    }

    #[test]
    fn partials_echo_the_longest_so_far() {
        let mut s = fresh(2);
        assert_eq!(s.on_partial("ab".into()), Emit::Partial("ab".into()));
        assert_eq!(s.on_partial("a".into()), Emit::None); // shorter → no flicker back
        assert_eq!(s.on_partial("abcd".into()), Emit::Partial("abcd".into()));
        s.emitted = true;
        assert_eq!(s.on_partial("abcdef".into()), Emit::None); // nothing after the final
    }

    #[test]
    fn autostop_on_max_duration() {
        assert!(should_autostop(30_000, 5_000, 30, 8));
        assert!(!should_autostop(29_000, 28_000, 30, 8));
    }

    #[test]
    fn autostop_on_silence() {
        assert!(should_autostop(8_000, 0, 30, 8)); // nothing heard for 8s
        assert!(!should_autostop(7_000, 0, 30, 8));
        assert!(should_autostop(20_000, 12_000, 30, 8)); // 8s since last result
        assert!(!should_autostop(19_000, 12_000, 30, 8)); // 7s since last result
    }

    #[test]
    fn autostop_uses_ms_precision_no_off_by_one() {
        // 7.5s of real silence must NOT trip the 8s timeout (the seconds-truncating
        // version wrongly stopped here).
        assert!(!should_autostop(8_000, 500, 30, 8));
    }
}
