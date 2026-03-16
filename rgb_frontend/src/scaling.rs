//! Integer scaling and viewport calculations.
//!
//! The DMG screen is 160×144.  When the host window is resized, the viewport
//! is the largest centred rectangle that maintains an integer scale factor.
//! The `pixels` crate handles the actual GPU blit; this module provides the
//! math for computing surface dimensions and the largest fitting scale.

use rgb_core::{SCREEN_HEIGHT, SCREEN_WIDTH};

/// Compute the largest integer scale factor that fits inside the given
/// window dimensions while preserving the DMG aspect ratio.
///
/// Returns at least 1, so the screen is always visible.
pub fn fit_scale(window_width: u32, window_height: u32) -> u32 {
    let scale_x = window_width / SCREEN_WIDTH as u32;
    let scale_y = window_height / SCREEN_HEIGHT as u32;
    scale_x.min(scale_y).max(1)
}
