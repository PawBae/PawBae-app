use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::core::HSTRING;
use windows::Foundation::TypedEventHandler;
use windows::Globalization::Language;
use windows::Media::SpeechRecognition::{
    SpeechContinuousRecognitionCompletedEventArgs,
    SpeechContinuousRecognitionResultGeneratedEventArgs, SpeechContinuousRecognitionSession,
    SpeechRecognitionHypothesisGeneratedEventArgs, SpeechRecognitionResultStatus, SpeechRecognizer,
};

use super::{
    emit_transcript_if_current, epoch_ms, recognizer_locales, Emit, Selection, LAST_RESULT_TICK,
    RECORDING_GEN, RECORDING_START_MS,
};

pub(super) struct RecordingState {
    app: tauri::AppHandle,
    #[allow(dead_code)] // kept alive so the WinRT recognizer isn't dropped mid-session
    recognizer: SpeechRecognizer,
    session: SpeechContinuousRecognitionSession,
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

/// No-op on Windows. Windows has no programmatic consent API for speech recognition;
/// the OS prompts the user automatically on first use, or the user enables "Online
/// speech recognition" in Settings → Privacy & security → Speech.
pub(super) fn request_authorization() {
    log::info!("[voice] request_authorization is a no-op on Windows (OS manages consent)");
}

pub(super) fn do_start_recording(app: &tauri::AppHandle) -> Result<RecordingState, String> {
    let locales = recognizer_locales();
    let locale_tag = locales.first().map(|s| s.as_str()).unwrap_or("zh-CN");
    log::info!("[voice] Windows: creating recognizer for locale {locale_tag}");

    let lang = Language::CreateLanguage(&HSTRING::from(locale_tag)).map_err(|e| {
        format!(
            "Language '{locale_tag}' not available. Install the language pack in \
             Settings → Time & language → Language & region. Error: {e}"
        )
    })?;

    let recognizer = SpeechRecognizer::Create(&lang).map_err(|e| {
        if e.code().0 as u32 == 0x80070005 {
            "Speech recognition denied. Enable \"Online speech recognition\" in \
             Settings → Privacy & security → Speech."
                .to_string()
        } else {
            format!(
                "Failed to create SpeechRecognizer for '{locale_tag}'. \
                 Ensure the language pack is installed. Error: {e}"
            )
        }
    })?;

    let session = recognizer
        .ContinuousRecognitionSession()
        .map_err(|e| format!("Failed to get ContinuousRecognitionSession: {e}"))?;

    let start_ms = epoch_ms();
    RECORDING_START_MS.store(start_ms, Ordering::SeqCst);
    LAST_RESULT_TICK.store(0, Ordering::SeqCst);

    let generation = RECORDING_GEN.fetch_add(1, Ordering::SeqCst) + 1;

    let selection = Arc::new(Mutex::new(Selection {
        pending_finals: 1,
        best_text: String::new(),
        best_conf: 0.0,
        have_best: false,
        emitted: false,
        last_partial_len: 0,
    }));

    // HypothesisGenerated → partial results
    {
        let sel = selection.clone();
        let app_handle = app.clone();
        recognizer
            .HypothesisGenerated(&TypedEventHandler::<
                SpeechRecognizer,
                SpeechRecognitionHypothesisGeneratedEventArgs,
            >::new(move |_sender, args| {
                if let Some(args) = args {
                    if let Ok(hyp) = args.Hypothesis() {
                        if let Ok(text_h) = hyp.Text() {
                            let text = text_h.to_string_lossy();
                            if !text.is_empty() {
                                let now = epoch_ms();
                                let elapsed =
                                    now.saturating_sub(RECORDING_START_MS.load(Ordering::SeqCst));
                                LAST_RESULT_TICK.store(elapsed, Ordering::SeqCst);

                                log::info!("[voice] [windows] partial: '{text}'");
                                let to_emit = if let Ok(mut s) = sel.lock() {
                                    s.on_partial(text.clone())
                                } else {
                                    Emit::None
                                };
                                if let Emit::Partial(t) = to_emit {
                                    emit_transcript_if_current(&app_handle, generation, t, false);
                                }
                            }
                        }
                    }
                }
                Ok(())
            }))
            .map_err(|e| format!("Failed to register HypothesisGenerated: {e}"))?;
    }

    // ResultGenerated → final results
    {
        let sel = selection.clone();
        let app_handle = app.clone();
        session
            .ResultGenerated(&TypedEventHandler::<
                SpeechContinuousRecognitionSession,
                SpeechContinuousRecognitionResultGeneratedEventArgs,
            >::new(move |_sender, args| {
                if let Some(args) = args {
                    if let Ok(result) = args.Result() {
                        let status = result.Status().unwrap_or(SpeechRecognitionResultStatus(99));
                        if status == SpeechRecognitionResultStatus::Success {
                            if let Ok(text_h) = result.Text() {
                                let text = text_h.to_string_lossy();
                                let conf = result.RawConfidence().unwrap_or(0.0);

                                let now = epoch_ms();
                                let elapsed =
                                    now.saturating_sub(RECORDING_START_MS.load(Ordering::SeqCst));
                                LAST_RESULT_TICK.store(elapsed, Ordering::SeqCst);

                                log::info!("[voice] [windows] final: '{text}' conf={conf:.2}");
                                let to_emit = if let Ok(mut s) = sel.lock() {
                                    s.on_done(Some((text, conf)))
                                } else {
                                    Emit::None
                                };
                                match to_emit {
                                    Emit::Final(t) => {
                                        emit_transcript_if_current(
                                            &app_handle,
                                            generation,
                                            t,
                                            true,
                                        );
                                    }
                                    Emit::Partial(t) => {
                                        emit_transcript_if_current(
                                            &app_handle,
                                            generation,
                                            t,
                                            false,
                                        );
                                    }
                                    Emit::None => {}
                                }
                            }
                        } else {
                            log::warn!(
                                "[voice] [windows] result status {:?}, treating as empty",
                                status.0
                            );
                        }
                    }
                }
                Ok(())
            }))
            .map_err(|e| format!("Failed to register ResultGenerated: {e}"))?;
    }

    // Completed → session ended (error or user-initiated stop)
    {
        let sel = selection.clone();
        let app_handle = app.clone();
        session
            .Completed(&TypedEventHandler::<
                SpeechContinuousRecognitionSession,
                SpeechContinuousRecognitionCompletedEventArgs,
            >::new(move |_sender, args| {
                let status_code = args
                    .as_ref()
                    .and_then(|a| a.Status().ok())
                    .map(|s| s.0)
                    .unwrap_or(0);
                log::info!("[voice] [windows] session completed, status={status_code}");
                let to_emit = if let Ok(mut s) = sel.lock() {
                    s.on_done(None)
                } else {
                    Emit::None
                };
                if let Emit::Final(t) = to_emit {
                    emit_transcript_if_current(&app_handle, generation, t, true);
                }
                Ok(())
            }))
            .map_err(|e| format!("Failed to register Completed: {e}"))?;
    }

    log::info!("[voice] [windows] starting continuous recognition");
    let map_start_err = |e: windows::core::Error| -> String {
        if e.code().0 as u32 == 0x80045509 {
            "Enable \"Online speech recognition\" in Windows Settings → \
             Privacy & security → Speech, then try again."
                .to_string()
        } else {
            format!("StartAsync failed: {e}")
        }
    };
    session
        .StartAsync()
        .map_err(&map_start_err)?
        .get()
        .map_err(&map_start_err)?;
    log::info!("[voice] [windows] continuous recognition started");

    Ok(RecordingState {
        app: app.clone(),
        recognizer,
        session,
        selection,
        start_time: Instant::now(),
        generation,
    })
}

pub(super) fn do_stop_recording(state: RecordingState) {
    log::info!("[voice] [windows] stopping continuous recognition");
    if let Err(e) = state.session.StopAsync().and_then(|op| op.get()) {
        log::warn!("[voice] [windows] StopAsync error: {e}");
    }
    log::info!("[voice] [windows] recording stopped");

    // Fallback: same 900ms grace as macOS for stuck recognizers
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
