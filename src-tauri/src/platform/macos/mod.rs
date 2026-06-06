//! macOS-specific helpers. Gated by the outer `#[cfg(target_os = "macos")]` in `platform/mod.rs`.

mod drag;
mod interaction;
mod media;
mod screen;
mod terminal;

// Re-export all public items so external callers see a flat `platform::macos::*` namespace.

// screen.rs
pub(crate) use screen::compute_frontmost_app_window_macos;
pub(crate) use screen::frontmost_app_window_cache;
pub(crate) use screen::get_notch_offset;

// drag.rs
pub(crate) use drag::macos_cursor_position;
pub(crate) use drag::macos_pressed_mouse_buttons;
pub(crate) use drag::request_drag_apply;

// media.rs
pub(crate) use media::get_frontmost_bundle_id;
pub(crate) use media::is_any_music_app_playing;
pub(crate) use media::is_browser;
pub(crate) use media::is_music_app;
pub(crate) use media::is_video_app;
pub(crate) use media::nowplaying_cli_status;

// interaction.rs
pub(crate) use interaction::pet_context_schedule_restore_alpha;
pub(crate) use interaction::pet_passthrough_poll;

// terminal.rs
pub(crate) use terminal::activate_cursor_workspace_window;
pub(crate) use terminal::check_accessibility_permission;
pub(crate) use terminal::find_terminal_app_for_pid;
pub(crate) use terminal::get_active_ghostty_terminal_id;
pub(crate) use terminal::get_frontmost_app_name;
pub(crate) use terminal::get_tty_for_pid;
pub(crate) use terminal::install_wry_webview_ime_fix;
pub(crate) use terminal::request_accessibility_permission;
