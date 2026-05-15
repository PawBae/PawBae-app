//! System tray menu and on_menu_event handler.

#[cfg(target_os = "macos")]
use std::sync::atomic::Ordering;

#[cfg(target_os = "macos")]
use tauri::menu::CheckMenuItem;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager};

#[cfg(target_os = "macos")]
use crate::state::{STROLL_MODE_ENABLED, THROW_TRACKING_ENABLED};

#[cfg(target_os = "windows")]
use crate::state::FULLSCREEN_HIDING;
#[cfg(target_os = "windows")]
use crate::platform::windows::win_ui_scale;

// Tray label tuple: (show, hide, stroll, settings, quit). The `stroll` slot is
// populated for every language but only inserted into the tray menu on
// macOS — Phase 2 pet physics is currently macOS-only.
pub(crate) fn tray_labels(lang: &str) -> (&'static str, &'static str, &'static str, &'static str, &'static str) {
    match lang {
        "zh" => ("显示", "隐藏", "散步模式", "设置", "退出"),
        _ => ("Show", "Hide", "Stroll Mode", "Settings", "Quit"),
    }
}

pub(crate) fn init<R: tauri::Runtime>(app: &mut tauri::App<R>) -> tauri::Result<()> {
    // System tray — use saved language, fallback to system language
    let initial_lang = {
        let store_path = app.path().app_data_dir().ok().map(|p| p.join("settings.json"));
        let mut lang = None;
        if let Some(ref sp) = store_path {
            if let Ok(data) = std::fs::read_to_string(sp) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&data) {
                    lang = val.get("pawbae-lang").and_then(|v| v.as_str()).map(|s| s.to_string());
                }
            }
        }
        lang.unwrap_or_else(|| {
            let sys = std::env::var("LANG").unwrap_or_default().to_lowercase();
            if sys.starts_with("zh") { "zh".into() }
            else { "en".into() }
        })
    };
    let (show_label, hide_label, stroll_label, settings_label, quit_label) = tray_labels(&initial_lang);
    let _ = stroll_label;
    let show = MenuItem::with_id(app, "show", show_label, true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", hide_label, true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", settings_label, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit_label, true, None::<&str>)?;
    #[cfg(target_os = "macos")]
    let menu = {
        let stroll = CheckMenuItem::with_id(
            app,
            "stroll",
            stroll_label,
            true,
            STROLL_MODE_ENABLED.load(Ordering::SeqCst),
            None::<&str>,
        )?;
        Menu::with_items(app, &[&show, &hide, &stroll, &settings, &quit])?
    };
    #[cfg(not(target_os = "macos"))]
    let menu = Menu::with_items(app, &[&show, &hide, &settings, &quit])?;

    // Use dedicated tray icon (logo-mini: white cat silhouette on transparent bg)
    // instead of the app icon, so it renders correctly in macOS menu bar / Windows tray
    let tray_icon_bytes = include_bytes!("../icons/tray-icon.png");
    let tray_icon = tauri::image::Image::from_bytes(tray_icon_bytes)
        .expect("failed to load tray icon");
    TrayIconBuilder::with_id("main")
        .icon(tray_icon)
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(win) = app.get_webview_window("main") {
                    #[cfg(target_os = "windows")]
                    {
                        FULLSCREEN_HIDING.store(false, std::sync::atomic::Ordering::SeqCst);
                        if let Ok(Some(monitor)) = win.primary_monitor() {
                            let scale = monitor.scale_factor();
                            let sw = monitor.size().width as f64 / scale;
                            let ui = win_ui_scale(&monitor);
                            let x = sw / 2.0 + (80.0 * ui).round();
                            let _ = win.set_position(tauri::LogicalPosition::new(x, 0.0));
                        }
                        let _ = win.set_always_on_top(true);
                    }
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            "hide" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.hide();
                }
            }
            #[cfg(target_os = "macos")]
            "stroll" => {
                // Toggle the global stroll-mode flag, persist it
                // through the frontend (which owns settings.json),
                // and broadcast the new value so Mini.tsx can flip
                // the physics loop on/off without a polling read.
                let prev = STROLL_MODE_ENABLED.load(Ordering::SeqCst);
                let next = !prev;
                STROLL_MODE_ENABLED.store(next, Ordering::SeqCst);
                // If the user disables stroll, also drop throw
                // tracking so we stop sampling drag velocities.
                if !next {
                    THROW_TRACKING_ENABLED.store(false, Ordering::SeqCst);
                }
                let _ = app.emit("stroll-mode-changed", next);
            }
            "settings" => {
                if let Some(win) = app.get_webview_window("main") {
                    #[cfg(target_os = "windows")]
                    {
                        FULLSCREEN_HIDING.store(false, std::sync::atomic::Ordering::SeqCst);
                        if let Ok(Some(monitor)) = win.primary_monitor() {
                            let scale = monitor.scale_factor();
                            let sw = monitor.size().width as f64 / scale;
                            let ui = win_ui_scale(&monitor);
                            let x = sw / 2.0 + (80.0 * ui).round();
                            let _ = win.set_position(tauri::LogicalPosition::new(x, 0.0));
                        }
                        let _ = win.set_always_on_top(true);
                    }
                    let _ = win.show();
                    let _ = win.set_focus();
                }
                let _ = app.emit("tray-open-settings", ());
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
