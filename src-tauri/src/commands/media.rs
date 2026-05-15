//! Tauri media commands: system idle time, now-playing detection, system sound playback.

#[cfg(target_os = "macos")]
use tauri::Manager;

#[cfg(target_os = "macos")]
use crate::platform::macos::{
    get_frontmost_bundle_id, is_any_music_app_playing, is_browser, is_music_app, is_video_app,
    nowplaying_cli_status,
};

#[cfg(target_os = "windows")]
use crate::platform::windows::{is_browser_win, is_music_app_win, is_video_app_win};

/// Detect what media the user is consuming.
///
/// Returns: "music", "video", or "none".
///
/// Priority:
/// 1) System-level now playing playback state (MediaRemote)
/// 2) Frontmost app fallback (video/music bundle IDs)
/// 3) Explicit player-state scripts for background music fallback
#[tauri::command]
pub async fn get_system_idle_time(app: tauri::AppHandle) -> Result<f64, String> {
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel::<f64>();
        app.run_on_main_thread(move || {
            #[link(name = "CoreGraphics", kind = "framework")]
            extern "C" {
                fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
            }
            let idle = unsafe { CGEventSourceSecondsSinceLastEventType(0, 0xFFFFFFFF) };
            let _ = tx.send(idle);
        })
        .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(0.0)
    }
}
#[tauri::command]
pub async fn get_now_playing(app: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        app.run_on_main_thread(move || {
            let bid = get_frontmost_bundle_id().to_lowercase();
            let cli_status = nowplaying_cli_status();

            let result = if let Some((playing, ref source)) = cli_status {
                if !playing
                    || source.contains("openclaw")
                    || source.contains("ooclaw")
                    || source.contains("com.apple.webkit")
                {
                    // Not playing, or our own pet SFX hijacked the Now Playing session.
                    // WebView audio (HTML5 Audio / <video>) reports as "com.apple.WebKit.GPU",
                    // not the host app's bundle ID, so we must also filter that.
                    // Fall back to AppleScript to check if a real music app is still playing,
                    // because nowplaying-cli only reports one source at a time.
                    if is_any_music_app_playing() {
                        "music"
                    } else {
                        "none"
                    }
                } else if is_music_app(source) {
                    "music"
                } else if is_video_app(source) || is_browser(source) {
                    "video"
                } else {
                    "music"
                }
            } else {
                // nowplaying-cli not available, fall back to AppleScript
                if is_any_music_app_playing() {
                    "music"
                } else {
                    "none"
                }
            };
            log::info!(
                "[now_playing] frontmost_bid={} cli_status={:?} result={}",
                bid,
                cli_status,
                result
            );
            let _ = tx.send(result.into());
        })
        .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())
    }
    #[cfg(target_os = "windows")]
    {
        let result = tokio::task::spawn_blocking(|| -> Result<String, String> {
            use windows::Media::Control::{
                GlobalSystemMediaTransportControlsSessionManager,
                GlobalSystemMediaTransportControlsSessionPlaybackStatus,
            };

            let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
                .map_err(|e| format!("GSMTC RequestAsync failed: {}", e))?
                .get()
                .map_err(|e| format!("GSMTC get manager failed: {}", e))?;

            let sessions = match manager.GetSessions() {
                Ok(s) => s,
                Err(_) => return Ok("none".into()),
            };

            let count = sessions.Size().unwrap_or(0);
            let mut best: Option<&str> = None;
            for i in 0..count {
                let session: windows::Media::Control::GlobalSystemMediaTransportControlsSession =
                    match sessions.GetAt(i) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                let source = session
                    .SourceAppUserModelId()
                    .map(|s| s.to_string_lossy().to_lowercase())
                    .unwrap_or_default();

                let info = match session.GetPlaybackInfo() {
                    Ok(i) => i,
                    Err(_) => continue,
                };
                let status = match info.PlaybackStatus() {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                log::info!(
                    "[now_playing/gsmtc] source={} status={:?}",
                    source,
                    status.0
                );

                if status != GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing {
                    continue;
                }

                let kind = if is_video_app_win(&source) || is_browser_win(&source) {
                    "video"
                } else if is_music_app_win(&source) {
                    "music"
                } else {
                    "music"
                };

                if kind == "video" {
                    best = Some("video");
                    break;
                }
                if best.is_none() {
                    best = Some(kind);
                }
            }
            Ok(best.unwrap_or("none").into())
        })
        .await
        .map_err(|e| format!("spawn_blocking join error: {}", e))?;
        result
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok("none".into())
    }
}
// ─── Play system sound ───
#[tauri::command]
pub async fn play_sound(name: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::runtime::{AnyClass, AnyObject};
        let name_clone = name.clone();
        std::thread::spawn(move || unsafe {
            let cls = match AnyClass::get(c"NSSound") {
                Some(c) => c,
                None => return,
            };
            let ns_string_cls = AnyClass::get(c"NSString").unwrap();
            let c_str = std::ffi::CString::new(name_clone.as_bytes()).unwrap();
            let ns_name: *mut AnyObject =
                msg_send![ns_string_cls, stringWithUTF8String: c_str.as_ptr()];
            let sound: *mut AnyObject = msg_send![cls, soundNamed: ns_name];
            if !sound.is_null() {
                let _: () = msg_send![&*sound, play];
            }
        });
    }
    #[cfg(target_os = "windows")]
    {
        // Map macOS system sound names to Windows equivalents.
        // Windows PlaySound uses registry aliases: SystemAsterisk, SystemExclamation, etc.
        let win_sound = match name.as_str() {
            "Blow" | "Basso" | "Funk" | "Sosumi" => "SystemExclamation",
            "Bottle" | "Pop" | "Purr" | "Tink" => "SystemAsterisk",
            "Glass" | "Ping" => "SystemDefault",
            "Hero" | "Morse" | "Submarine" => "SystemNotification",
            "Frog" => "SystemQuestion",
            _ => "SystemDefault",
        };
        let sound_name = win_sound.to_string();
        std::thread::spawn(move || {
            use windows::core::PCWSTR;
            use windows::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC};
            let wide: Vec<u16> = sound_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            unsafe {
                let _ = PlaySoundW(PCWSTR(wide.as_ptr()), None, SND_ALIAS | SND_ASYNC);
            }
        });
    }
    Ok(())
}
