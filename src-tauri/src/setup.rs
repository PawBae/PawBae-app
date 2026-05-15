//! Tauri Builder::setup body — startup hooks, window props, tray + socket init.

use crate::app_init;
use crate::commands::hook::{install_claude_hooks, install_cursor_hooks};
#[cfg(target_os = "windows")]
use crate::mascot::MASCOT_TOP_INSET;
#[cfg(target_os = "macos")]
use crate::platform::macos::{get_notch_offset, install_wry_webview_ime_fix};
#[cfg(target_os = "windows")]
use crate::platform::windows::fullscreen_foreground_monitor;
#[cfg(target_os = "macos")]
use crate::speech;
#[cfg(target_os = "windows")]
use crate::state::FULLSCREEN_HIDING;
#[cfg(target_os = "macos")]
use crate::state::{MINI_WINDOW_FRAME, NOTCH_SCREEN_INFO};
use crate::{socket, tray};

use tauri::Manager;

#[cfg(target_os = "windows")]
pub(crate) fn init_webview2_env() {
    // WebView2 hardware video decode can drop VP9 alpha; force software decode.
    let key = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";
    let flag = "--disable-accelerated-video-decode";
    let merged = match std::env::var(key) {
        Ok(existing) if !existing.contains(flag) && !existing.trim().is_empty() => {
            format!("{} {}", existing, flag)
        }
        Ok(existing) if existing.contains(flag) => existing,
        _ => flag.to_string(),
    };
    std::env::set_var(key, merged);
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn init_webview2_env() {}

pub(crate) fn init(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Fix PATH so openclaw (Node.js script) and node are both reachable
    app_init::fix_path();

    // Install Claude + Codex hooks on every startup (idempotent)
    if let Err(e) = tauri::async_runtime::block_on(install_claude_hooks()) {
        log::warn!("Failed to install Claude hooks on startup: {}", e);
    }
    // Install Cursor hooks + terminal-focus extension on startup (idempotent)
    if let Err(e) = tauri::async_runtime::block_on(install_cursor_hooks()) {
        log::warn!("Failed to install Cursor hooks on startup: {}", e);
    }

    app.handle().plugin(
        tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
    )?;

    // Run the WKWebView swizzle AFTER the log plugin is initialized so
    // its [first-mouse] / IME log lines are actually visible in the
    // tauri-plugin-log stream. Order vs window creation is fine —
    // setup() runs after the mini webview already exists.
    #[cfg(target_os = "macos")]
    install_wry_webview_ime_fix();

    // Init speech recognition thread and register global shortcut
    #[cfg(target_os = "macos")]
    {
        speech::init_speech_thread(app.handle().clone());
        log::info!("[voice] speech thread started, registering shortcut");
        use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
        if let Err(e) =
            app.global_shortcut()
                .on_shortcut("ctrl+shift+v", move |_app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        log::info!(
                            "[voice] shortcut pressed, recording={}",
                            speech::is_recording()
                        );
                        if speech::is_recording() {
                            let _ = speech::stop_recording();
                        } else {
                            let _ = speech::start_recording();
                        }
                    }
                })
        {
            log::warn!("[voice] failed to register shortcut: {}", e);
        }
        log::info!("[voice] shortcut registered, setup continuing");
    }

    // Hide from Dock, show only in menu bar (macOS only)
    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::runtime::{AnyClass, AnyObject};
        unsafe {
            let ns_app_cls = AnyClass::get(c"NSApplication").unwrap();
            let ns_app: *mut AnyObject = msg_send![ns_app_cls, sharedApplication];
            // NSApplicationActivationPolicyAccessory = 1
            let _: () = msg_send![ns_app, setActivationPolicy: 1i64];
        }
    }

    // Set window properties, seed screen/frame info for the hover
    // poll thread, and show.
    #[cfg(target_os = "macos")]
    if let Some(win) = app.get_webview_window("main") {
        let win_clone = win.clone();
        let _ = app.handle().run_on_main_thread(move || {
            use objc2::msg_send;
            use objc2::runtime::AnyObject;
            use objc2_foundation::NSRect;

            if let Ok(ns_win) = win_clone.ns_window() {
                let obj = unsafe { &*(ns_win as *mut AnyObject) };
                unsafe {
                    let _: () = msg_send![obj, setLevel: 27isize];
                    let behavior: usize = (1 << 0) | (1 << 4) | (1 << 8) | (1 << 6);
                    let _: () = msg_send![obj, setCollectionBehavior: behavior];
                    let _: () = msg_send![obj, setAcceptsMouseMovedEvents: true];

                    // Seed NOTCH_SCREEN_INFO + MINI_WINDOW_FRAME so the
                    // efficiency hover/drag poll thread can work from the
                    // first tick (otherwise it silently no-ops until the
                    // panel is toggled via set_mini_expanded).
                    let screen: *mut AnyObject = msg_send![obj, screen];
                    if !screen.is_null() {
                        let sf: NSRect = msg_send![&*screen, frame];
                        let notch_off = get_notch_offset(screen);
                        if let Ok(mut info) = NOTCH_SCREEN_INFO.lock() {
                            *info = Some((
                                sf.origin.x,
                                sf.origin.y,
                                sf.size.width,
                                sf.size.height,
                                notch_off,
                            ));
                        }
                    }
                    let wf: NSRect = msg_send![obj, frame];
                    if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                        *f = Some((wf.origin.x, wf.origin.y, wf.size.width, wf.size.height));
                    }
                }
            }
        });
        let _ = win.show();
    }

    // Windows: position mini window at top-center of primary monitor
    #[cfg(target_os = "windows")]
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_always_on_top(true);
        let _ = win.set_skip_taskbar(true);
        if let Ok(Some(monitor)) = win.primary_monitor() {
            let screen = monitor.size();
            let scale = monitor.scale_factor();
            let sw = screen.width as f64 / scale;
            let x = sw / 2.0 + 40.0;
            let _ = win.set_position(tauri::LogicalPosition::new(x, MASCOT_TOP_INSET));
        }
        let _ = win.show();
    }

    // Windows: move window off-screen when a fullscreen app is on the SAME
    // monitor as the mini window.  We avoid hide()/show() because show()
    // triggers a focus event which causes the panel to expand.
    #[cfg(target_os = "windows")]
    {
        let app_handle = app.handle().clone();
        std::thread::spawn(move || {
            use windows::Win32::Foundation::POINT;
            use windows::Win32::Graphics::Gdi::{
                MonitorFromPoint, HMONITOR, MONITOR_DEFAULTTONEAREST,
            };

            let mut was_hidden = false;
            let mut saved_pos: Option<tauri::LogicalPosition<f64>> = None;
            let mut hidden_monitor: Option<HMONITOR> = None;
            // Debounce counter: require several consecutive non-fullscreen
            // polls before restoring, so brief foreground changes (mouse
            // movement, overlay popups) during video playback don't cause
            // the pet to flicker.
            let mut non_fs_streak: u32 = 0;
            const RESTORE_THRESHOLD: u32 = 4; // 4 × 500ms = 2s
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let fs_monitor = fullscreen_foreground_monitor();

                if let Some(win) = app_handle.get_webview_window("main") {
                    let tracked_monitor = if was_hidden {
                        hidden_monitor
                    } else if let Ok(pos) = win.outer_position() {
                        Some(unsafe {
                            MonitorFromPoint(POINT { x: pos.x, y: pos.y }, MONITOR_DEFAULTTONEAREST)
                        })
                    } else {
                        None
                    };
                    let same_monitor = matches!(
                        (fs_monitor, tracked_monitor),
                        (Some(fs_mon), Some(mini_mon)) if mini_mon == fs_mon
                    );

                    if same_monitor {
                        non_fs_streak = 0;
                        if !was_hidden {
                            log::info!("[fullscreen] detected fullscreen app on same monitor, moving mini off-screen");
                            FULLSCREEN_HIDING.store(true, std::sync::atomic::Ordering::SeqCst);
                            if let Ok(pos) = win.outer_position() {
                                hidden_monitor = Some(unsafe {
                                    MonitorFromPoint(
                                        POINT { x: pos.x, y: pos.y },
                                        MONITOR_DEFAULTTONEAREST,
                                    )
                                });
                            }
                            if let Ok(Some(pos)) = win.outer_position().map(|p| {
                                win.current_monitor().ok().flatten().map(|m| {
                                    let s = m.scale_factor();
                                    tauri::LogicalPosition::new(p.x as f64 / s, p.y as f64 / s)
                                })
                            }) {
                                saved_pos = Some(pos);
                            }
                            let _ = win.set_always_on_top(false);
                            let _ = win.set_position(tauri::LogicalPosition::new(
                                -9999.0_f64,
                                -9999.0_f64,
                            ));
                            was_hidden = true;
                        }
                    } else if was_hidden {
                        non_fs_streak += 1;
                        if non_fs_streak >= RESTORE_THRESHOLD {
                            log::info!("[fullscreen] fullscreen exited or on different monitor, restoring mini position");
                            FULLSCREEN_HIDING.store(false, std::sync::atomic::Ordering::SeqCst);
                            if let Some(pos) = saved_pos.take() {
                                let _ = win.set_position(pos);
                            }
                            let _ = win.set_always_on_top(true);
                            was_hidden = false;
                            hidden_monitor = None;
                            non_fs_streak = 0;
                        }
                    }
                }
            }
        });
    }

    socket::init(app);
    tray::init(app)?;

    Ok(())
}
