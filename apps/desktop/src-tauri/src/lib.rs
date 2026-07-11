// `LazyLock` (stable in 1.80) is used in `state.rs` for the session-watcher
// registry. `rust-version` in Cargo.toml stays at 1.77.2 for downstream
// compatibility; this allow skips the lint clippy raises for that gap.
#![allow(clippy::incompatible_msrv)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod state;
use crate::state::{ActiveAgentPid, ClaudeState, InputState, PetState, SshState, WindowState};

mod platform;

mod commands;
use crate::commands::*;

mod input;

mod asset;

mod cursor;
mod tray;

use crate::cursor::{focus_cursor_terminal, jump_to_claude_terminal};

mod session_watcher;

mod socket;
use crate::socket::resolve_claude_permission;

mod agent_gateway;
mod app_init;
mod jsonl_paths;
mod lsof;
mod mascot;
mod pet_core;
mod setup;
mod ssh_core;
mod terminal;

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod speech;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    setup::init_webview2_env();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build());
    // Anonymous opt-in telemetry (Aptabase). The app key is baked in at compile
    // time; keyless builds (local dev) skip registration and the frontend track()
    // wrapper swallows the missing-plugin error. The plugin never auto-sends —
    // every event goes through the opt-in gate in utils/telemetry.ts.
    let builder = match option_env!("APTABASE_APP_KEY") {
        Some(key) if !key.is_empty() => {
            builder.plugin(tauri_plugin_aptabase::Builder::new(key).build())
        }
        _ => builder,
    };
    asset::register(builder)
        .setup(setup::init)
        .on_window_event(|window, event| {
            // The stage window is borderless (no close button of ours); when the
            // OS closes it anyway (Cmd+W, app quit ordering, crash recovery),
            // the settings toggle must fall back in sync in the main webview.
            if window.label() == "stage" {
                if let tauri::WindowEvent::Destroyed = event {
                    use tauri::{Emitter, Manager};
                    // Emitting inside the window-event callback deadlocks the
                    // main thread (the emit dispatches into webviews while the
                    // destroy is still unwinding) — hop off the callback first.
                    let app = window.app_handle().clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = app.emit("stage-closed", ());
                    });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            await_oauth_callback,
            report_frontend_error,
            take_unseen_crashes,
            get_status,
            send_chat,
            open_detail_panel,
            get_agents,
            get_health,
            get_agent_metrics,
            interrupt_agent,
            get_agent_extra_info,
            open_mini,
            close_mini,
            set_mini_expanded,
            set_mini_size,
            set_efficiency_hover_tracking,
            resize_mini_height,
            move_mini_by,
            get_mini_origin,
            get_mini_monitor_rect,
            get_pet_floor_info,
            get_frontmost_app_window,
            set_sprite_pad_fractions,
            set_mini_origin,
            set_ime_mode,
            get_agent_sessions,
            get_session_preview,
            get_session_messages,
            get_active_sessions,
            proxy_post,
            play_sound,
            get_claude_sessions,
            get_claude_conversation,
            install_claude_hooks,
            install_cursor_hooks,
            remove_claude_session,
            resolve_claude_permission,
            get_claude_stats,
            open_url,
            activate_app,
            focus_cursor_terminal,
            check_ax_permission,
            request_ax_permission,
            jump_to_claude_terminal,
            check_for_update,
            run_update,
            close_ssh,
            read_local_file,
            exit_app,
            get_ssh_key_info,
            reset_ssh,
            get_ui_scale,
            list_custom_codex_pets,
            open_codex_pets_dir,
            import_codex_pet,
            pick_codex_pet_folder,
            pick_skin_image,
            import_skin_image,
            commit_staged_skin,
            discard_staged_skin,
            remove_custom_skin,
            reassert_floating,
            open_stage_window,
            close_stage_window,
            spawn_demo_mascot,
            close_demo_mascot,
            close_demo_mascots,
            debug_log,
            update_tray_language,
            set_pet_mode_window,
            set_pet_passthrough,
            set_note_hitbox,
            set_pet_context_menu,
            set_pet_pomodoro_active,
            get_now_playing,
            get_system_idle_time,
            set_stroll_mode,
            set_throw_tracking,
            set_input_tracking,
            get_input_tracking_status,
            voice_toggle,
            voice_is_recording,
            voice_set_locale,
            voice_set_enabled,
            save_png_file
        ])
        .manage(ActiveAgentPid {
            pid: Mutex::new(None),
        })
        .manage(ClaudeState {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
        })
        .manage(Arc::new(WindowState::new()))
        .manage(Arc::new(PetState::new()))
        .manage(Arc::new(SshState::new()))
        .manage(Arc::new(InputState::new()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
