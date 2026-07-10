//! Mascot/sprite size constants and helpers (collapsed_x, scaling, sprite-pad).

#[cfg(target_os = "macos")]
use crate::state::{PetState, SpritePadFracs};

/// Compute collapsed mascot x position based on side preference.
#[cfg(target_os = "macos")]
pub(crate) fn collapsed_x(sx: f64, sw: f64, win_w: f64, position: &str, notch_offset: f64) -> f64 {
    if position == "left" {
        sx + sw / 2.0 - notch_offset - win_w
    } else {
        sx + sw / 2.0 + notch_offset
    }
}

// Width was bumped from 60 so the sprite-pet (rendered at ~86 CSS px wide) fits inside the
// native window with room for its evolution-aura glow on the left/right.
pub(crate) const COLLAPSED_MASCOT_BASE_W: f64 = 96.0;
// Height is TALLER than width on purpose. The collapsed window stacks its content from the
// top (`.root` is flex-column, flex-start), and when a bubble can sit above the pet the
// sprite is pushed DOWN ~48px (`.mascot-view.headroom`) to make room for it. With a square
// 96px window that push shoved a ~65px-tall sprite (yoonie/default atlas aspect ≈ 1.083)
// past the window's bottom edge, clipping the pet's legs AND the bottom of its aura glow.
// The window's top is pinned by the positioning formula (`y = sy + sh - win_h - INSET`,
// independent of win_h), so growing the height only extends the window DOWNWARD — the pet's
// on-screen position is unchanged, it just gains the room its legs + glow need. Edge/physics
// re-measure the sprite-to-window gap, so the floor still lands under the pet's feet.
pub(crate) const COLLAPSED_MASCOT_BASE_H: f64 = 128.0;
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
    scale.clamp(MASCOT_SCALE_MIN, MASCOT_SCALE_MAX)
}

pub(crate) fn collapsed_mascot_window_size(scale: f64) -> (f64, f64) {
    (
        COLLAPSED_MASCOT_BASE_W * scale,
        COLLAPSED_MASCOT_BASE_H * scale,
    )
}

pub(crate) fn large_collapsed_mascot_window_size(scale: f64, large_scale: f64) -> (f64, f64) {
    let lms = if large_scale.is_finite() && (1.0..=6.0).contains(&large_scale) {
        large_scale
    } else {
        LARGE_MASCOT_SIZE_MULTIPLIER
    };
    let size = 43.0 * scale * lms;
    (size, size)
}

#[cfg(target_os = "macos")]
pub(crate) fn current_sprite_pad(ps: &PetState) -> SpritePadFracs {
    ps.sprite_pad.lock().map(|g| *g).unwrap_or(SpritePadFracs {
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
