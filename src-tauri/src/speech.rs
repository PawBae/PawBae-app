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
static SPEECH_TX: OnceLock<Mutex<mpsc::Sender<SpeechCommand>>> = OnceLock::new();

const MAX_RECORDING_SECS: u64 = 30;
const SILENCE_TIMEOUT_SECS: u64 = 8;

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

pub fn start_recording() -> Result<(), String> {
    if VOICE_ACTIVE.load(Ordering::SeqCst) {
        return Ok(());
    }
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

/// Ask macOS for Speech Recognition permission once at startup. The previous code only
/// *read* `authorizationStatus`, so an unprompted (notDetermined) status never advanced and
/// the recognizer silently returned "No speech detected". This triggers the system prompt
/// (requires `NSSpeechRecognitionUsageDescription` in the bundle Info.plist) and logs the
/// outcome. Calling it when already authorized/denied is a cheap no-op.
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
    request_authorization();

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

// Locale (e.g. "zh-CN" / "en-US") the recognizer is built with. The frontend sets this
// from the app language via the `voice_set_locale` command, so a Chinese user's speech is
// recognized as Chinese instead of falling back to the system locale. Empty → system default.
static VOICE_LOCALE: OnceLock<Mutex<String>> = OnceLock::new();

pub fn set_voice_locale(locale: String) {
    let cell = VOICE_LOCALE.get_or_init(|| Mutex::new(String::new()));
    if let Ok(mut g) = cell.lock() {
        *g = locale;
    }
}

/// Locales the recognizer set runs. Empty or "auto" → bilingual (zh-CN + en-US) so the user
/// can speak either language without switching anything; a specific identifier pins to one.
fn recognizer_locales() -> Vec<String> {
    let pinned = VOICE_LOCALE
        .get()
        .and_then(|m| m.lock().ok())
        .map(|g| g.trim().to_string())
        .unwrap_or_default();
    if pinned.is_empty() || pinned.eq_ignore_ascii_case("auto") {
        vec!["zh-CN".to_string(), "en-US".to_string()]
    } else {
        vec![pinned]
    }
}

/// Shared across the per-language result handlers: picks the highest-confidence FINAL
/// transcript among the recognizers (each language hears the same audio; the wrong-language
/// recognizer scores low), while letting partials echo live. Emits the final exactly once.
struct Selection {
    pending_finals: usize,
    best_text: String,
    best_conf: f64,
    have_best: bool,
    emitted: bool,
    last_partial_len: usize,
}

enum Emit {
    None,
    Partial(String),
    Final(String),
}

impl Selection {
    /// A live partial result. Echo the longest one so far (monotonic) to avoid flicker
    /// between the two languages.
    fn on_partial(&mut self, text: String) -> Emit {
        if !self.emitted && text.len() > self.last_partial_len {
            self.last_partial_len = text.len();
            Emit::Partial(text)
        } else {
            Emit::None
        }
    }

    /// A recognizer reached a terminal state: `Some((text, conf))` for a final transcript,
    /// or `None` on error (that language heard nothing). Decrements the pending count and
    /// emits the highest-confidence final as soon as every recognizer is done — so a single
    /// matching language no longer waits on the other's silence.
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
}

unsafe impl Send for RecordingState {}

fn speech_thread_main(app: tauri::AppHandle, rx: mpsc::Receiver<SpeechCommand>) {
    let mut recording: Option<RecordingState> = None;

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(SpeechCommand::Start) => {
                log::info!("[voice] received Start command");
                if recording.is_some() {
                    continue;
                }
                match do_start_recording(&app) {
                    Ok(state) => {
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
            let elapsed = state.start_time.elapsed().as_secs();
            let last_result_ms = LAST_RESULT_TICK.load(Ordering::SeqCst);
            let silence_elapsed = if last_result_ms == 0 {
                elapsed
            } else {
                elapsed.saturating_sub(last_result_ms / 1000)
            };

            let should_stop =
                elapsed >= MAX_RECORDING_SECS || silence_elapsed >= SILENCE_TIMEOUT_SECS;

            if should_stop {
                log::info!(
                    "[voice] auto-stop: elapsed={}s silence={}s",
                    elapsed,
                    silence_elapsed
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
/// confidences; the wrong-language recognizer scores low, which is how we pick the winner.
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

        // One recognizer + request per locale (e.g. zh-CN + en-US). A locale unavailable on
        // this device is skipped; if none come up, bail.
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

        let selection = std::sync::Arc::new(Mutex::new(Selection {
            pending_finals: recognizers.len(),
            best_text: String::new(),
            best_conf: 0.0,
            have_best: false,
            emitted: false,
            last_partial_len: 0,
        }));

        // One recognition task per language; each handler is tagged with its locale and
        // funnels results through the shared Selection.
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
                    log::warn!("[voice] [{loc_label}] recognition error: {}", nsstring_to_string(desc));
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
                        let _ = app_handle.emit(
                            "voice-transcript",
                            VoiceTranscriptPayload { text: t, is_final: false },
                        );
                    }
                    Emit::Final(t) => {
                        let _ = app_handle.emit(
                            "voice-transcript",
                            VoiceTranscriptPayload { text: t, is_final: true },
                        );
                    }
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
        log::info!("[voice] recording stopped, finishing recognition");
    }

    // Fallback: if a recognizer errored and never finalized, the pending counter never hits
    // zero. After a short grace period, emit the best final we collected so the pet still
    // reacts (no-op if a winner was already emitted).
    let sel = state.selection.clone();
    let app = state.app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(900));
        let emit = match sel.lock() {
            Ok(mut s) => s.force_emit(),
            Err(_) => Emit::None,
        };
        if let Emit::Final(t) = emit {
            let _ = app.emit("voice-transcript", VoiceTranscriptPayload { text: t, is_final: true });
        }
    });
}
