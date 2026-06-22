use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject};

use super::{
    emit_transcript_if_current, recognizer_locales, Emit, Selection, LAST_RESULT_TICK,
    RECORDING_GEN,
};

#[link(name = "Speech", kind = "framework")]
unsafe extern "C" {}
#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {}

pub(super) struct RecordingState {
    pub(super) app: tauri::AppHandle,
    engine: *mut AnyObject,
    requests: Vec<*mut AnyObject>,
    tasks: Vec<*mut AnyObject>,
    selection: Arc<Mutex<Selection>>,
    start_time: Instant,
    generation: u64,
}

unsafe impl Send for RecordingState {}

impl RecordingState {
    pub(super) fn start_time(&self) -> Instant {
        self.start_time
    }
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

/// Ask macOS for Speech Recognition permission. Called when the user opts in (enables voice)
/// or on their first recording — NOT at app launch, so a fresh opted-out user sees no prompt.
pub(super) fn request_authorization() {
    unsafe {
        let Some(speech_cls) = AnyClass::get(c"SFSpeechRecognizer") else {
            log::warn!("[voice] SFSpeechRecognizer unavailable; cannot request authorization");
            return;
        };
        let current: i64 = msg_send![speech_cls, authorizationStatus];
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

pub(super) fn do_start_recording(app: &tauri::AppHandle) -> Result<RecordingState, String> {
    unsafe {
        let speech_cls =
            AnyClass::get(c"SFSpeechRecognizer").ok_or("SFSpeechRecognizer not available")?;

        let auth_status: i64 = msg_send![speech_cls, authorizationStatus];
        log::info!("[voice] speech auth status = {} (0=notDetermined, 1=denied, 2=restricted, 3=authorized)", auth_status);
        if auth_status == 1 || auth_status == 2 {
            return Err("Speech recognition denied. Enable in System Settings → Privacy & Security → Speech Recognition.".into());
        }

        let locales = recognizer_locales();
        log::info!("[voice] creating recognizers for locales {locales:?}");
        let locale_cls = AnyClass::get(c"NSLocale").ok_or("NSLocale not available")?;
        let req_cls = AnyClass::get(c"SFSpeechAudioBufferRecognitionRequest")
            .ok_or("SFSpeechAudioBufferRecognitionRequest not available")?;

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

        LAST_RESULT_TICK.store(0, Ordering::SeqCst);

        let generation = RECORDING_GEN.fetch_add(1, Ordering::SeqCst) + 1;
        let start_time = Instant::now();

        let selection = Arc::new(Mutex::new(Selection {
            pending_finals: recognizers.len(),
            best_text: String::new(),
            best_conf: 0.0,
            have_best: false,
            emitted: false,
            last_partial_len: 0,
        }));

        let mut tasks: Vec<*mut AnyObject> = Vec::new();
        for (i, (loc, recognizer)) in recognizers.iter().enumerate() {
            let recog_ptr = *recognizer;
            let request = requests[i];
            let app_handle = app.clone();
            let sel = selection.clone();
            let loc_label = loc.clone();
            let done = Arc::new(AtomicBool::new(false));
            let cb_start = start_time;
            let handler = RcBlock::new(move |result: *mut AnyObject, error: *mut AnyObject| {
                if RECORDING_GEN.load(Ordering::SeqCst) != generation {
                    return;
                }
                let mut to_emit = Emit::None;
                if !result.is_null() {
                    let best: *mut AnyObject = msg_send![&*result, bestTranscription];
                    let text_ns: *mut AnyObject = msg_send![&*best, formattedString];
                    let text = nsstring_to_string(text_ns);
                    let is_final: bool = msg_send![&*result, isFinal];
                    let conf = average_confidence(best);

                    let elapsed = cb_start.elapsed().as_millis() as u64;
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
        let buf_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let buf_count2 = buf_count.clone();
        let tap_block = RcBlock::new(move |buffer: *mut AnyObject, _time: *mut AnyObject| {
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

        let mut err_obj: *mut AnyObject = std::ptr::null_mut();
        let started: bool = msg_send![&*engine, startAndReturnError: &mut err_obj];
        if !started {
            let _: () = msg_send![&*input_node, removeTapOnBus: 0u64];
            for &request in &requests {
                let _: () = msg_send![&*request, endAudio];
            }
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
            start_time,
            generation,
        })
    }
}

pub(super) fn do_stop_recording(state: RecordingState) {
    unsafe {
        let input_node: *mut AnyObject = msg_send![&*state.engine, inputNode];
        let _: () = msg_send![&*input_node, removeTapOnBus: 0u64];
        let _: () = msg_send![&*state.engine, stop];
        for &request in &state.requests {
            let _: () = msg_send![&*request, endAudio];
        }
        for &task in &state.tasks {
            let _: () = msg_send![&*task, finish];
        }
        let _: () = msg_send![&*state.engine, release];
        log::info!("[voice] recording stopped, finishing recognition");
    }

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
