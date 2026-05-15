use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod state;
use crate::state::{ActiveAgentPid, ClaudeState};

mod platform;

mod commands;
use crate::commands::*;

mod asset;

mod cursor;
mod tray;

use crate::cursor::{focus_cursor_terminal, jump_to_claude_terminal};

mod session_watcher;

mod socket;
use crate::socket::resolve_claude_permission;

mod ssh;
pub(crate) use crate::ssh::{
    close_ssh_master, ssh_backoff_reset, ssh_exec, ssh_is_agent_active, ssh_read_file,
};

mod lsof;
pub(crate) use crate::lsof::{lsof_active_agents, lsof_open_jsonl_paths};

mod agent;
pub(crate) use crate::agent::{
    build_agent_health_from_meta, check_agent_active_from_lines, extract_sessions, invoke_tool,
    is_remote_session_active, is_session_active, remote_sessions_json_path, sessions_json_path,
};

mod mascot;
pub(crate) use crate::mascot::{
    collapsed_mascot_window_size, large_collapsed_mascot_window_size, sanitized_mascot_scale,
    COLLAPSED_MASCOT_BASE_H, COLLAPSED_MASCOT_BASE_W, LARGE_MASCOT_SIZE_MULTIPLIER,
    MASCOT_TOP_INSET,
};
#[cfg(target_os = "macos")]
pub(crate) use crate::mascot::{collapsed_x, current_sprite_pad};

mod pet;
pub(crate) use crate::pet::{
    efficiency_hover_poll, is_codex_internal_utility_session, reassert_mini_floating,
};

mod jsonl_paths;
pub(crate) use crate::jsonl_paths::{
    collect_claude_project_jsonl_files, collect_codex_session_jsonl_files,
    resolve_session_jsonl_path,
};

mod terminal;
pub(crate) use crate::terminal::{
    frontmost_matches_host_terminal, is_codex_frontmost_app, is_codex_host_terminal,
    is_cursor_frontmost_app, is_pid_alive,
};
#[cfg(not(target_os = "macos"))]
pub(crate) use crate::terminal::{get_active_ghostty_terminal_id, get_frontmost_app_name};

mod app_init;
pub(crate) use crate::app_init::home_dir_string;

mod setup;

#[cfg(target_os = "macos")]
mod speech;

#[cfg(unix)]
use libc;

#[cfg(target_os = "macos")]
pub(crate) use crate::platform::macos::check_accessibility_permission;

#[cfg(not(target_os = "macos"))]
pub(crate) fn check_accessibility_permission() -> bool {
    true
}

// Re-export Windows helpers that command modules reach via `crate::*`.
#[cfg(target_os = "windows")]
pub(crate) use crate::platform::windows::hide_window_cmd;

// Re-export macOS-only helpers that command modules reach via `crate::*`.
#[cfg(target_os = "macos")]
pub(crate) use crate::platform::macos::{
    activate_cursor_workspace_window, compute_frontmost_app_window_macos,
    find_terminal_app_for_pid, frontmost_app_window_cache, get_active_ghostty_terminal_id,
    get_frontmost_app_name, get_notch_offset, get_tty_for_pid, pet_context_schedule_restore_alpha,
    pet_passthrough_poll,
};

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
            voice_toggle,
            voice_is_recording
        ])
        .manage(ActiveAgentPid {
            pid: Mutex::new(None),
        })
        .manage(ClaudeState {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
