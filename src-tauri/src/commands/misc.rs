//! Miscellaneous Tauri commands.

#[cfg(target_os = "macos")]
use std::sync::atomic::Ordering;
#[cfg(target_os = "macos")]
use std::sync::Arc;

#[cfg(target_os = "macos")]
use tauri::menu::CheckMenuItem;
use tauri::menu::{Menu, MenuItem};
#[cfg(target_os = "macos")]
use tauri::Manager;

#[cfg(target_os = "macos")]
use crate::state::PetState;

#[tauri::command]
pub fn update_tray_language(app: tauri::AppHandle, lang: String) -> Result<(), String> {
    let (show_label, hide_label, stroll_label, settings_label, quit_label) =
        crate::tray::tray_labels(&lang);
    let _ = stroll_label;
    let show = MenuItem::with_id(&app, "show", show_label, true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let hide = MenuItem::with_id(&app, "hide", hide_label, true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let settings = MenuItem::with_id(&app, "settings", settings_label, true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let quit = MenuItem::with_id(&app, "quit", quit_label, true, None::<&str>)
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    let menu = {
        let ps = app.state::<Arc<PetState>>();
        let stroll = CheckMenuItem::with_id(
            &app,
            "stroll",
            stroll_label,
            true,
            ps.stroll_enabled.load(Ordering::SeqCst),
            None::<&str>,
        )
        .map_err(|e| e.to_string())?;
        Menu::with_items(&app, &[&show, &hide, &stroll, &settings, &quit])
            .map_err(|e| e.to_string())?
    };
    #[cfg(not(target_os = "macos"))]
    let menu =
        Menu::with_items(&app, &[&show, &hide, &settings, &quit]).map_err(|e| e.to_string())?;
    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// Proxy a POST request to bypass CORS restrictions in the webview.
#[tauri::command]
pub async fn proxy_post(url: String, body: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("request failed: {}", e))?;
    let status = resp.status().as_u16();
    let text = resp.text().await.map_err(|e| format!("read body: {}", e))?;
    if status >= 400 {
        return Err(format!("HTTP {}: {}", status, text));
    }
    Ok(text)
}

#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        // `cmd /C start ""` opens the URL in the default browser, but cmd
        // itself is a console app so without CREATE_NO_WINDOW the user
        // sees a black console flash next to the freshly opened browser
        // tab. Hide it.
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/C", "start", "", &url]);
        crate::platform::windows::hide_window_cmd(&mut cmd);
        cmd.spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Forward a frontend diagnostic line to the dev terminal so debugging
/// modal/blur/exit paths doesn't require opening webview DevTools.
#[tauri::command]
pub async fn debug_log(scope: String, msg: String) -> Result<(), String> {
    log::info!("[fe:{}] {}", scope, msg);
    Ok(())
}

/// Activate a macOS app by its name (e.g. "Feishu", "Telegram", "Lark").
#[tauri::command]
#[allow(unused_variables)]
pub async fn activate_app(app_name: String) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let script = format!(r#"tell application "{}" to activate"#, app_name);
        std::process::Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|e| e.to_string())?;
        Ok(format!("Activated {}", app_name))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("activate_app not supported on this platform".to_string())
    }
}

#[tauri::command]
pub async fn check_ax_permission() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        Ok(crate::platform::macos::check_accessibility_permission())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(crate::platform::common::check_accessibility_permission())
    }
}

#[tauri::command]
pub async fn request_ax_permission() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        crate::platform::macos::request_accessibility_permission();
    }
    Ok(())
}

#[tauri::command]
pub async fn voice_toggle() -> Result<(), String> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        if crate::speech::is_recording() {
            crate::speech::stop_recording()
        } else {
            crate::speech::start_recording()
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err("Voice input not supported on this platform".into())
    }
}

#[tauri::command]
pub fn voice_is_recording() -> bool {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        crate::speech::is_recording()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

/// Master on/off for voice interaction (Settings → Privacy). When off, the shortcut opens
/// no microphone at all. Synced from the frontend setting.
#[tauri::command]
pub fn voice_set_enabled(enabled: bool) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        crate::speech::set_voice_enabled(enabled);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = enabled;
    }
}

/// Set the speech-recognition locale (e.g. "zh-CN"). "auto" resolves to the default single
/// recognizer (Chinese) in the speech module.
#[tauri::command]
pub fn voice_set_locale(locale: String) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        crate::speech::set_voice_locale(locale);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = locale;
    }
}
