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

#[cfg(target_os = "macos")]
mod speech;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    setup::init_webview2_env();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build());
    asset::register(builder)
        .setup(setup::init)
        .invoke_handler(tauri::generate_handler![
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
            reassert_floating,
            spawn_demo_mascot,
            close_demo_mascot,
            close_demo_mascots,
            debug_log,
            update_tray_language,
            set_pet_mode_window,
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
            voice_set_locale
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
