//! Miscellaneous Tauri commands.

#[cfg(target_os = "macos")]
use std::sync::atomic::Ordering;

#[cfg(target_os = "macos")]
use tauri::menu::CheckMenuItem;
use tauri::menu::{Menu, MenuItem};

#[cfg(target_os = "macos")]
use crate::state::STROLL_MODE_ENABLED;

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
        let stroll = CheckMenuItem::with_id(
            &app,
            "stroll",
            stroll_label,
            true,
            STROLL_MODE_ENABLED.load(Ordering::SeqCst),
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
        use std::ffi::c_void;

        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFStringCreateWithCString(
                alloc: *const c_void,
                c_str: *const u8,
                encoding: u32,
            ) -> *const c_void;
            fn CFDictionaryCreate(
                alloc: *const c_void,
                keys: *const *const c_void,
                values: *const *const c_void,
                count: isize,
                key_cbs: *const c_void,
                val_cbs: *const c_void,
            ) -> *const c_void;
            fn CFRelease(cf: *const c_void);
            static kCFTypeDictionaryKeyCallBacks: c_void;
            static kCFTypeDictionaryValueCallBacks: c_void;
            static kCFBooleanTrue: *const c_void;
        }

        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
        }

        unsafe {
            let key = CFStringCreateWithCString(
                std::ptr::null(),
                b"AXTrustedCheckOptionPrompt\0".as_ptr(),
                0x08000100, // kCFStringEncodingUTF8
            );
            let keys = [key];
            let values = [kCFBooleanTrue];
            let dict = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                1,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            );
            AXIsProcessTrustedWithOptions(dict);
            CFRelease(dict);
            CFRelease(key);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn voice_toggle() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if crate::speech::is_recording() {
            crate::speech::stop_recording()
        } else {
            crate::speech::start_recording()
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Voice input not supported on this platform".into())
    }
}

#[tauri::command]
pub fn voice_is_recording() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::speech::is_recording()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}
