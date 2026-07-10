//! Window lifecycle commands: open_mini, close_mini, open_detail_panel, reassert_floating, get_ui_scale.

use std::sync::Arc;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

#[cfg(target_os = "macos")]
use crate::mascot::MASCOT_TOP_INSET;
use crate::mascot::{
    collapsed_mascot_window_size, COLLAPSED_MASCOT_BASE_H, COLLAPSED_MASCOT_BASE_W,
};
use crate::pet_core::reassert_mini_floating;
use crate::state::WindowState;

#[cfg(target_os = "macos")]
use crate::platform::macos::get_notch_offset;

#[cfg(target_os = "windows")]
use crate::platform::windows::win_ui_scale;

#[tauri::command]
pub async fn open_mini(app: tauri::AppHandle) -> Result<(), String> {
    log::info!("[mini-pos] open_mini called");
    let ws = app.state::<Arc<WindowState>>();
    if let Some(win) = app.get_webview_window("main") {
        // Reposition to collapsed position before showing
        #[cfg(target_os = "macos")]
        {
            let win_clone = win.clone();
            let cached_frame_size = ws
                .mini_frame
                .lock()
                .ok()
                .and_then(|g| *g)
                .map(|(_, _, w, h)| (w, h))
                .unwrap_or_else(|| collapsed_mascot_window_size(1.0));
            let _ = app.run_on_main_thread(move || {
                use objc2::runtime::{AnyClass, AnyObject};
                use objc2::msg_send;
                use objc2_foundation::{NSRect, NSPoint, NSSize};

                if let Ok(ns_win) = win_clone.ns_window() {
                    let obj = unsafe { &*(ns_win as *mut AnyObject) };
                    unsafe {
                        let _: () = msg_send![obj, setLevel: 27isize];
                        let behavior: usize = (1 << 0) | (1 << 4) | (1 << 8) | (1 << 6);
                        let _: () = msg_send![obj, setCollectionBehavior: behavior];
                    }
                    let screen_info: Option<(f64, f64, f64, f64, f64)> = unsafe {
                        let cls = match AnyClass::get(c"NSScreen") {
                            Some(c) => c,
                            None => return,
                        };
                        let screens: *mut AnyObject = msg_send![cls, screens];
                        if screens.is_null() { return; }
                        let count: usize = msg_send![&*screens, count];
                        if count == 0 { return; }
                        let screen: *mut AnyObject = msg_send![&*screens, objectAtIndex: 0usize];
                        if screen.is_null() { return; }
                        let frame: NSRect = msg_send![&*screen, frame];
                        let notch_off = get_notch_offset(screen);
                        Some((frame.origin.x, frame.origin.y, frame.size.width, frame.size.height, notch_off))
                    };
                    if let Some((sx, sy, sw, sh, notch_off)) = screen_info {
                        let (win_w, win_h) = cached_frame_size;
                        let x = sx + sw / 2.0 + notch_off;
                        // Pull the window down by MASCOT_TOP_INSET so it
                        // does not sit under the menu bar / notch on launch.
                        let y = sy + sh - win_h - MASCOT_TOP_INSET;
                        log::info!(
                            "[mini-pos] open_mini(existing,mac) target frame x={:.1} y={:.1} w={:.1} h={:.1} inset={:.1} screen=({:.1},{:.1},{:.1},{:.1}) notch_off={:.1}",
                            x, y, win_w, win_h, MASCOT_TOP_INSET, sx, sy, sw, sh, notch_off
                        );
                        let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(win_w, win_h));
                        unsafe {
                            let _: () = msg_send![obj, setFrame: frame, display: true];
                            let _: () = msg_send![obj, orderFrontRegardless];
                        }
                    }
                }
            });
        }
        #[cfg(target_os = "windows")]
        {
            // Reposition to top-center (simulating macOS notch position), DPI-aware
            if let Ok(Some(monitor)) = win.primary_monitor() {
                let scale = monitor.scale_factor();
                let sw = monitor.size().width as f64 / scale;
                let ui = win_ui_scale(&monitor);
                let (base_w, base_h) = ws
                    .mini_frame
                    .lock()
                    .ok()
                    .and_then(|g| *g)
                    .map(|(_, _, w, h)| (w, h))
                    .unwrap_or_else(|| collapsed_mascot_window_size(1.0));
                let win_w = (base_w * ui).round();
                let win_h = (base_h * ui).round();
                let notch_off = (80.0 * ui).round();
                let x = sw / 2.0 + notch_off;
                let _ = win.set_size(tauri::LogicalSize::new(win_w, win_h));
                log::info!(
                    "[mini-pos] open_mini(existing,win) target pos x={:.1} y={:.1} w={:.1} h={:.1} ui={:.2} notch_off={:.1}",
                    x, 0.0, win_w, win_h, ui, notch_off
                );
                let _ = win.set_position(tauri::LogicalPosition::new(x, 0.0));
            }
            if !ws
                .fullscreen_hiding
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                win.show().map_err(|e| e.to_string())?;
                win.set_focus().map_err(|e| e.to_string())?;
            }
        }
        return Ok(());
    }

    let builder = WebviewWindowBuilder::new(&app, "main", WebviewUrl::App("index.html".into()))
        .title("PawBae")
        .inner_size(COLLAPSED_MASCOT_BASE_W, COLLAPSED_MASCOT_BASE_H)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .accept_first_mouse(true); // single click from any app

    let win = builder.build().map_err(|e| e.to_string())?;

    // Use macOS native API to position at menu bar level (like notchi)
    // Must run on main thread for AppKit calls
    #[cfg(target_os = "macos")]
    {
        let win_clone = win.clone();
        let _ = app.run_on_main_thread(move || {
            use objc2::runtime::{AnyClass, AnyObject};
            use objc2::msg_send;
            use objc2_foundation::{NSRect, NSPoint, NSSize};

            if let Ok(ns_win) = win_clone.ns_window() {
                let obj = unsafe { &*(ns_win as *mut AnyObject) };

                unsafe {
                    let _: () = msg_send![obj, setLevel: 27isize];
                    let behavior: usize = (1 << 0) | (1 << 4) | (1 << 8) | (1 << 6);
                    let _: () = msg_send![obj, setCollectionBehavior: behavior];
                }

                let screen_info: Option<(f64, f64, f64, f64, f64)> = unsafe {
                    let cls = match AnyClass::get(c"NSScreen") {
                        Some(c) => c,
                        None => return,
                    };
                    let screens: *mut AnyObject = msg_send![cls, screens];
                    if screens.is_null() { return; }
                    let count: usize = msg_send![&*screens, count];
                    if count == 0 { return; }
                    let screen: *mut AnyObject = msg_send![&*screens, objectAtIndex: 0usize];
                    if screen.is_null() { return; }
                    let frame: NSRect = msg_send![&*screen, frame];
                    let notch_off = get_notch_offset(screen);
                    Some((frame.origin.x, frame.origin.y, frame.size.width, frame.size.height, notch_off))
                };

                if let Some((sx, sy, sw, sh, notch_off)) = screen_info {
                    let (win_w, win_h) = collapsed_mascot_window_size(1.0);
                    let x = sx + sw / 2.0 + notch_off;
                    // Pull the window down by MASCOT_TOP_INSET so the sprite
                    // is fully visible below the menu bar / notch on launch.
                    let y = sy + sh - win_h - MASCOT_TOP_INSET;
                    log::info!(
                        "[mini-pos] open_mini(new,mac) target frame x={:.1} y={:.1} w={:.1} h={:.1} inset={:.1} screen=({:.1},{:.1},{:.1},{:.1}) notch_off={:.1}",
                        x, y, win_w, win_h, MASCOT_TOP_INSET, sx, sy, sw, sh, notch_off
                    );
                    let frame = NSRect::new(
                        NSPoint::new(x, y),
                        NSSize::new(win_w, win_h),
                    );
                    unsafe {
                        let _: () = msg_send![obj, setFrame: frame, display: true];
                    }
                }

                unsafe {
                    let _: () = msg_send![obj, orderFrontRegardless];
                }
            }
        });
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows: position at top-center (simulating macOS notch position), DPI-aware
        if let Ok(Some(monitor)) = win.primary_monitor() {
            let scale = monitor.scale_factor();
            let sw = monitor.size().width as f64 / scale;
            let ui = win_ui_scale(&monitor);
            let (base_w, base_h) = collapsed_mascot_window_size(1.0);
            let win_w = (base_w * ui).round();
            let win_h = (base_h * ui).round();
            let notch_off = (80.0 * ui).round();
            let x = sw / 2.0 + notch_off;
            let _ = win.set_size(tauri::LogicalSize::new(win_w, win_h));
            log::info!(
                "[mini-pos] open_mini(new,win) target pos x={:.1} y={:.1} w={:.1} h={:.1} ui={:.2} notch_off={:.1}",
                x, 0.0, win_w, win_h, ui, notch_off
            );
            let _ = win.set_position(tauri::LogicalPosition::new(x, 0.0));
        }
        if !ws
            .fullscreen_hiding
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            let _ = win.show();
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn close_mini(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        win.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
#[allow(unused_variables)]
pub async fn get_ui_scale(app: tauri::AppHandle) -> Result<f64, String> {
    #[cfg(target_os = "windows")]
    {
        let win = app.get_webview_window("main").ok_or("mini not found")?;
        if let Ok(Some(m)) = win.current_monitor() {
            return Ok(win_ui_scale(&m));
        }
    }
    Ok(1.0)
}

#[tauri::command]
pub async fn open_detail_panel(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("detail") {
        win.show().map_err(|e| e.to_string())?;
        win.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let win =
        WebviewWindowBuilder::new(&app, "detail", WebviewUrl::App("index.html#/detail".into()))
            .title("PawBae - Detail")
            .inner_size(480.0, 600.0)
            .decorations(true)
            .resizable(true)
            .center()
            .build()
            .map_err(|e| e.to_string())?;
    let _ = win.maximize();

    Ok(())
}

#[tauri::command]
pub async fn reassert_floating(app: tauri::AppHandle) -> Result<(), String> {
    reassert_mini_floating(&app);
    Ok(())
}

/// Open the OBS stage window: a borderless, non-topmost mirror the streamer
/// window-captures and chroma-keys into their scene. Deliberately NOT
/// always-on-top — OBS (ScreenCaptureKit / WGC) captures fully occluded
/// windows, so on a single monitor the stage can sit buried behind the IDE.
/// It never takes focus: the pet must not interrupt a live stream.
#[tauri::command]
pub async fn open_stage_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("stage") {
        win.show().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Plain index.html — the frontend routes to StageApp by window LABEL (the
    // dev server drops URL fragments, so a #/stage hash never arrives in dev).
    WebviewWindowBuilder::new(&app, "stage", WebviewUrl::App("index.html".into()))
        .title("PawBae Stage")
        .inner_size(480.0, 270.0)
        .min_inner_size(160.0, 120.0)
        .resizable(true)
        .decorations(false)
        .shadow(false)
        .always_on_top(false)
        .skip_taskbar(false)
        .focused(false)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn close_stage_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("stage") {
        win.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}
