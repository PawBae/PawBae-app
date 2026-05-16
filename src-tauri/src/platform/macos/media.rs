//! Media detection: music apps, now-playing status, video apps, browsers, audio output.

/// Get the bundle identifier of the frontmost application.
pub(crate) fn get_frontmost_bundle_id() -> String {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    unsafe {
        let cls = match AnyClass::get(c"NSWorkspace") {
            Some(c) => c,
            None => return String::new(),
        };
        let ws: *mut AnyObject = msg_send![cls, sharedWorkspace];
        if ws.is_null() {
            return String::new();
        }
        let front_app: *mut AnyObject = msg_send![&*ws, frontmostApplication];
        if front_app.is_null() {
            return String::new();
        }
        let bid_ns: *mut AnyObject = msg_send![&*front_app, bundleIdentifier];
        if bid_ns.is_null() {
            return String::new();
        }
        let utf8: *const u8 = msg_send![&*bid_ns, UTF8String];
        if utf8.is_null() {
            return String::new();
        }
        let len: usize = msg_send![&*bid_ns, length];
        String::from_utf8_lossy(std::slice::from_raw_parts(utf8, len)).into_owned()
    }
}
const MUSIC_APP_BIDS: &[&str] = &[
    "com.apple.music",
    "com.spotify.client",
    "com.netease.163music",
    "com.tencent.qqmusic",
    "com.kugou",
    "com.kuwo",
    "com.xiami.client",
    "com.apple.itunes",
    "com.soda.music",
    "com.bytedance.soda.music",
];
pub(crate) fn is_music_app(bid: &str) -> bool {
    MUSIC_APP_BIDS.iter().any(|m| bid.contains(m))
}
#[allow(dead_code, clashing_extern_declarations)]
fn is_music_app_running() -> bool {
    let script = r#"
        set musicBids to {"com.apple.music", "com.spotify.client", "com.netease.163music", "com.tencent.qqmusic", "com.kugou", "com.kuwo", "com.xiami.client", "com.apple.itunes", "com.soda.music", "com.bytedance.soda.music"}
        repeat with bid in musicBids
            try
                if application id (bid as text) is running then return "1"
            end try
        end repeat
        return "0"
    "#;
    match std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim() == "1",
        Err(_) => false,
    }
}
fn _get_system_now_playing_is_playing_unused() -> Option<bool> {
    use block2::RcBlock;
    use std::ffi::c_void;
    use std::sync::mpsc::channel;
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    type DispatchQueue = *mut std::ffi::c_void;
    type PlaybackState = u32;

    const MEDIA_REMOTE_PLAYING: PlaybackState = 1;
    const MEDIA_REMOTE_AMBIGUOUS: PlaybackState = 2;
    const K_CFNUMBER_DOUBLE_TYPE: i32 = 13;
    const K_CFSTRING_ENCODING_UTF8: u32 = 0x0800_0100;

    type MrGetIsPlayingFn = unsafe extern "C" fn(DispatchQueue, &block2::Block<dyn Fn(i8)>);
    type MrGetPlaybackStateFn =
        unsafe extern "C" fn(DispatchQueue, &block2::Block<dyn Fn(PlaybackState)>);
    type MrGetNowPlayingInfoFn =
        unsafe extern "C" fn(DispatchQueue, &block2::Block<dyn Fn(*const c_void)>);
    type DispatchGetGlobalQueueFn = unsafe extern "C" fn(isize, usize) -> DispatchQueue;

    #[allow(dead_code, clashing_extern_declarations)]
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
        fn CFNumberGetValue(number: *const c_void, the_type: i32, value: *mut c_void) -> u8;
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const u8,
            encoding: u32,
        ) -> *const c_void;
    }

    static MR_GET_IS_PLAYING_FN: OnceLock<MrGetIsPlayingFn> = OnceLock::new();
    static MR_GET_STATE_FN: OnceLock<MrGetPlaybackStateFn> = OnceLock::new();
    static MR_GET_INFO_FN: OnceLock<MrGetNowPlayingInfoFn> = OnceLock::new();
    static MR_PLAYBACK_RATE_KEY_ADDR: OnceLock<usize> = OnceLock::new();
    static MR_ELAPSED_TIME_KEY_ADDR: OnceLock<usize> = OnceLock::new();
    static DISPATCH_GET_GLOBAL_QUEUE_FN: OnceLock<DispatchGetGlobalQueueFn> = OnceLock::new();
    static LAST_ELAPSED_SAMPLE: OnceLock<Mutex<Option<(f64, f64)>>> = OnceLock::new();

    unsafe {
        let mr_handle = libc::dlopen(
            c"/System/Library/PrivateFrameworks/MediaRemote.framework/MediaRemote"
                .as_ptr()
                .cast(),
            libc::RTLD_NOW,
        );
        if mr_handle.is_null() {
            log::info!("[now_playing/media_remote] dlopen MediaRemote failed");
            return None;
        }

        let get_is_playing = if let Some(f) = MR_GET_IS_PLAYING_FN.get() {
            Some(*f)
        } else {
            let mr_is_playing_sym = libc::dlsym(
                mr_handle,
                c"MRMediaRemoteGetNowPlayingApplicationIsPlaying"
                    .as_ptr()
                    .cast(),
            );
            if mr_is_playing_sym.is_null() {
                None
            } else {
                let f: MrGetIsPlayingFn =
                    std::mem::transmute::<*mut c_void, MrGetIsPlayingFn>(mr_is_playing_sym);
                let _ = MR_GET_IS_PLAYING_FN.set(f);
                Some(f)
            }
        };

        let get_playback_state = if let Some(f) = MR_GET_STATE_FN.get() {
            Some(*f)
        } else {
            let mr_handle = libc::dlopen(
                c"/System/Library/PrivateFrameworks/MediaRemote.framework/MediaRemote"
                    .as_ptr()
                    .cast(),
                libc::RTLD_NOW,
            );
            if mr_handle.is_null() {
                None
            } else {
                let mr_sym = libc::dlsym(
                    mr_handle,
                    c"MRMediaRemoteGetNowPlayingApplicationPlaybackState"
                        .as_ptr()
                        .cast(),
                );
                if mr_sym.is_null() {
                    None
                } else {
                    let f: MrGetPlaybackStateFn =
                        std::mem::transmute::<*mut c_void, MrGetPlaybackStateFn>(mr_sym);
                    let _ = MR_GET_STATE_FN.set(f);
                    Some(f)
                }
            }
        };

        let get_now_playing_info = if let Some(f) = MR_GET_INFO_FN.get() {
            Some(*f)
        } else {
            let mr_info_sym =
                libc::dlsym(mr_handle, c"MRMediaRemoteGetNowPlayingInfo".as_ptr().cast());
            if mr_info_sym.is_null() {
                None
            } else {
                let f: MrGetNowPlayingInfoFn =
                    std::mem::transmute::<*mut c_void, MrGetNowPlayingInfoFn>(mr_info_sym);
                let _ = MR_GET_INFO_FN.set(f);
                Some(f)
            }
        };

        let playback_rate_key = if let Some(addr) = MR_PLAYBACK_RATE_KEY_ADDR.get() {
            Some(*addr as *const c_void)
        } else {
            let key_sym = libc::dlsym(
                mr_handle,
                c"kMRMediaRemoteNowPlayingInfoPlaybackRate".as_ptr().cast(),
            );
            let key = if key_sym.is_null() {
                let fallback = CFStringCreateWithCString(
                    std::ptr::null(),
                    c"kMRMediaRemoteNowPlayingInfoPlaybackRate".as_ptr().cast(),
                    K_CFSTRING_ENCODING_UTF8,
                );
                if fallback.is_null() {
                    std::ptr::null()
                } else {
                    fallback
                }
            } else {
                // Exported as CFStringRef* global; dereference once to get key object.
                *(key_sym as *const *const c_void)
            };
            if key.is_null() {
                None
            } else {
                let _ = MR_PLAYBACK_RATE_KEY_ADDR.set(key as usize);
                Some(key)
            }
        };

        let elapsed_time_key = if let Some(addr) = MR_ELAPSED_TIME_KEY_ADDR.get() {
            Some(*addr as *const c_void)
        } else {
            let key_sym = libc::dlsym(
                mr_handle,
                c"kMRMediaRemoteNowPlayingInfoElapsedTime".as_ptr().cast(),
            );
            let key = if key_sym.is_null() {
                let fallback = CFStringCreateWithCString(
                    std::ptr::null(),
                    c"kMRMediaRemoteNowPlayingInfoElapsedTime".as_ptr().cast(),
                    K_CFSTRING_ENCODING_UTF8,
                );
                if fallback.is_null() {
                    std::ptr::null()
                } else {
                    fallback
                }
            } else {
                *(key_sym as *const *const c_void)
            };
            if key.is_null() {
                None
            } else {
                let _ = MR_ELAPSED_TIME_KEY_ADDR.set(key as usize);
                Some(key)
            }
        };

        let get_global_queue = if let Some(f) = DISPATCH_GET_GLOBAL_QUEUE_FN.get() {
            *f
        } else {
            let dispatch_handle = libc::dlopen(
                c"/usr/lib/system/libdispatch.dylib".as_ptr().cast(),
                libc::RTLD_NOW,
            );
            if dispatch_handle.is_null() {
                log::info!("[now_playing/media_remote] dlopen libdispatch failed");
                return None;
            }
            let dispatch_sym = libc::dlsym(
                dispatch_handle,
                c"dispatch_get_global_queue".as_ptr().cast(),
            );
            if dispatch_sym.is_null() {
                log::info!("[now_playing/media_remote] dlsym dispatch_get_global_queue failed");
                return None;
            }
            let f: DispatchGetGlobalQueueFn =
                std::mem::transmute::<*mut c_void, DispatchGetGlobalQueueFn>(dispatch_sym);
            let _ = DISPATCH_GET_GLOBAL_QUEUE_FN.set(f);
            f
        };

        let queue = get_global_queue(0, 0);

        // Best signal: now playing info playbackRate (0 paused, 1 playing).
        if let Some(get_now_playing_info_fn) = get_now_playing_info {
            let (tx, rx) = channel::<(Option<f64>, Option<f64>)>();
            let callback = RcBlock::new(move |info: *const c_void| {
                if info.is_null() {
                    let _ = tx.send((None, None));
                    return;
                }
                let read_number = |key: Option<*const c_void>| -> Option<f64> {
                    let k = key?;
                    let value = CFDictionaryGetValue(info, k);
                    if value.is_null() {
                        return None;
                    }
                    let mut n: f64 = 0.0;
                    let ok = CFNumberGetValue(
                        value,
                        K_CFNUMBER_DOUBLE_TYPE,
                        &mut n as *mut f64 as *mut c_void,
                    );
                    if ok != 0 {
                        Some(n)
                    } else {
                        None
                    }
                };
                let rate = read_number(playback_rate_key);
                let elapsed = read_number(elapsed_time_key);
                let _ = tx.send((rate, elapsed));
            });
            get_now_playing_info_fn(queue, &callback);
            match rx.recv_timeout(Duration::from_millis(220)) {
                Ok((Some(rate), _)) => {
                    let is_playing = rate > 0.01;
                    log::info!(
                        "[now_playing/media_remote] playback_rate={} source=now_playing_info is_playing={}",
                        rate, is_playing
                    );
                    return Some(is_playing);
                }
                Ok((None, Some(elapsed))) => {
                    let now_sec = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs_f64())
                        .unwrap_or(0.0);
                    let cache = LAST_ELAPSED_SAMPLE.get_or_init(|| Mutex::new(None));
                    let mut guard = cache.lock().unwrap();
                    let inferred = if let Some((prev_elapsed, prev_ts)) = *guard {
                        let dt = (now_sec - prev_ts).max(0.001);
                        let de = elapsed - prev_elapsed;
                        // Progress increasing at a meaningful pace => playing.
                        // Paused typically keeps elapsed almost unchanged.
                        Some(de > dt * 0.15)
                    } else {
                        None
                    };
                    *guard = Some((elapsed, now_sec));
                    log::info!(
                        "[now_playing/media_remote] elapsed_time={} source=elapsed_fallback inferred={:?}",
                        elapsed, inferred
                    );
                    if let Some(v) = inferred {
                        return Some(v);
                    }
                }
                Ok((None, None)) => {
                    log::info!(
                        "[now_playing/media_remote] playback_rate/elapsed missing source=now_playing_info fallback=is_playing/state"
                    );
                }
                Err(_) => {
                    log::info!(
                        "[now_playing/media_remote] now_playing_info timeout fallback=is_playing/state"
                    );
                }
            }
        }

        let mut is_playing_api_result: Option<bool> = None;
        if let Some(get_is_playing_fn) = get_is_playing {
            let (tx, rx) = channel::<i8>();
            let callback = RcBlock::new(move |is_playing: i8| {
                let _ = tx.send(is_playing);
            });
            get_is_playing_fn(queue, &callback);
            match rx.recv_timeout(Duration::from_millis(220)) {
                Ok(is_playing_raw) => {
                    let is_playing = is_playing_raw != 0;
                    log::info!(
                        "[now_playing/media_remote] is_playing_api={} source=is_playing",
                        is_playing
                    );
                    is_playing_api_result = Some(is_playing);
                }
                Err(_) => {
                    log::info!("[now_playing/media_remote] is_playing_api timeout, fallback=playback_state");
                }
            }
        }

        if let Some(get_playback_state_fn) = get_playback_state {
            let (tx, rx) = channel::<PlaybackState>();
            let callback = RcBlock::new(move |state: PlaybackState| {
                let _ = tx.send(state);
            });
            get_playback_state_fn(queue, &callback);
            let playback_state_result = match rx.recv_timeout(Duration::from_millis(220)) {
                Ok(state) => {
                    log::info!(
                        "[now_playing/media_remote] playback_state={} source=state_fallback",
                        state
                    );
                    Some(state)
                }
                Err(_) => {
                    log::info!("[now_playing/media_remote] playback_state timeout");
                    None
                }
            };
            let audio_active = is_audio_output_active();
            return match (is_playing_api_result, playback_state_result) {
                // Prefer explicit API when it reliably reports playing.
                (Some(true), _) => Some(true),
                // Some integrations always return false from is_playing API.
                // In that case, accept ambiguous state=2 only when audio output is active.
                (Some(false), Some(state)) if state == MEDIA_REMOTE_AMBIGUOUS => {
                    let inferred = false;
                    log::info!(
                        "[now_playing/media_remote] reconcile is_playing=false state=2 audio_active={} inferred={}",
                        audio_active, inferred
                    );
                    Some(inferred)
                }
                (Some(false), Some(state)) => {
                    let inferred = state == MEDIA_REMOTE_PLAYING;
                    log::info!(
                        "[now_playing/media_remote] reconcile is_playing=false state={} inferred={}",
                        state, inferred
                    );
                    Some(inferred)
                }
                // If explicit API timed out/unavailable, use state + audio tie-breaker.
                (None, Some(state)) if state == MEDIA_REMOTE_AMBIGUOUS => {
                    let inferred = audio_active;
                    log::info!(
                        "[now_playing/media_remote] reconcile no_is_playing state=2 audio_active={} inferred={}",
                        audio_active, inferred
                    );
                    Some(inferred)
                }
                (None, Some(state)) => Some(state == MEDIA_REMOTE_PLAYING),
                (Some(v), None) => Some(v),
                (None, None) => None,
            };
        }

        if is_playing_api_result.is_some() {
            return is_playing_api_result;
        }
        log::info!("[now_playing/media_remote] no usable media_remote symbol");
        None
    }
}
/// Check if the default audio output device has any audio running.
/// Used only as a tie-breaker for ambiguous MediaRemote states.
#[allow(dead_code)]
fn is_audio_output_active() -> bool {
    #[allow(non_upper_case_globals)]
    const kAudioHardwarePropertyDefaultOutputDevice: u32 = u32::from_be_bytes(*b"dOut");
    #[allow(non_upper_case_globals)]
    const kAudioDevicePropertyDeviceIsRunningSomewhere: u32 = u32::from_be_bytes(*b"gone");
    #[allow(non_upper_case_globals)]
    const kAudioObjectPropertyScopeGlobal: u32 = u32::from_be_bytes(*b"glob");
    #[allow(non_upper_case_globals)]
    const kAudioObjectPropertyElementMain: u32 = 0;
    #[allow(non_upper_case_globals)]
    const kAudioObjectSystemObject: u32 = 1;

    #[repr(C)]
    struct AudioObjectPropertyAddress {
        selector: u32,
        scope: u32,
        element: u32,
    }

    #[allow(dead_code)]
    #[link(name = "CoreAudio", kind = "framework")]
    unsafe extern "C" {
        fn AudioObjectGetPropertyData(
            id: u32,
            addr: *const AudioObjectPropertyAddress,
            qualifier_size: u32,
            qualifier: *const std::ffi::c_void,
            data_size: *mut u32,
            data: *mut std::ffi::c_void,
        ) -> i32;
    }

    unsafe {
        let addr = AudioObjectPropertyAddress {
            selector: kAudioHardwarePropertyDefaultOutputDevice,
            scope: kAudioObjectPropertyScopeGlobal,
            element: kAudioObjectPropertyElementMain,
        };
        let mut device: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let err = AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            &mut device as *mut u32 as *mut std::ffi::c_void,
        );
        if err != 0 || device == 0 {
            return false;
        }

        let addr2 = AudioObjectPropertyAddress {
            selector: kAudioDevicePropertyDeviceIsRunningSomewhere,
            scope: kAudioObjectPropertyScopeGlobal,
            element: kAudioObjectPropertyElementMain,
        };
        let mut running: u32 = 0;
        size = std::mem::size_of::<u32>() as u32;
        let err2 = AudioObjectGetPropertyData(
            device,
            &addr2,
            0,
            std::ptr::null(),
            &mut size,
            &mut running as *mut u32 as *mut std::ffi::c_void,
        );
        err2 == 0 && running != 0
    }
}
/// Use `nowplaying-cli` to check playback rate and source app.
/// Returns (is_playing, source_bundle_id) or None if tool unavailable.
pub(crate) fn nowplaying_cli_status() -> Option<(bool, String)> {
    static CLI_PATH: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let path = CLI_PATH.get_or_init(|| {
        for p in &[
            "/opt/homebrew/bin/nowplaying-cli",
            "/usr/local/bin/nowplaying-cli",
        ] {
            if std::path::Path::new(p).exists() {
                return Some(p.to_string());
            }
        }
        None
    });
    let cli = path.as_deref()?;
    let output = std::process::Command::new(cli)
        .args(["get", "playbackRate", "clientBundleIdentifier"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();
    let rate: f64 = lines.next()?.trim().parse().ok()?;
    let source_bid = lines.next().unwrap_or("").trim().to_lowercase();
    Some((rate > 0.01, source_bid))
}
pub(crate) fn is_any_music_app_playing() -> bool {
    let script = r#"
        set isPlaying to false

        -- Check apps that support "player state" AppleScript
        if application "Music" is running then
            tell application "Music"
                try
                    if player state is playing then set isPlaying to true
                end try
            end tell
        end if

        if (not isPlaying) and application "Spotify" is running then
            tell application "Spotify"
                try
                    if player state is playing then set isPlaying to true
                end try
            end tell
        end if

        -- For apps without AppleScript player-state (NeteaseMusic, QQ Music, etc.),
        -- check the system menu bar: the first item in the "控制" menu
        -- toggles between "播放"/"暂停" or "Play"/"Pause".
        if not isPlaying then
            tell application "System Events"
                set menuChecks to {{"com.netease.163music", "控制"}, {"com.tencent.qqmusic", "控制"}, {"com.soda.music", "控制"}, {"com.bytedance.soda.music", "控制"}}
                repeat with entry in menuChecks
                    if isPlaying then exit repeat
                    set bid to item 1 of entry
                    set menuName to item 2 of entry
                    try
                        set procs to every process whose bundle identifier is bid
                        if (count of procs) > 0 then
                            set p to item 1 of procs
                            set firstItem to name of menu item 1 of menu 1 of menu bar item menuName of menu bar 1 of p
                            if firstItem is "暂停" or firstItem is "Pause" then
                                set isPlaying to true
                            end if
                        end if
                    end try
                end repeat
            end tell
        end if

        if isPlaying then
            return "1"
        else
            return "0"
        end if
    "#;

    match std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
    {
        Ok(output) => {
            let result = String::from_utf8_lossy(&output.stdout).trim() == "1";
            log::info!("[now_playing/script] is_any_music_app_playing={}", result);
            result
        }
        Err(_) => false,
    }
}
pub(crate) fn is_video_app(bid: &str) -> bool {
    const VIDEO_APPS: &[&str] = &[
        "com.colliderli.iina",
        "org.videolan.vlc",
        "com.apple.quicktimeplayer",
        "tv.plex.plexmediaplayer",
        "io.mpv",
        "com.apple.tv",
        "com.bilibili.bili",
        "com.disneyplus",
        "com.netflix",
    ];
    VIDEO_APPS.iter().any(|v| bid.contains(v))
}
pub(crate) fn is_browser(bid: &str) -> bool {
    const BROWSERS: &[&str] = &[
        "com.google.chrome",
        "org.mozilla.firefox",
        "com.apple.safari",
        "com.microsoft.edgemac",
        "com.brave.browser",
        "com.vivaldi.vivaldi",
        "company.thebrowser.browser",
        "com.operasoftware.opera",
    ];
    BROWSERS.iter().any(|b| bid.contains(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_music_app_spotify() {
        assert!(is_music_app("com.spotify.client"));
    }

    #[test]
    fn is_music_app_apple_music() {
        assert!(is_music_app("com.apple.music"));
    }

    #[test]
    fn is_music_app_unknown() {
        assert!(!is_music_app("com.example.calculator"));
    }

    #[test]
    fn is_video_app_vlc() {
        assert!(is_video_app("org.videolan.vlc"));
    }

    #[test]
    fn is_video_app_iina() {
        assert!(is_video_app("com.colliderli.iina"));
    }

    #[test]
    fn is_video_app_unknown() {
        assert!(!is_video_app("com.example.notes"));
    }

    #[test]
    fn is_browser_chrome() {
        assert!(is_browser("com.google.chrome"));
    }

    #[test]
    fn is_browser_safari() {
        assert!(is_browser("com.apple.safari"));
    }

    #[test]
    fn is_browser_unknown() {
        assert!(!is_browser("com.example.terminal"));
    }
}
