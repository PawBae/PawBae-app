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

fn current_locale() -> Option<String> {
    let g = VOICE_LOCALE.get()?.lock().ok()?;
    if g.trim().is_empty() {
        None
    } else {
        Some(g.clone())
    }
}

struct RecordingState {
    engine: *mut AnyObject,
    request: *mut AnyObject,
    task: *mut AnyObject,
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

        log::info!("[voice] creating recognizer");
        let recognizer: *mut AnyObject = match current_locale() {
            Some(loc) => {
                log::info!("[voice] creating recognizer with locale {loc}");
                let ns_loc = make_nsstring(&loc);
                let locale_cls = AnyClass::get(c"NSLocale").ok_or("NSLocale not available")?;
                let locale_obj: *mut AnyObject =
                    msg_send![locale_cls, localeWithLocaleIdentifier: ns_loc];
                let r: *mut AnyObject = msg_send![speech_cls, alloc];
                msg_send![r, initWithLocale: locale_obj]
            }
            None => {
                let r: *mut AnyObject = msg_send![speech_cls, alloc];
                msg_send![r, init]
            }
        };
        if recognizer.is_null() {
            return Err("Failed to create speech recognizer.".into());
        }
        let available: bool = msg_send![&*recognizer, isAvailable];
        log::info!("[voice] recognizer available = {}", available);
        if !available {
            let _: () = msg_send![&*recognizer, release];
            return Err("Speech recognition not available on this device.".into());
        }

        log::info!("[voice] creating request and engine");
        let req_cls = AnyClass::get(c"SFSpeechAudioBufferRecognitionRequest")
            .ok_or("SFSpeechAudioBufferRecognitionRequest not available")?;
        let request: *mut AnyObject = msg_send![req_cls, alloc];
        let request: *mut AnyObject = msg_send![request, init];
        let _: () = msg_send![&*request, setShouldReportPartialResults: true];

        let engine_cls = AnyClass::get(c"AVAudioEngine").ok_or("AVAudioEngine not available")?;
        let engine: *mut AnyObject = msg_send![engine_cls, alloc];
        let engine: *mut AnyObject = msg_send![engine, init];

        let input_node: *mut AnyObject = msg_send![&*engine, inputNode];
        let format: *mut AnyObject = msg_send![&*input_node, outputFormatForBus: 0u64];

        // Log audio format details
        let sample_rate: f64 = msg_send![&*format, sampleRate];
        let channel_count: u32 = msg_send![&*format, channelCount];
        log::info!(
            "[voice] audio format: sampleRate={} channels={}",
            sample_rate,
            channel_count
        );

        // Check microphone permission
        if let Some(av_audio_app) = AnyClass::get(c"AVAudioApplication") {
            let shared: *mut AnyObject = msg_send![av_audio_app, sharedInstance];
            if !shared.is_null() {
                let mic_status: i64 = msg_send![&*shared, recordPermission];
                log::info!(
                    "[voice] mic recordPermission = {} (0=undetermined, 1=denied, 2=granted)",
                    mic_status
                );
            }
        }

        let start_ms = epoch_ms();
        RECORDING_START_MS.store(start_ms, Ordering::SeqCst);
        LAST_RESULT_TICK.store(0, Ordering::SeqCst);

        let app_handle = app.clone();
        let result_handler = RcBlock::new(move |result: *mut AnyObject, error: *mut AnyObject| {
            if !result.is_null() {
                let best: *mut AnyObject = msg_send![&*result, bestTranscription];
                let text_ns: *mut AnyObject = msg_send![&*best, formattedString];
                let text = nsstring_to_string(text_ns);
                let is_final: bool = msg_send![&*result, isFinal];

                let now = epoch_ms();
                let elapsed = now.saturating_sub(RECORDING_START_MS.load(Ordering::SeqCst));
                LAST_RESULT_TICK.store(elapsed, Ordering::SeqCst);

                log::info!("[voice] transcript: '{}' final={}", text, is_final);
                let _ = app_handle.emit(
                    "voice-transcript",
                    VoiceTranscriptPayload { text, is_final },
                );
            }
            if !error.is_null() {
                let desc: *mut AnyObject = msg_send![&*error, localizedDescription];
                let err_str = nsstring_to_string(desc);
                log::warn!("[voice] recognition error: {}", err_str);
            }
        });

        log::info!("[voice] starting recognition task");
        let task: *mut AnyObject = msg_send![
            &*recognizer,
            recognitionTaskWithRequest: &*request,
            resultHandler: &*result_handler
        ];
        log::info!("[voice] recognition task created, task={:?}", task);

        log::info!("[voice] installing audio tap");
        let request_addr = request as usize;
        let buf_count = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let buf_count2 = buf_count.clone();
        let tap_block = RcBlock::new(move |buffer: *mut AnyObject, _time: *mut AnyObject| {
            let req = request_addr as *mut AnyObject;
            let _: () = msg_send![&*req, appendAudioPCMBuffer: buffer];
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
            let _: () = msg_send![&*request, endAudio];
            let _: () = msg_send![&*recognizer, release];
            let err_msg = if !err_obj.is_null() {
                let desc: *mut AnyObject = msg_send![&*err_obj, localizedDescription];
                nsstring_to_string(desc)
            } else {
                "Unknown error starting audio engine".into()
            };
            return Err(err_msg);
        }
        log::info!("[voice] engine started");

        let _: () = msg_send![&*recognizer, retain];
        std::mem::forget(result_handler);
        std::mem::forget(tap_block);

        Ok(RecordingState {
            engine,
            request,
            task,
            start_time: Instant::now(),
        })
    }
}

fn do_stop_recording(state: RecordingState) {
    unsafe {
        let input_node: *mut AnyObject = msg_send![&*state.engine, inputNode];
        let _: () = msg_send![&*input_node, removeTapOnBus: 0u64];
        let _: () = msg_send![&*state.engine, stop];
        let _: () = msg_send![&*state.request, endAudio];
        // `finish` (not `cancel`) so the recognizer delivers its final result — cancel
        // discards the in-flight transcription, leaving is_final text empty.
        let _: () = msg_send![&*state.task, finish];
        log::info!("[voice] recording stopped, finishing recognition");
    }
}
