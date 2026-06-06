//! Terminal and workspace helpers: Ghostty, Cursor activation, IME fix, AX, TTY.

/// Get the terminal ID of Ghostty's currently focused tab, if Ghostty is frontmost.
/// Returns None if Ghostty is not running or not frontmost.
pub(crate) fn get_active_ghostty_terminal_id() -> Option<String> {
    let script = r#"
        if not (application "Ghostty" is running) then return ""
        tell application "System Events"
            set fp to name of first application process whose frontmost is true
        end tell
        if fp is not "Ghostty" then return ""
        tell application "Ghostty"
            try
                return id of first terminal of selected tab of front window as text
            end try
        end tell
        return ""
    "#;
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;
    let tid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if tid.is_empty() {
        None
    } else {
        Some(tid)
    }
}
/// Returns the short name of the frontmost application (macOS only).
/// Used to suppress completion popups when the user is already looking
/// at the relevant app (Cursor, Codex, etc.).
pub(crate) fn get_frontmost_app_name() -> String {
    let script = r#"
        set appName to short name of (info for (path to frontmost application))
        return appName
    "#;
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => String::new(),
    }
}
pub(crate) fn check_accessibility_permission() -> bool {
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    unsafe { AXIsProcessTrusted() }
}
/// Prompt the user for Accessibility access via the system dialog that
/// deep-links to System Settings → Privacy & Security → Accessibility. macOS
/// shows the prompt at most once per app launch, so repeated calls are safe.
/// No visible effect if already trusted.
pub(crate) fn request_accessibility_permission() {
    use std::ffi::{c_char, c_void};

    #[allow(clashing_extern_declarations)]
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const c_char,
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
            c"AXTrustedCheckOptionPrompt".as_ptr(),
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
pub(crate) fn activate_cursor_workspace_window(workspace_name: &str) {
    let ax_ok = check_accessibility_permission();
    let escaped_workspace = workspace_name.replace('\\', "\\\\").replace('"', "\\\"");

    let script = if escaped_workspace.is_empty() {
        r#"tell application "Cursor" to activate"#.to_string()
    } else if ax_ok {
        format!(
            r#"tell application "System Events"
    set cursorProcs to every process whose name is "Cursor"
    if (count of cursorProcs) is 0 then
        tell application "Cursor" to activate
        return
    end if
    set cursorProc to item 1 of cursorProcs
    set matched to false
    repeat with w in windows of cursorProc
        try
            if name of w contains "{workspace}" then
                perform action "AXRaise" of w
                set frontmost of cursorProc to true
                set matched to true
                exit repeat
            end if
        end try
    end repeat
    if not matched then
        set frontmost of cursorProc to true
    end if
end tell"#,
            workspace = escaped_workspace,
        )
    } else {
        // No AX permission — use Cursor's own AppleScript dictionary
        // to find and raise the matching window by index, which does
        // not require System Events / Accessibility permission.
        format!(
            r#"tell application "Cursor"
    activate
    set matched to false
    repeat with i from 1 to count of windows
        if name of window i contains "{workspace}" then
            set index of window i to 1
            set matched to true
            exit repeat
        end if
    end repeat
end tell"#,
            workspace = escaped_workspace,
        )
    };

    let _ = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output();
}
/// Walk the parent process chain to find the terminal app name.
/// Returns the process name of the first recognized terminal emulator.
pub(crate) fn find_terminal_app_for_pid(pid: u32) -> Option<String> {
    let known_terminals = [
        "Ghostty",
        "ghostty",
        "iTerm2",
        "iterm2",
        "Terminal",
        "Apple_Terminal",
        "WezTerm",
        "wezterm-gui",
        "Warp",
        "warp",
        "kitty",
        "Alacritty",
        "alacritty",
        "kaku",
        "Cursor",
        "Codex",
        "codex",
    ];

    let mut current_pid = pid;
    for _ in 0..20 {
        let output = std::process::Command::new("ps")
            .args(["-p", &current_pid.to_string(), "-o", "ppid=,comm="])
            .output()
            .ok()?;
        let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if line.is_empty() {
            return None;
        }

        let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
        if parts.len() < 2 {
            return None;
        }

        let ppid: u32 = parts[0].trim().parse().ok()?;
        let comm = parts[1].trim();
        // Extract basename from full path
        let name = comm.rsplit('/').next().unwrap_or(comm);

        if known_terminals.iter().any(|t| name.eq_ignore_ascii_case(t)) {
            return Some(name.to_string());
        }

        if ppid <= 1 {
            return None;
        }
        current_pid = ppid;
    }
    None
}
/// Get the TTY device path for a given PID.
pub(crate) fn get_tty_for_pid(pid: u32) -> Option<String> {
    let output = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "tty="])
        .output()
        .ok()?;
    let tty = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if tty.is_empty() || tty == "??" {
        return None;
    }
    // Normalize: ps outputs like "ttys003", convert to "/dev/ttys003"
    if tty.starts_with("/dev/") {
        Some(tty)
    } else {
        Some(format!("/dev/{}", tty))
    }
}
pub(crate) fn install_wry_webview_ime_fix() {
    use std::ffi::CString;
    use std::sync::Once;

    use objc2::ffi;
    use objc2::runtime::{AnyClass, AnyObject, AnyProtocol, Imp, Sel};
    use objc2::{msg_send, sel};

    static INSTALL_ONCE: Once = Once::new();

    unsafe extern "C-unwind" fn window_level(this: &AnyObject, _cmd: Sel) -> isize {
        let window: *mut AnyObject = unsafe { msg_send![this, window] };
        if window.is_null() {
            0
        } else {
            unsafe { msg_send![&*window, level] }
        }
    }

    // Always accept the first mouse event. By default NSView returns NO,
    // which means the first click on an inactive floating window only
    // activates the app — pointerdown is never delivered to the webview,
    // breaking direct drag on the mini mascot. Returning YES delivers
    // every click to the view immediately.
    unsafe extern "C-unwind" fn accepts_first_mouse(
        _this: &AnyObject,
        _cmd: Sel,
        _event: *mut AnyObject,
    ) -> bool {
        true
    }

    fn patch_class(
        class_name: &'static std::ffi::CStr,
        text_input_protocol: Option<&'static AnyProtocol>,
    ) {
        let Some(cls) = AnyClass::get(class_name) else {
            log::warn!("[ime] class not found: {}", class_name.to_string_lossy());
            return;
        };

        let cls_ptr = cls as *const AnyClass as *mut AnyClass;
        let level_encoding = CString::new("q@:").unwrap();
        let bool_arg_encoding = CString::new("c@:@").unwrap();
        unsafe {
            if let Some(protocol) = text_input_protocol {
                let _ = ffi::class_addProtocol(cls_ptr, protocol);
            }
            let _ = ffi::class_addMethod(
                cls_ptr,
                sel!(windowLevel),
                std::mem::transmute::<unsafe extern "C-unwind" fn(&AnyObject, Sel) -> isize, Imp>(
                    window_level,
                ),
                level_encoding.as_ptr(),
            );
            // Use class_replaceMethod so we win even when the class (or one
            // of its superclasses, via class_addMethod's behavior) already
            // implements acceptsFirstMouse:.
            let _ = ffi::class_replaceMethod(
                cls_ptr,
                sel!(acceptsFirstMouse:),
                std::mem::transmute::<
                    unsafe extern "C-unwind" fn(&AnyObject, Sel, *mut AnyObject) -> bool,
                    Imp,
                >(accepts_first_mouse),
                bool_arg_encoding.as_ptr(),
            );
            log::info!(
                "[first-mouse] patched {} with acceptsFirstMouse:=YES",
                class_name.to_string_lossy()
            );
        }
    }

    INSTALL_ONCE.call_once(|| {
        let text_input_protocol = AnyProtocol::get(c"NSTextInputClient");
        patch_class(c"WryWebView", text_input_protocol);
        patch_class(c"WKWebView", text_input_protocol);
        // Patch NSView itself so EVERY subclass (including private/leaf
        // WebKit views whose names we cannot rely on across macOS versions)
        // returns YES from acceptsFirstMouse:. acceptsFirstMouse: is only
        // queried when the click target's window is not the key window, so
        // patching the base class is safe for normal activating windows.
        patch_class(c"NSView", None);
    });
}
