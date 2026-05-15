//! macOS-specific helpers. Gated by the outer `#[cfg(target_os = "macos")]` in `platform/mod.rs`.

use std::sync::atomic::Ordering;
use tauri::Manager;

use crate::large_collapsed_mascot_window_size;
use crate::platform::common::AppWindowInfo;
#[allow(unused_imports)]
use crate::state::*;

/// Get the notch half-width (distance from screen center to notch edge) using
/// macOS 12+ `auxiliaryTopRightArea` API. Falls back to 80pt for older systems
/// or screens without a notch (external displays, pre-notch Macs).
pub(crate) unsafe fn get_notch_offset(screen: *mut objc2::runtime::AnyObject) -> f64 {
    use objc2::msg_send;
    use objc2_foundation::NSRect;

    if screen.is_null() {
        return 80.0;
    }
    let sel = objc2::runtime::Sel::register(c"auxiliaryTopRightArea");
    let responds: bool = msg_send![&*screen, respondsToSelector: sel];
    if responds {
        let right_area: NSRect = msg_send![&*screen, auxiliaryTopRightArea];
        if right_area.size.width > 0.0 {
            let frame: NSRect = msg_send![&*screen, frame];
            let center_x = frame.origin.x + frame.size.width / 2.0;
            let half_w = right_area.origin.x - center_x;
            if half_w > 10.0 {
                return half_w;
            }
        }
    }
    80.0
}

/// Minimal CoreGraphics / CoreFoundation FFI for querying the live Dock
/// window bounds. We deliberately avoid the heavyweight `core-graphics`
/// crate — this is the only place that needs CGWindowList, and the
/// surface is tiny.
mod cg_window {
    use std::ffi::c_void;
    use std::os::raw::c_char;

    pub type CFTypeRef = *const c_void;
    pub type CFArrayRef = *const c_void;
    pub type CFDictionaryRef = *const c_void;
    pub type CFStringRef = *const c_void;
    pub type CFIndex = isize;

    #[link(name = "CoreGraphics", kind = "framework")]
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> CFArrayRef;
        pub fn CFArrayGetCount(arr: CFArrayRef) -> CFIndex;
        pub fn CFArrayGetValueAtIndex(arr: CFArrayRef, idx: CFIndex) -> CFTypeRef;
        pub fn CFDictionaryGetValue(d: CFDictionaryRef, key: CFTypeRef) -> CFTypeRef;
        pub fn CFStringCreateWithCString(
            alloc: CFTypeRef,
            cstr: *const c_char,
            enc: u32,
        ) -> CFStringRef;
        pub fn CFStringGetCString(
            s: CFStringRef,
            buf: *mut c_char,
            bufsz: CFIndex,
            enc: u32,
        ) -> bool;
        pub fn CFStringGetLength(s: CFStringRef) -> CFIndex;
        pub fn CFNumberGetValue(num: CFTypeRef, ty: i32, val: *mut c_void) -> bool;
        pub fn CFRelease(cf: CFTypeRef);
    }

    pub const OPTION_ON_SCREEN_ONLY: u32 = 1 << 0;
    pub const NULL_WINDOW_ID: u32 = 0;
    pub const STRING_ENCODING_UTF8: u32 = 0x08000100;
    pub const NUMBER_DOUBLE_TYPE: i32 = 13;
    // CFNumberType for 32-bit signed integers — used to read
    // kCGWindowLayer (declared as SInt32 in CoreGraphics).
    pub const NUMBER_SINT32_TYPE: i32 = 9;
}

unsafe fn cf_string_from(s: &str) -> cg_window::CFStringRef {
    if let Ok(c) = std::ffi::CString::new(s) {
        cg_window::CFStringCreateWithCString(
            std::ptr::null(),
            c.as_ptr(),
            cg_window::STRING_ENCODING_UTF8,
        )
    } else {
        std::ptr::null()
    }
}

unsafe fn cf_dict_get_double(dict: cg_window::CFDictionaryRef, key: &str) -> Option<f64> {
    let key_cf = cf_string_from(key);
    if key_cf.is_null() {
        return None;
    }
    let val = cg_window::CFDictionaryGetValue(dict, key_cf);
    cg_window::CFRelease(key_cf);
    if val.is_null() {
        return None;
    }
    let mut out: f64 = 0.0;
    let ok = cg_window::CFNumberGetValue(
        val,
        cg_window::NUMBER_DOUBLE_TYPE,
        &mut out as *mut f64 as *mut std::ffi::c_void,
    );
    if ok {
        Some(out)
    } else {
        None
    }
}

/// Read a top-level i32 value (such as kCGWindowLayer or kCGWindowAlpha
/// rounded to integer). Returns None if the key is absent or the bridge
/// fails.
unsafe fn cf_dict_get_i32(dict: cg_window::CFDictionaryRef, key: &str) -> Option<i32> {
    let key_cf = cf_string_from(key);
    if key_cf.is_null() {
        return None;
    }
    let val = cg_window::CFDictionaryGetValue(dict, key_cf);
    cg_window::CFRelease(key_cf);
    if val.is_null() {
        return None;
    }
    let mut out: i32 = 0;
    let ok = cg_window::CFNumberGetValue(
        val,
        cg_window::NUMBER_SINT32_TYPE,
        &mut out as *mut i32 as *mut std::ffi::c_void,
    );
    if ok {
        Some(out)
    } else {
        None
    }
}

unsafe fn cf_dict_get_string(dict: cg_window::CFDictionaryRef, key: &str) -> Option<String> {
    let key_cf = cf_string_from(key);
    if key_cf.is_null() {
        return None;
    }
    let val = cg_window::CFDictionaryGetValue(dict, key_cf);
    cg_window::CFRelease(key_cf);
    if val.is_null() {
        return None;
    }
    let len = cg_window::CFStringGetLength(val);
    if len <= 0 {
        return Some(String::new());
    }
    let bufsz = (len as usize) * 4 + 1; // UTF-8 worst case
    let mut buf: Vec<i8> = vec![0; bufsz];
    let ok = cg_window::CFStringGetCString(
        val,
        buf.as_mut_ptr(),
        bufsz as cg_window::CFIndex,
        cg_window::STRING_ENCODING_UTF8,
    );
    if !ok {
        return None;
    }
    let cstr = std::ffi::CStr::from_ptr(buf.as_ptr());
    Some(cstr.to_string_lossy().into_owned())
}

/// Compute the visible Dock strip's bounds in NS bottom-up logical
/// coords. Returns `Some((x, y, w, h))` where `(x, y)` is the
/// bottom-left of the Dock rect, or `None` when no Dock strip is on
/// screen (auto-hide engaged, side-Dock that the rest of the pipeline
/// treats as a wall, etc.).
///
/// We iterate every on-screen window from CGWindowList and pick the
/// strip-shaped one owned by either the `Dock` process or the
/// `Window Server` (the macOS WindowServer composes the Dock strip
/// in some configurations, so the strip's `kCGWindowOwnerName` can be
/// either). The first time this function runs after process start it
/// also logs every on-screen window (`owner | layer | alpha | x,y,w,h`)
/// — that lets us confirm on a real machine which row IS the Dock
/// strip without guessing. Subsequent calls only log the picked
/// candidate.
// Currently unused: see `get_pet_floor_info` / `move_mini_by`. Kept
// behind `#[allow(dead_code)]` so re-enabling Dock x-range detection
// later (e.g. when an explicit Screen-Recording opt-in lands) is a
// one-line change instead of a re-port of the CGWindowList scan.
#[allow(dead_code)]
unsafe fn compute_dock_rect_macos() -> Option<(f64, f64, f64, f64)> {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSRect;
    use std::sync::atomic::{AtomicBool, Ordering};

    static DUMPED_ONCE: AtomicBool = AtomicBool::new(false);
    let dump_now = !DUMPED_ONCE.swap(true, Ordering::Relaxed);

    let cls = AnyClass::get(c"NSScreen")?;
    let main_screen: *mut AnyObject = msg_send![cls, mainScreen];
    if main_screen.is_null() {
        return None;
    }
    let mframe: NSRect = msg_send![&*main_screen, frame];
    let main_w = mframe.size.width;
    let main_h = mframe.size.height;

    let list = cg_window::CGWindowListCopyWindowInfo(
        cg_window::OPTION_ON_SCREEN_ONLY,
        cg_window::NULL_WINDOW_ID,
    );
    if list.is_null() {
        log::warn!("[dock] CGWindowListCopyWindowInfo returned NULL");
        return None;
    }
    let count = cg_window::CFArrayGetCount(list);

    // Filter strategy (no hardcoded layer numbers — those vary by
    // macOS version; the diagnostic dump above lets us learn the
    // actual value on this machine and tighten the filter later):
    //   1. Owner must be "Dock" OR "Window Server" / "WindowServer"
    //      (case-insensitive). Dock-strip ownership varies across
    //      macOS versions.
    //   2. Width AND height each >= 30 (sanity floor).
    //   3. Reject wallpaper-sized backdrops: w >= 60% main_w AND
    //      h >= 60% main_h. The Dock process renders desktop
    //      backdrops which look wallpaper-shaped.
    //   4. Prefer strip-shaped survivors (max/min aspect >= 3) —
    //      the Dock is always long-thin (bottom: wide+short; side:
    //      tall+narrow). Among strip-shaped survivors, pick the
    //      one whose long side is largest.
    //   5. If no strip survives, fall back to the largest
    //      non-wallpaper survivor (atypical Dock configurations).
    let mut best: Option<(f64, f64, f64, f64, i32, String)> = None; // x, y_cg, w, h, layer, owner
    let mut best_strip_long_side: f64 = 0.0;
    let mut fallback: Option<(f64, f64, f64, f64, i32, String)> = None;
    let mut fallback_long_side: f64 = 0.0;
    let mut candidate_count = 0usize;

    for i in 0..count {
        let dict = cg_window::CFArrayGetValueAtIndex(list, i) as cg_window::CFDictionaryRef;
        if dict.is_null() {
            continue;
        }
        let owner = cf_dict_get_string(dict, "kCGWindowOwnerName").unwrap_or_default();
        let layer = cf_dict_get_i32(dict, "kCGWindowLayer").unwrap_or(0);
        let alpha = cf_dict_get_double(dict, "kCGWindowAlpha").unwrap_or(0.0);
        let bounds_key = cf_string_from("kCGWindowBounds");
        if bounds_key.is_null() {
            continue;
        }
        let bounds = cg_window::CFDictionaryGetValue(dict, bounds_key);
        cg_window::CFRelease(bounds_key);
        if bounds.is_null() {
            continue;
        }
        let x = cf_dict_get_double(bounds, "X").unwrap_or(0.0);
        let y_cg = cf_dict_get_double(bounds, "Y").unwrap_or(0.0);
        let w = cf_dict_get_double(bounds, "Width").unwrap_or(0.0);
        let h = cf_dict_get_double(bounds, "Height").unwrap_or(0.0);

        // First-call diagnostic: dump everything visible so we can
        // identify the Dock row by hand from real data.
        if dump_now {
            log::info!(
                "[dock/dump] owner={:?} layer={} alpha={:.2} x={} y_cg={} w={} h={}",
                owner,
                layer,
                alpha,
                x,
                y_cg,
                w,
                h,
            );
        }

        // Owner gate — Dock-strip ownership has historically been
        // either "Dock" or the Window Server depending on macOS
        // version, so accept both. Case-insensitive comparison
        // covers small naming variations like "Window Server" vs
        // "WindowServer".
        let owner_lower = owner.to_ascii_lowercase();
        let is_dock_owner = owner_lower == "dock"
            || owner_lower == "windowserver"
            || owner_lower == "window server";
        if !is_dock_owner {
            continue;
        }
        if w < 30.0 || h < 30.0 {
            continue;
        }
        let wallpaper_like = w >= main_w * 0.6 && h >= main_h * 0.6;
        if wallpaper_like {
            continue;
        }
        // Position gate — the Dock is always at a screen edge. On
        // macOS bottom-up coords (`y_cg + h ≈ main_h` = bottom edge,
        // `y_cg ≈ 0` = top edge of main screen). Reject the menu
        // bar, which lives at `y_cg ≈ 0` and would otherwise pass
        // the strip-shape filter and be mistaken for a Dock.
        let touches_bottom = (y_cg + h - main_h).abs() < 2.0;
        let touches_left = x.abs() < 2.0 && h > w; // tall strip at x≈0
        let touches_right = (x + w - main_w).abs() < 2.0 && h > w;
        if !(touches_bottom || touches_left || touches_right) {
            continue;
        }

        candidate_count += 1;
        let long_side = w.max(h);
        let short_side = w.min(h);
        let aspect = long_side / short_side.max(1.0);
        let row = (x, y_cg, w, h, layer, owner.clone());
        if aspect >= 3.0 {
            if long_side > best_strip_long_side {
                best_strip_long_side = long_side;
                best = Some(row);
            }
        } else if long_side > fallback_long_side {
            fallback_long_side = long_side;
            fallback = Some(row);
        }
    }
    cg_window::CFRelease(list);

    let chosen = best.or(fallback);
    // Per-tick selection log is debug-only — it would otherwise spam
    // INFO at 2 Hz. The first-call window-table dump above is the
    // INFO record we keep around to verify behavior on a real machine.
    log::debug!(
        "[dock] count={} dock_or_ws_candidates={} chosen={:?}",
        count,
        candidate_count,
        chosen,
    );
    let (x, y_cg, w, h, _layer, _owner) = chosen?;
    let ns_y = mframe.origin.y + mframe.size.height - y_cg - h;
    Some((x, ns_y, w, h))
}

/// 500 ms TTL cache around `compute_dock_rect_macos`. Returns `None`
/// when the Dock strip isn't visible to CGWindowList (the macOS 14.4+
/// privacy gate hides cross-app windows from callers without Screen
/// Recording permission). The frontend / `move_mini_by` clamp treat
/// `None` as "Dock spans the full visibleFrame width" — the cat sits
/// on top of the Dock as a full-width platform. No estimates, no
/// guessing: either we have real data or we explicitly have nothing.
#[allow(dead_code)]
fn get_cached_dock_rect_macos() -> Option<(f64, f64, f64, f64)> {
    use std::time::{Duration, Instant};
    static CACHE: std::sync::Mutex<Option<(Instant, Option<(f64, f64, f64, f64)>)>> =
        std::sync::Mutex::new(None);
    const TTL: Duration = Duration::from_millis(500);
    let mut cache = CACHE.lock().ok()?;
    if let Some((at, val)) = *cache {
        if at.elapsed() < TTL {
            return val;
        }
    }
    let fresh = unsafe { compute_dock_rect_macos() };
    *cache = Some((Instant::now(), fresh));
    fresh
}

/// Pick the topmost normal app window from CGWindowList, excluding the
/// mascot's own windows. Returns `None` when no qualifying window is
/// visible (everything minimized to Dock, only utility panels open, …).
///
/// Filtering pipeline matches Shimeji's "interactable window" concept:
/// `layer == 0` keeps normal app windows (rejecting menu bar items,
/// floating palettes, Spotlight, the Dock itself); `alpha >= 0.5` skips
/// fading/transitioning windows; a minimum size threshold drops
/// tooltips, popovers and completion menus that would be silly
/// platforms for a cat.
///
/// CGWindowList returns windows in z-order front-to-back, so the first
/// passing entry is the topmost — exactly what we want.
pub(crate) unsafe fn compute_frontmost_app_window_macos() -> Option<AppWindowInfo> {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSRect;

    // Main-screen frame is needed to flip CGWindowList's top-down y to
    // Cocoa bottom-up y. Multi-display refinement is deferred — the
    // mascot itself is currently main-screen-centric per `get_pet_floor_info`.
    let cls = AnyClass::get(c"NSScreen")?;
    let main: *mut AnyObject = msg_send![cls, mainScreen];
    if main.is_null() {
        return None;
    }
    let mframe: NSRect = msg_send![&*main, frame];
    let main_h = mframe.size.height;
    let main_origin_y = mframe.origin.y;

    let our_pid = std::process::id() as i32;

    let list = cg_window::CGWindowListCopyWindowInfo(
        cg_window::OPTION_ON_SCREEN_ONLY,
        cg_window::NULL_WINDOW_ID,
    );
    if list.is_null() {
        return None;
    }
    let count = cg_window::CFArrayGetCount(list);

    let mut best: Option<AppWindowInfo> = None;
    for i in 0..count {
        let dict = cg_window::CFArrayGetValueAtIndex(list, i) as cg_window::CFDictionaryRef;
        if dict.is_null() {
            continue;
        }

        let layer = cf_dict_get_i32(dict, "kCGWindowLayer").unwrap_or(99);
        if layer != 0 {
            continue;
        }
        let alpha = cf_dict_get_double(dict, "kCGWindowAlpha").unwrap_or(0.0);
        if alpha < 0.5 {
            continue;
        }
        let owner_pid = cf_dict_get_i32(dict, "kCGWindowOwnerPID").unwrap_or(0);
        if owner_pid == our_pid {
            continue;
        }
        let owner = cf_dict_get_string(dict, "kCGWindowOwnerName").unwrap_or_default();
        let owner_lower = owner.to_ascii_lowercase();
        if owner_lower == "dock" || owner_lower == "windowserver" || owner_lower == "window server"
        {
            continue;
        }

        let bounds_key = cf_string_from("kCGWindowBounds");
        if bounds_key.is_null() {
            continue;
        }
        let bounds = cg_window::CFDictionaryGetValue(dict, bounds_key);
        cg_window::CFRelease(bounds_key);
        if bounds.is_null() {
            continue;
        }
        let x = cf_dict_get_double(bounds, "X").unwrap_or(0.0);
        let y_cg = cf_dict_get_double(bounds, "Y").unwrap_or(0.0);
        let w = cf_dict_get_double(bounds, "Width").unwrap_or(0.0);
        let h = cf_dict_get_double(bounds, "Height").unwrap_or(0.0);
        // Tooltips, popovers and completion menus are too small to be
        // useful platforms; also skip degenerate w/h <= 0.
        if w < 200.0 || h < 120.0 {
            continue;
        }

        let window_id = cf_dict_get_i32(dict, "kCGWindowNumber").unwrap_or(0) as u32;
        let ns_y = main_origin_y + main_h - y_cg - h;

        best = Some(AppWindowInfo {
            window_id,
            owner_name: owner,
            owner_pid,
            x,
            y: ns_y,
            width: w,
            height: h,
        });
        break; // z-order front-to-back: first match wins.
    }
    cg_window::CFRelease(list);
    best
}

/// 50 ms TTL cache around the frontmost-window scan. Keeps 30 ms physics
/// ticks cheap — at most ~20 actual CGWindowList scans/sec regardless
/// of tick rate.
pub(crate) mod frontmost_app_window_cache {
    use super::AppWindowInfo;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    static CACHE: Mutex<Option<(Instant, Option<AppWindowInfo>)>> = Mutex::new(None);
    const TTL: Duration = Duration::from_millis(50);

    /// Returns `Some(val)` when a fresh value is cached; `None` when the
    /// caller should run a fresh scan.
    pub fn try_fresh() -> Option<Option<AppWindowInfo>> {
        let cache = CACHE.lock().ok()?;
        let (at, val) = cache.as_ref()?;
        if at.elapsed() < TTL {
            Some(val.clone())
        } else {
            None
        }
    }

    pub fn store(val: Option<AppWindowInfo>) {
        if let Ok(mut c) = CACHE.lock() {
            *c = Some((Instant::now(), val));
        }
    }
}

// === Phase 2b: cursor/mouse + music/audio cluster ===

/// Schedule a main-thread task that snaps the mini window origin to
/// `(cursor_now - DRAG_ANCHOR)` — i.e. wherever the cursor currently is,
/// minus the offset captured at drag-start. Calls coalesce: while a task
/// is in flight, repeated invocations are no-ops; the running task always
/// reads the freshest cursor position. This keeps drag tracking tight
/// even when the poll thread runs much faster than the main thread can
/// repaint, and avoids the cumulative lag of relative-delta translation.
pub(crate) fn request_drag_apply(app: &tauri::AppHandle) {
    if DRAG_TASK_PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app_clone = app.clone();
    let _ = app.run_on_main_thread(move || {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;
        use objc2_foundation::NSPoint;

        DRAG_TASK_PENDING.store(false, Ordering::SeqCst);
        let anchor = drag_anchor().lock().ok().and_then(|g| *g);
        let Some((ax, ay)) = anchor else { return };

        let cursor = macos_cursor_position();
        let new_origin = NSPoint::new(cursor.0 - ax, cursor.1 - ay);

        if let Some(win) = app_clone.get_webview_window("main") {
            if let Ok(ns_win) = win.ns_window() {
                let obj = unsafe { &*(ns_win as *mut AnyObject) };
                // setFrameOrigin: only moves the window — it does not
                // redraw the contents — so it is far cheaper than
                // setFrame:display:animate:NO and keeps up with fast
                // cursor motion.
                unsafe {
                    let _: () = msg_send![obj, setFrameOrigin: new_origin];
                }
                if let Ok(mut f) = MINI_WINDOW_FRAME.lock() {
                    if let Some((_, _, w, h)) = *f {
                        *f = Some((new_origin.x, new_origin.y, w, h));
                    }
                }
            }
        }
    });
}
/// Read the current mouse cursor position via `[NSEvent mouseLocation]`.
/// Returns (x, y) in macOS screen coordinates (bottom-left origin).
pub(crate) fn macos_cursor_position() -> (f64, f64) {
    unsafe {
        use objc2::msg_send;
        use objc2_foundation::NSPoint;
        if let Some(cls) = objc2::runtime::AnyClass::get(c"NSEvent") {
            let loc: NSPoint = msg_send![cls, mouseLocation];
            (loc.x, loc.y)
        } else {
            (0.0, 0.0)
        }
    }
}
/// Returns the bitmask of currently pressed mouse buttons via
/// `[NSEvent pressedMouseButtons]`. Bit 0 = left button. This works
/// regardless of whether the receiving window is the key window, which is
/// what we need to detect drags on the floating mini mascot.
pub(crate) fn macos_pressed_mouse_buttons() -> usize {
    unsafe {
        use objc2::msg_send;
        if let Some(cls) = objc2::runtime::AnyClass::get(c"NSEvent") {
            let mask: usize = msg_send![cls, pressedMouseButtons];
            mask
        } else {
            0
        }
    }
}
/// Schedule restoring the NSWindow alpha to 1.0 after the webview has had
/// time to composite at the new frame size.  Uses GCD `dispatch_after_f` on
/// the main queue so the restore runs at a precise time without thread-spawn
/// overhead.  A generation counter (`PET_ALPHA_GEN`) prevents stale callbacks
/// from restoring alpha during a subsequent resize (fast double-clicks).
pub(crate) fn pet_context_schedule_restore_alpha(ns_win_ptr: *mut std::ffi::c_void) {
    extern "C" {
        // dispatch_get_main_queue() is a C macro; the real symbol is a global.
        #[link_name = "_dispatch_main_q"]
        static DISPATCH_MAIN_Q: std::ffi::c_void;
        fn dispatch_after_f(
            when: u64,
            queue: *const std::ffi::c_void,
            context: *mut std::ffi::c_void,
            work: extern "C" fn(*mut std::ffi::c_void),
        );
        fn dispatch_time(when: u64, delta: i64) -> u64;
    }

    /// Packed context passed through GCD void* pointer.
    struct RestoreCtx {
        ns_win: *mut std::ffi::c_void,
        gen: u64,
    }

    extern "C" fn restore_alpha(ctx_raw: *mut std::ffi::c_void) {
        let ctx = unsafe { Box::from_raw(ctx_raw as *mut RestoreCtx) };
        // Only restore if no newer resize has happened since we were scheduled.
        if PET_ALPHA_GEN.load(Ordering::SeqCst) != ctx.gen {
            return;
        }
        use objc2::msg_send;
        let obj = unsafe { &*(ctx.ns_win as *const objc2::runtime::AnyObject) };
        unsafe {
            let _: () = msg_send![obj, setAlphaValue: 1.0f64];
        }
    }

    let gen = PET_ALPHA_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let ctx = Box::new(RestoreCtx {
        ns_win: ns_win_ptr,
        gen,
    });
    unsafe {
        // 34ms ≈ 2 frames at 60Hz — minimal delay for the webview to
        // finish compositing at the new window size.
        let when = dispatch_time(0, 34_000_000); // nanoseconds
        dispatch_after_f(
            when,
            &DISPATCH_MAIN_Q as *const std::ffi::c_void,
            Box::into_raw(ctx) as *mut std::ffi::c_void,
            restore_alpha,
        );
    }
}
/// Get the bundle identifier of the frontmost application.
pub(crate) fn get_frontmost_bundle_id() -> String {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    unsafe {
        let cls = match AnyClass::get(c"NSWorkspace") {
            Some(c) => c,
            None => return String::new(),
        };
        let ws: *mut AnyObject = msg_send![cls, sharedWorkspace];
        if ws.is_null() {
            return String::new();
        }
        let front_app: *mut AnyObject = msg_send![&*ws, frontmostApplication];
        if front_app.is_null() {
            return String::new();
        }
        let bid_ns: *mut AnyObject = msg_send![&*front_app, bundleIdentifier];
        if bid_ns.is_null() {
            return String::new();
        }
        let utf8: *const u8 = msg_send![&*bid_ns, UTF8String];
        if utf8.is_null() {
            return String::new();
        }
        let len: usize = msg_send![&*bid_ns, length];
        String::from_utf8_lossy(std::slice::from_raw_parts(utf8, len)).into_owned()
    }
}
const MUSIC_APP_BIDS: &[&str] = &[
    "com.apple.music",
    "com.spotify.client",
    "com.netease.163music",
    "com.tencent.qqmusic",
    "com.kugou",
    "com.kuwo",
    "com.xiami.client",
    "com.apple.itunes",
    "com.soda.music",
    "com.bytedance.soda.music",
];
pub(crate) fn is_music_app(bid: &str) -> bool {
    MUSIC_APP_BIDS.iter().any(|m| bid.contains(m))
}
fn is_music_app_running() -> bool {
    let script = r#"
        set musicBids to {"com.apple.music", "com.spotify.client", "com.netease.163music", "com.tencent.qqmusic", "com.kugou", "com.kuwo", "com.xiami.client", "com.apple.itunes", "com.soda.music", "com.bytedance.soda.music"}
        repeat with bid in musicBids
            try
                if application id (bid as text) is running then return "1"
            end try
        end repeat
        return "0"
    "#;
    match std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim() == "1",
        Err(_) => false,
    }
}
fn _get_system_now_playing_is_playing_unused() -> Option<bool> {
    use block2::RcBlock;
    use std::ffi::c_void;
    use std::sync::mpsc::channel;
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    type DispatchQueue = *mut std::ffi::c_void;
    type PlaybackState = u32;

    const MEDIA_REMOTE_PLAYING: PlaybackState = 1;
    const MEDIA_REMOTE_AMBIGUOUS: PlaybackState = 2;
    const K_CFNUMBER_DOUBLE_TYPE: i32 = 13;
    const K_CFSTRING_ENCODING_UTF8: u32 = 0x0800_0100;

    type MrGetIsPlayingFn = unsafe extern "C" fn(DispatchQueue, &block2::Block<dyn Fn(i8)>);
    type MrGetPlaybackStateFn =
        unsafe extern "C" fn(DispatchQueue, &block2::Block<dyn Fn(PlaybackState)>);
    type MrGetNowPlayingInfoFn =
        unsafe extern "C" fn(DispatchQueue, &block2::Block<dyn Fn(*const c_void)>);
    type DispatchGetGlobalQueueFn = unsafe extern "C" fn(isize, usize) -> DispatchQueue;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
        fn CFNumberGetValue(number: *const c_void, the_type: i32, value: *mut c_void) -> u8;
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const u8,
            encoding: u32,
        ) -> *const c_void;
    }

    static MR_GET_IS_PLAYING_FN: OnceLock<MrGetIsPlayingFn> = OnceLock::new();
    static MR_GET_STATE_FN: OnceLock<MrGetPlaybackStateFn> = OnceLock::new();
    static MR_GET_INFO_FN: OnceLock<MrGetNowPlayingInfoFn> = OnceLock::new();
    static MR_PLAYBACK_RATE_KEY_ADDR: OnceLock<usize> = OnceLock::new();
    static MR_ELAPSED_TIME_KEY_ADDR: OnceLock<usize> = OnceLock::new();
    static DISPATCH_GET_GLOBAL_QUEUE_FN: OnceLock<DispatchGetGlobalQueueFn> = OnceLock::new();
    static LAST_ELAPSED_SAMPLE: OnceLock<Mutex<Option<(f64, f64)>>> = OnceLock::new();

    unsafe {
        let mr_handle = libc::dlopen(
            c"/System/Library/PrivateFrameworks/MediaRemote.framework/MediaRemote"
                .as_ptr()
                .cast(),
            libc::RTLD_NOW,
        );
        if mr_handle.is_null() {
            log::info!("[now_playing/media_remote] dlopen MediaRemote failed");
            return None;
        }

        let get_is_playing = if let Some(f) = MR_GET_IS_PLAYING_FN.get() {
            Some(*f)
        } else {
            let mr_is_playing_sym = libc::dlsym(
                mr_handle,
                c"MRMediaRemoteGetNowPlayingApplicationIsPlaying"
                    .as_ptr()
                    .cast(),
            );
            if mr_is_playing_sym.is_null() {
                None
            } else {
                let f: MrGetIsPlayingFn =
                    std::mem::transmute::<*mut c_void, MrGetIsPlayingFn>(mr_is_playing_sym);
                let _ = MR_GET_IS_PLAYING_FN.set(f);
                Some(f)
            }
        };

        let get_playback_state = if let Some(f) = MR_GET_STATE_FN.get() {
            Some(*f)
        } else {
            let mr_handle = libc::dlopen(
                c"/System/Library/PrivateFrameworks/MediaRemote.framework/MediaRemote"
                    .as_ptr()
                    .cast(),
                libc::RTLD_NOW,
            );
            if mr_handle.is_null() {
                None
            } else {
                let mr_sym = libc::dlsym(
                    mr_handle,
                    c"MRMediaRemoteGetNowPlayingApplicationPlaybackState"
                        .as_ptr()
                        .cast(),
                );
                if mr_sym.is_null() {
                    None
                } else {
                    let f: MrGetPlaybackStateFn =
                        std::mem::transmute::<*mut c_void, MrGetPlaybackStateFn>(mr_sym);
                    let _ = MR_GET_STATE_FN.set(f);
                    Some(f)
                }
            }
        };

        let get_now_playing_info = if let Some(f) = MR_GET_INFO_FN.get() {
            Some(*f)
        } else {
            let mr_info_sym =
                libc::dlsym(mr_handle, c"MRMediaRemoteGetNowPlayingInfo".as_ptr().cast());
            if mr_info_sym.is_null() {
                None
            } else {
                let f: MrGetNowPlayingInfoFn =
                    std::mem::transmute::<*mut c_void, MrGetNowPlayingInfoFn>(mr_info_sym);
                let _ = MR_GET_INFO_FN.set(f);
                Some(f)
            }
        };

        let playback_rate_key = if let Some(addr) = MR_PLAYBACK_RATE_KEY_ADDR.get() {
            Some(*addr as *const c_void)
        } else {
            let key_sym = libc::dlsym(
                mr_handle,
                c"kMRMediaRemoteNowPlayingInfoPlaybackRate".as_ptr().cast(),
            );
            let key = if key_sym.is_null() {
                let fallback = CFStringCreateWithCString(
                    std::ptr::null(),
                    c"kMRMediaRemoteNowPlayingInfoPlaybackRate".as_ptr().cast(),
                    K_CFSTRING_ENCODING_UTF8,
                );
                if fallback.is_null() {
                    std::ptr::null()
                } else {
                    fallback
                }
            } else {
                // Exported as CFStringRef* global; dereference once to get key object.
                *(key_sym as *const *const c_void)
            };
            if key.is_null() {
                None
            } else {
                let _ = MR_PLAYBACK_RATE_KEY_ADDR.set(key as usize);
                Some(key)
            }
        };

        let elapsed_time_key = if let Some(addr) = MR_ELAPSED_TIME_KEY_ADDR.get() {
            Some(*addr as *const c_void)
        } else {
            let key_sym = libc::dlsym(
                mr_handle,
                c"kMRMediaRemoteNowPlayingInfoElapsedTime".as_ptr().cast(),
            );
            let key = if key_sym.is_null() {
                let fallback = CFStringCreateWithCString(
                    std::ptr::null(),
                    c"kMRMediaRemoteNowPlayingInfoElapsedTime".as_ptr().cast(),
                    K_CFSTRING_ENCODING_UTF8,
                );
                if fallback.is_null() {
                    std::ptr::null()
                } else {
                    fallback
                }
            } else {
                *(key_sym as *const *const c_void)
            };
            if key.is_null() {
                None
            } else {
                let _ = MR_ELAPSED_TIME_KEY_ADDR.set(key as usize);
                Some(key)
            }
        };

        let get_global_queue = if let Some(f) = DISPATCH_GET_GLOBAL_QUEUE_FN.get() {
            *f
        } else {
            let dispatch_handle = libc::dlopen(
                c"/usr/lib/system/libdispatch.dylib".as_ptr().cast(),
                libc::RTLD_NOW,
            );
            if dispatch_handle.is_null() {
                log::info!("[now_playing/media_remote] dlopen libdispatch failed");
                return None;
            }
            let dispatch_sym = libc::dlsym(
                dispatch_handle,
                c"dispatch_get_global_queue".as_ptr().cast(),
            );
            if dispatch_sym.is_null() {
                log::info!("[now_playing/media_remote] dlsym dispatch_get_global_queue failed");
                return None;
            }
            let f: DispatchGetGlobalQueueFn =
                std::mem::transmute::<*mut c_void, DispatchGetGlobalQueueFn>(dispatch_sym);
            let _ = DISPATCH_GET_GLOBAL_QUEUE_FN.set(f);
            f
        };

        let queue = get_global_queue(0, 0);

        // Best signal: now playing info playbackRate (0 paused, 1 playing).
        if let Some(get_now_playing_info_fn) = get_now_playing_info {
            let (tx, rx) = channel::<(Option<f64>, Option<f64>)>();
            let callback = RcBlock::new(move |info: *const c_void| {
                if info.is_null() {
                    let _ = tx.send((None, None));
                    return;
                }
                let read_number = |key: Option<*const c_void>| -> Option<f64> {
                    let k = key?;
                    let value = CFDictionaryGetValue(info, k);
                    if value.is_null() {
                        return None;
                    }
                    let mut n: f64 = 0.0;
                    let ok = CFNumberGetValue(
                        value,
                        K_CFNUMBER_DOUBLE_TYPE,
                        &mut n as *mut f64 as *mut c_void,
                    );
                    if ok != 0 {
                        Some(n)
                    } else {
                        None
                    }
                };
                let rate = read_number(playback_rate_key);
                let elapsed = read_number(elapsed_time_key);
                let _ = tx.send((rate, elapsed));
            });
            get_now_playing_info_fn(queue, &callback);
            match rx.recv_timeout(Duration::from_millis(220)) {
                Ok((Some(rate), _)) => {
                    let is_playing = rate > 0.01;
                    log::info!(
                        "[now_playing/media_remote] playback_rate={} source=now_playing_info is_playing={}",
                        rate, is_playing
                    );
                    return Some(is_playing);
                }
                Ok((None, Some(elapsed))) => {
                    let now_sec = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs_f64())
                        .unwrap_or(0.0);
                    let cache = LAST_ELAPSED_SAMPLE.get_or_init(|| Mutex::new(None));
                    let mut guard = cache.lock().unwrap();
                    let inferred = if let Some((prev_elapsed, prev_ts)) = *guard {
                        let dt = (now_sec - prev_ts).max(0.001);
                        let de = elapsed - prev_elapsed;
                        // Progress increasing at a meaningful pace => playing.
                        // Paused typically keeps elapsed almost unchanged.
                        Some(de > dt * 0.15)
                    } else {
                        None
                    };
                    *guard = Some((elapsed, now_sec));
                    log::info!(
                        "[now_playing/media_remote] elapsed_time={} source=elapsed_fallback inferred={:?}",
                        elapsed, inferred
                    );
                    if let Some(v) = inferred {
                        return Some(v);
                    }
                }
                Ok((None, None)) => {
                    log::info!(
                        "[now_playing/media_remote] playback_rate/elapsed missing source=now_playing_info fallback=is_playing/state"
                    );
                }
                Err(_) => {
                    log::info!(
                        "[now_playing/media_remote] now_playing_info timeout fallback=is_playing/state"
                    );
                }
            }
        }

        let mut is_playing_api_result: Option<bool> = None;
        if let Some(get_is_playing_fn) = get_is_playing {
            let (tx, rx) = channel::<i8>();
            let callback = RcBlock::new(move |is_playing: i8| {
                let _ = tx.send(is_playing);
            });
            get_is_playing_fn(queue, &callback);
            match rx.recv_timeout(Duration::from_millis(220)) {
                Ok(is_playing_raw) => {
                    let is_playing = is_playing_raw != 0;
                    log::info!(
                        "[now_playing/media_remote] is_playing_api={} source=is_playing",
                        is_playing
                    );
                    is_playing_api_result = Some(is_playing);
                }
                Err(_) => {
                    log::info!("[now_playing/media_remote] is_playing_api timeout, fallback=playback_state");
                }
            }
        }

        if let Some(get_playback_state_fn) = get_playback_state {
            let (tx, rx) = channel::<PlaybackState>();
            let callback = RcBlock::new(move |state: PlaybackState| {
                let _ = tx.send(state);
            });
            get_playback_state_fn(queue, &callback);
            let playback_state_result = match rx.recv_timeout(Duration::from_millis(220)) {
                Ok(state) => {
                    log::info!(
                        "[now_playing/media_remote] playback_state={} source=state_fallback",
                        state
                    );
                    Some(state)
                }
                Err(_) => {
                    log::info!("[now_playing/media_remote] playback_state timeout");
                    None
                }
            };
            let audio_active = is_audio_output_active();
            return match (is_playing_api_result, playback_state_result) {
                // Prefer explicit API when it reliably reports playing.
                (Some(true), _) => Some(true),
                // Some integrations always return false from is_playing API.
                // In that case, accept ambiguous state=2 only when audio output is active.
                (Some(false), Some(state)) if state == MEDIA_REMOTE_AMBIGUOUS => {
                    let inferred = false;
                    log::info!(
                        "[now_playing/media_remote] reconcile is_playing=false state=2 audio_active={} inferred={}",
                        audio_active, inferred
                    );
                    Some(inferred)
                }
                (Some(false), Some(state)) => {
                    let inferred = state == MEDIA_REMOTE_PLAYING;
                    log::info!(
                        "[now_playing/media_remote] reconcile is_playing=false state={} inferred={}",
                        state, inferred
                    );
                    Some(inferred)
                }
                // If explicit API timed out/unavailable, use state + audio tie-breaker.
                (None, Some(state)) if state == MEDIA_REMOTE_AMBIGUOUS => {
                    let inferred = audio_active;
                    log::info!(
                        "[now_playing/media_remote] reconcile no_is_playing state=2 audio_active={} inferred={}",
                        audio_active, inferred
                    );
                    Some(inferred)
                }
                (None, Some(state)) => Some(state == MEDIA_REMOTE_PLAYING),
                (Some(v), None) => Some(v),
                (None, None) => None,
            };
        }

        if is_playing_api_result.is_some() {
            return is_playing_api_result;
        }
        log::info!("[now_playing/media_remote] no usable media_remote symbol");
        None
    }
}
/// Check if the default audio output device has any audio running.
/// Used only as a tie-breaker for ambiguous MediaRemote states.
fn is_audio_output_active() -> bool {
    #[allow(non_upper_case_globals)]
    const kAudioHardwarePropertyDefaultOutputDevice: u32 = u32::from_be_bytes(*b"dOut");
    #[allow(non_upper_case_globals)]
    const kAudioDevicePropertyDeviceIsRunningSomewhere: u32 = u32::from_be_bytes(*b"gone");
    #[allow(non_upper_case_globals)]
    const kAudioObjectPropertyScopeGlobal: u32 = u32::from_be_bytes(*b"glob");
    #[allow(non_upper_case_globals)]
    const kAudioObjectPropertyElementMain: u32 = 0;
    #[allow(non_upper_case_globals)]
    const kAudioObjectSystemObject: u32 = 1;

    #[repr(C)]
    struct AudioObjectPropertyAddress {
        selector: u32,
        scope: u32,
        element: u32,
    }

    #[link(name = "CoreAudio", kind = "framework")]
    unsafe extern "C" {
        fn AudioObjectGetPropertyData(
            id: u32,
            addr: *const AudioObjectPropertyAddress,
            qualifier_size: u32,
            qualifier: *const std::ffi::c_void,
            data_size: *mut u32,
            data: *mut std::ffi::c_void,
        ) -> i32;
    }

    unsafe {
        let addr = AudioObjectPropertyAddress {
            selector: kAudioHardwarePropertyDefaultOutputDevice,
            scope: kAudioObjectPropertyScopeGlobal,
            element: kAudioObjectPropertyElementMain,
        };
        let mut device: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let err = AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            &mut device as *mut u32 as *mut std::ffi::c_void,
        );
        if err != 0 || device == 0 {
            return false;
        }

        let addr2 = AudioObjectPropertyAddress {
            selector: kAudioDevicePropertyDeviceIsRunningSomewhere,
            scope: kAudioObjectPropertyScopeGlobal,
            element: kAudioObjectPropertyElementMain,
        };
        let mut running: u32 = 0;
        size = std::mem::size_of::<u32>() as u32;
        let err2 = AudioObjectGetPropertyData(
            device,
            &addr2,
            0,
            std::ptr::null(),
            &mut size,
            &mut running as *mut u32 as *mut std::ffi::c_void,
        );
        err2 == 0 && running != 0
    }
}
/// Use `nowplaying-cli` to check playback rate and source app.
/// Returns (is_playing, source_bundle_id) or None if tool unavailable.
pub(crate) fn nowplaying_cli_status() -> Option<(bool, String)> {
    static CLI_PATH: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let path = CLI_PATH.get_or_init(|| {
        for p in &[
            "/opt/homebrew/bin/nowplaying-cli",
            "/usr/local/bin/nowplaying-cli",
        ] {
            if std::path::Path::new(p).exists() {
                return Some(p.to_string());
            }
        }
        None
    });
    let cli = path.as_deref()?;
    let output = std::process::Command::new(cli)
        .args(["get", "playbackRate", "clientBundleIdentifier"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();
    let rate: f64 = lines.next()?.trim().parse().ok()?;
    let source_bid = lines.next().unwrap_or("").trim().to_lowercase();
    Some((rate > 0.01, source_bid))
}
pub(crate) fn is_any_music_app_playing() -> bool {
    let script = r#"
        set isPlaying to false

        -- Check apps that support "player state" AppleScript
        if application "Music" is running then
            tell application "Music"
                try
                    if player state is playing then set isPlaying to true
                end try
            end tell
        end if

        if (not isPlaying) and application "Spotify" is running then
            tell application "Spotify"
                try
                    if player state is playing then set isPlaying to true
                end try
            end tell
        end if

        -- For apps without AppleScript player-state (NeteaseMusic, QQ Music, etc.),
        -- check the system menu bar: the first item in the "控制" menu
        -- toggles between "播放"/"暂停" or "Play"/"Pause".
        if not isPlaying then
            tell application "System Events"
                set menuChecks to {{"com.netease.163music", "控制"}, {"com.tencent.qqmusic", "控制"}, {"com.soda.music", "控制"}, {"com.bytedance.soda.music", "控制"}}
                repeat with entry in menuChecks
                    if isPlaying then exit repeat
                    set bid to item 1 of entry
                    set menuName to item 2 of entry
                    try
                        set procs to every process whose bundle identifier is bid
                        if (count of procs) > 0 then
                            set p to item 1 of procs
                            set firstItem to name of menu item 1 of menu 1 of menu bar item menuName of menu bar 1 of p
                            if firstItem is "暂停" or firstItem is "Pause" then
                                set isPlaying to true
                            end if
                        end if
                    end try
                end repeat
            end tell
        end if

        if isPlaying then
            return "1"
        else
            return "0"
        end if
    "#;

    match std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
    {
        Ok(output) => {
            let result = String::from_utf8_lossy(&output.stdout).trim() == "1";
            log::info!("[now_playing/script] is_any_music_app_playing={}", result);
            result
        }
        Err(_) => false,
    }
}
pub(crate) fn is_video_app(bid: &str) -> bool {
    const VIDEO_APPS: &[&str] = &[
        "com.colliderli.iina",
        "org.videolan.vlc",
        "com.apple.quicktimeplayer",
        "tv.plex.plexmediaplayer",
        "io.mpv",
        "com.apple.tv",
        "com.bilibili.bili",
        "com.disneyplus",
        "com.netflix",
    ];
    VIDEO_APPS.iter().any(|v| bid.contains(v))
}
pub(crate) fn is_browser(bid: &str) -> bool {
    const BROWSERS: &[&str] = &[
        "com.google.chrome",
        "org.mozilla.firefox",
        "com.apple.safari",
        "com.microsoft.edgemac",
        "com.brave.browser",
        "com.vivaldi.vivaldi",
        "company.thebrowser.browser",
        "com.operasoftware.opera",
    ];
    BROWSERS.iter().any(|b| bid.contains(b))
}

// === Phase 2c: pet/terminal/AX/IME cluster ===

/// Polling loop for pet-mode click pass-through. Checks cursor position every
/// 20ms. When the cursor is over the mascot (bottom-right of the expanded
/// window) or the context menu is open, `setIgnoresMouseEvents: false` so the
/// webview receives events. Otherwise `setIgnoresMouseEvents: true` so clicks
/// pass through to whatever is behind.
pub(crate) fn pet_passthrough_poll(
    app: tauri::AppHandle,
    mascot_scale: f64,
    large_mascot_scale: f64,
) {
    use std::time::Duration;
    PET_PASSTHROUGH_THREAD_ALIVE.store(true, Ordering::SeqCst);
    let mut was_interactive = false;
    let (mascot_w, mascot_h) = large_collapsed_mascot_window_size(mascot_scale, large_mascot_scale);
    // Keep these ratios aligned with frontend `Mini.tsx` so hover cursor and
    // native pass-through behavior remain consistent around mascot edges.
    let hit_w = mascot_w * (2.4 / 3.0);
    let hit_h = mascot_h * (2.8 / 3.0);
    let inset_x = (mascot_w - hit_w) / 2.0;
    let inset_y = (mascot_h - hit_h) / 2.0;
    let edge_threshold = 30.0;
    // Get screen bounds once at startup so we can detect edge proximity.
    let screen_bounds: Option<(f64, f64, f64, f64)> = {
        let (tx, rx) = std::sync::mpsc::channel();
        let app_c = app.clone();
        let _ = app_c.run_on_main_thread(move || {
            use objc2::msg_send;
            use objc2::runtime::{AnyClass, AnyObject};
            use objc2_foundation::NSRect;
            let result: Option<(f64, f64, f64, f64)> = unsafe {
                AnyClass::get(c"NSScreen").and_then(|cls| {
                    let ms: *mut AnyObject = msg_send![cls, mainScreen];
                    if ms.is_null() {
                        None
                    } else {
                        let sf: NSRect = msg_send![&*ms, frame];
                        Some((sf.origin.x, sf.origin.y, sf.size.width, sf.size.height))
                    }
                })
            };
            let _ = tx.send(result);
        });
        rx.recv().ok().flatten()
    };

    while PET_PASSTHROUGH_ACTIVE.load(Ordering::SeqCst) {
        let menu_open = PET_CONTEXT_MENU_OPEN.load(Ordering::SeqCst);
        let pomodoro_active = PET_POMODORO_ACTIVE.load(Ordering::SeqCst);
        let frame = MINI_WINDOW_FRAME.lock().ok().and_then(|g| *g);

        let should_be_interactive = if menu_open || pomodoro_active {
            true
        } else if let Some((fx, fy, fw, _fh)) = frame {
            let cursor = macos_cursor_position();
            let mascot_left = fx + fw - mascot_w;
            let mascot_right = mascot_left + mascot_w;
            let mascot_bottom = fy;
            // Drop hitbox insets when the mascot extends near/past a screen
            // edge so the visible portion stays fully clickable.
            let near_edge = if let Some((sx, _sy, sw, _sh)) = screen_bounds {
                mascot_left < sx + edge_threshold || mascot_right > sx + sw - edge_threshold
            } else {
                mascot_left < edge_threshold
            };
            // Near screen edge, keep hitbox reasonably generous but never full-rect.
            // Full-rect near-edge hitboxes make peek feel "too clickable" and steal
            // hover/clicks away from nearby desktop content.
            let ix = if near_edge { inset_x * 0.5 } else { inset_x };
            let iy = inset_y;
            let hit_left = mascot_left + ix;
            let hit_right = mascot_right - ix;
            let hit_bottom = mascot_bottom + iy;
            let hit_top = mascot_bottom + mascot_h - iy;
            cursor.0 >= hit_left
                && cursor.0 <= hit_right
                && cursor.1 >= hit_bottom
                && cursor.1 <= hit_top
        } else {
            false
        };

        if should_be_interactive != was_interactive {
            let app1 = app.clone();
            let app2 = app.clone();
            let val = should_be_interactive;
            let _ = app1.run_on_main_thread(move || {
                if let Some(win) = app2.get_webview_window("main") {
                    if let Ok(ns_win) = win.ns_window() {
                        use objc2::msg_send;
                        let obj = unsafe { &*(ns_win as *mut objc2::runtime::AnyObject) };
                        unsafe {
                            let _: () = msg_send![obj, setIgnoresMouseEvents: !val];
                        }
                    }
                }
            });
            was_interactive = should_be_interactive;
        }

        std::thread::sleep(Duration::from_millis(20));
    }

    // Ensure events are re-enabled when the thread exits.
    let app_exit = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(win) = app_exit.get_webview_window("main") {
            if let Ok(ns_win) = win.ns_window() {
                use objc2::msg_send;
                let obj = unsafe { &*(ns_win as *mut objc2::runtime::AnyObject) };
                unsafe {
                    let _: () = msg_send![obj, setIgnoresMouseEvents: false];
                }
            }
        }
    });
    PET_PASSTHROUGH_THREAD_ALIVE.store(false, Ordering::SeqCst);
}
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
