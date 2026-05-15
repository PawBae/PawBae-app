//! Mascot/sprite size constants and helpers (collapsed_x, scaling, sprite-pad).

use crate::state::{SpritePadFracs, SPRITE_PAD};

/// Compute collapsed mascot x position based on side preference.
pub(crate) fn collapsed_x(sx: f64, sw: f64, win_w: f64, position: &str, notch_offset: f64) -> f64 {
    if position == "left" {
        sx + sw / 2.0 - notch_offset - win_w
    } else {
        sx + sw / 2.0 + notch_offset
    }
}

// Bumped from 60x45 so the codex sprite-pet (rendered at ~86x93 CSS px due
// to the MINI_SPRITE_DISPLAY_MULTIPLIER=2 used in Mini.tsx) fits entirely
// inside the native window. Without the extra room the sprite gets clipped
// at the bottom/right edges of the OS-level mascot window.
pub(crate) const COLLAPSED_MASCOT_BASE_W: f64 = 96.0;
pub(crate) const COLLAPSED_MASCOT_BASE_H: f64 = 96.0;
// Vertical inset applied to the default mascot position so the sprite is
// always rendered below the macOS menu bar / notch (or the equivalent top
// chrome on Windows). Covers both notched (~38pt) and non-notched (~24pt)
// menu bars with extra breathing room.
pub(crate) const MASCOT_TOP_INSET: f64 = 120.0;
const MASCOT_SCALE_MIN: f64 = 1.0;
const MASCOT_SCALE_MAX: f64 = 3.0;
pub(crate) const LARGE_MASCOT_SIZE_MULTIPLIER: f64 = 3.0;

pub(crate) fn sanitized_mascot_scale(scale: Option<f64>) -> f64 {
    let scale = scale.unwrap_or(1.0);
    if !scale.is_finite() {
        return 1.0;
    }
    scale.max(MASCOT_SCALE_MIN).min(MASCOT_SCALE_MAX)
}

pub(crate) fn collapsed_mascot_window_size(scale: f64) -> (f64, f64) {
    (COLLAPSED_MASCOT_BASE_W * scale, COLLAPSED_MASCOT_BASE_H * scale)
}

pub(crate) fn large_collapsed_mascot_window_size(scale: f64, large_scale: f64) -> (f64, f64) {
    let lms = if large_scale.is_finite() && large_scale >= 1.0 && large_scale <= 6.0 { large_scale } else { LARGE_MASCOT_SIZE_MULTIPLIER };
    let size = 43.0 * scale * lms;
    (size, size)
}




pub(crate) fn current_sprite_pad() -> SpritePadFracs {
    SPRITE_PAD.lock().map(|g| *g).unwrap_or(SpritePadFracs {
        top: 0.40,
        right: 0.45,
        bottom: 0.30,
        left: 0.45,
        top_px: None,
        right_px: None,
        bottom_px: None,
        left_px: None,
    })
}
