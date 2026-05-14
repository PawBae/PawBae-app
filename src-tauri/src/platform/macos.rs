//! macOS-specific helpers. Gated by the outer `#[cfg(target_os = "macos")]` in `platform/mod.rs`.

use crate::platform::common::AppWindowInfo;

/// Get the notch half-width (distance from screen center to notch edge) using
/// macOS 12+ `auxiliaryTopRightArea` API. Falls back to 80pt for older systems
/// or screens without a notch (external displays, pre-notch Macs).
pub(crate) unsafe fn get_notch_offset(screen: *mut objc2::runtime::AnyObject) -> f64 {
    use objc2::msg_send;
    use objc2_foundation::NSRect;

    if screen.is_null() { return 80.0; }
    let sel = objc2::runtime::Sel::register(c"auxiliaryTopRightArea");
    let responds: bool = msg_send![&*screen, respondsToSelector: sel];
    if responds {
        let right_area: NSRect = msg_send![&*screen, auxiliaryTopRightArea];
        if right_area.size.width > 0.0 {
            let frame: NSRect = msg_send![&*screen, frame];
            let center_x = frame.origin.x + frame.size.width / 2.0;
            let half_w = right_area.origin.x - center_x;
            if half_w > 10.0 { return half_w; }
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
        pub fn CFStringCreateWithCString(alloc: CFTypeRef, cstr: *const c_char, enc: u32) -> CFStringRef;
        pub fn CFStringGetCString(s: CFStringRef, buf: *mut c_char, bufsz: CFIndex, enc: u32) -> bool;
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
    if key_cf.is_null() { return None; }
    let val = cg_window::CFDictionaryGetValue(dict, key_cf);
    cg_window::CFRelease(key_cf);
    if val.is_null() { return None; }
    let mut out: f64 = 0.0;
    let ok = cg_window::CFNumberGetValue(
        val,
        cg_window::NUMBER_DOUBLE_TYPE,
        &mut out as *mut f64 as *mut std::ffi::c_void,
    );
    if ok { Some(out) } else { None }
}

/// Read a top-level i32 value (such as kCGWindowLayer or kCGWindowAlpha
/// rounded to integer). Returns None if the key is absent or the bridge
/// fails.
unsafe fn cf_dict_get_i32(dict: cg_window::CFDictionaryRef, key: &str) -> Option<i32> {
    let key_cf = cf_string_from(key);
    if key_cf.is_null() { return None; }
    let val = cg_window::CFDictionaryGetValue(dict, key_cf);
    cg_window::CFRelease(key_cf);
    if val.is_null() { return None; }
    let mut out: i32 = 0;
    let ok = cg_window::CFNumberGetValue(
        val,
        cg_window::NUMBER_SINT32_TYPE,
        &mut out as *mut i32 as *mut std::ffi::c_void,
    );
    if ok { Some(out) } else { None }
}

unsafe fn cf_dict_get_string(dict: cg_window::CFDictionaryRef, key: &str) -> Option<String> {
    let key_cf = cf_string_from(key);
    if key_cf.is_null() { return None; }
    let val = cg_window::CFDictionaryGetValue(dict, key_cf);
    cg_window::CFRelease(key_cf);
    if val.is_null() { return None; }
    let len = cg_window::CFStringGetLength(val);
    if len <= 0 { return Some(String::new()); }
    let bufsz = (len as usize) * 4 + 1; // UTF-8 worst case
    let mut buf: Vec<i8> = vec![0; bufsz];
    let ok = cg_window::CFStringGetCString(
        val,
        buf.as_mut_ptr(),
        bufsz as cg_window::CFIndex,
        cg_window::STRING_ENCODING_UTF8,
    );
    if !ok { return None; }
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
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2::msg_send;
    use objc2_foundation::NSRect;
    use std::sync::atomic::{AtomicBool, Ordering};

    static DUMPED_ONCE: AtomicBool = AtomicBool::new(false);
    let dump_now = !DUMPED_ONCE.swap(true, Ordering::Relaxed);

    let cls = AnyClass::get(c"NSScreen")?;
    let main_screen: *mut AnyObject = msg_send![cls, mainScreen];
    if main_screen.is_null() { return None; }
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
        if dict.is_null() { continue; }
        let owner = cf_dict_get_string(dict, "kCGWindowOwnerName").unwrap_or_default();
        let layer = cf_dict_get_i32(dict, "kCGWindowLayer").unwrap_or(0);
        let alpha = cf_dict_get_double(dict, "kCGWindowAlpha").unwrap_or(0.0);
        let bounds_key = cf_string_from("kCGWindowBounds");
        if bounds_key.is_null() { continue; }
        let bounds = cg_window::CFDictionaryGetValue(dict, bounds_key);
        cg_window::CFRelease(bounds_key);
        if bounds.is_null() { continue; }
        let x = cf_dict_get_double(bounds, "X").unwrap_or(0.0);
        let y_cg = cf_dict_get_double(bounds, "Y").unwrap_or(0.0);
        let w = cf_dict_get_double(bounds, "Width").unwrap_or(0.0);
        let h = cf_dict_get_double(bounds, "Height").unwrap_or(0.0);

        // First-call diagnostic: dump everything visible so we can
        // identify the Dock row by hand from real data.
        if dump_now {
            log::info!(
                "[dock/dump] owner={:?} layer={} alpha={:.2} x={} y_cg={} w={} h={}",
                owner, layer, alpha, x, y_cg, w, h,
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
        if !is_dock_owner { continue; }
        if w < 30.0 || h < 30.0 { continue; }
        let wallpaper_like = w >= main_w * 0.6 && h >= main_h * 0.6;
        if wallpaper_like { continue; }
        // Position gate — the Dock is always at a screen edge. On
        // macOS bottom-up coords (`y_cg + h ≈ main_h` = bottom edge,
        // `y_cg ≈ 0` = top edge of main screen). Reject the menu
        // bar, which lives at `y_cg ≈ 0` and would otherwise pass
        // the strip-shape filter and be mistaken for a Dock.
        let touches_bottom = (y_cg + h - main_h).abs() < 2.0;
        let touches_left = x.abs() < 2.0 && h > w; // tall strip at x≈0
        let touches_right = (x + w - main_w).abs() < 2.0 && h > w;
        if !(touches_bottom || touches_left || touches_right) { continue; }

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
        count, candidate_count, chosen,
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
        if at.elapsed() < TTL { return val; }
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
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2::msg_send;
    use objc2_foundation::NSRect;

    // Main-screen frame is needed to flip CGWindowList's top-down y to
    // Cocoa bottom-up y. Multi-display refinement is deferred — the
    // mascot itself is currently main-screen-centric per `get_pet_floor_info`.
    let cls = AnyClass::get(c"NSScreen")?;
    let main: *mut AnyObject = msg_send![cls, mainScreen];
    if main.is_null() { return None; }
    let mframe: NSRect = msg_send![&*main, frame];
    let main_h = mframe.size.height;
    let main_origin_y = mframe.origin.y;

    let our_pid = std::process::id() as i32;

    let list = cg_window::CGWindowListCopyWindowInfo(
        cg_window::OPTION_ON_SCREEN_ONLY,
        cg_window::NULL_WINDOW_ID,
    );
    if list.is_null() { return None; }
    let count = cg_window::CFArrayGetCount(list);

    let mut best: Option<AppWindowInfo> = None;
    for i in 0..count {
        let dict = cg_window::CFArrayGetValueAtIndex(list, i) as cg_window::CFDictionaryRef;
        if dict.is_null() { continue; }

        let layer = cf_dict_get_i32(dict, "kCGWindowLayer").unwrap_or(99);
        if layer != 0 { continue; }
        let alpha = cf_dict_get_double(dict, "kCGWindowAlpha").unwrap_or(0.0);
        if alpha < 0.5 { continue; }
        let owner_pid = cf_dict_get_i32(dict, "kCGWindowOwnerPID").unwrap_or(0);
        if owner_pid == our_pid { continue; }
        let owner = cf_dict_get_string(dict, "kCGWindowOwnerName").unwrap_or_default();
        let owner_lower = owner.to_ascii_lowercase();
        if owner_lower == "dock"
            || owner_lower == "windowserver"
            || owner_lower == "window server" { continue; }

        let bounds_key = cf_string_from("kCGWindowBounds");
        if bounds_key.is_null() { continue; }
        let bounds = cg_window::CFDictionaryGetValue(dict, bounds_key);
        cg_window::CFRelease(bounds_key);
        if bounds.is_null() { continue; }
        let x = cf_dict_get_double(bounds, "X").unwrap_or(0.0);
        let y_cg = cf_dict_get_double(bounds, "Y").unwrap_or(0.0);
        let w = cf_dict_get_double(bounds, "Width").unwrap_or(0.0);
        let h = cf_dict_get_double(bounds, "Height").unwrap_or(0.0);
        // Tooltips, popovers and completion menus are too small to be
        // useful platforms; also skip degenerate w/h <= 0.
        if w < 200.0 || h < 120.0 { continue; }

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
        if at.elapsed() < TTL { Some(val.clone()) } else { None }
    }

    pub fn store(val: Option<AppWindowInfo>) {
        if let Ok(mut c) = CACHE.lock() {
            *c = Some((Instant::now(), val));
        }
    }
}
