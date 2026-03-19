//! Pixel buffer setup and framebuffer conversion.
//!
//! Bridges the PPU's shade-index output (0–3) and the `pixels` crate's RGBA
//! pixel buffer.  [`create_pixels`] builds the GPU surface, and
//! [`shade_to_rgba`] converts a DMG framebuffer into RGBA bytes in-place —
//! writing directly into `Pixels::frame_mut()` to avoid per-frame allocation.

use crate::palette::Palette;
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use rgb_core::{SCREEN_HEIGHT, SCREEN_WIDTH};
use std::sync::Arc;
use winit::window::Window;

/// Create a [`Pixels`] instance backed by the given window.
///
/// The pixel buffer has the DMG's native resolution (160×144); the `pixels`
/// crate handles GPU-accelerated scaling to the window's actual size with
/// automatic integer-scale letterboxing.
///
/// `PresentMode::AutoVsync` is requested explicitly so frames are presented
/// at display refresh boundaries, preventing screen tearing.
pub fn create_pixels(window: Arc<Window>) -> Result<Pixels<'static>, pixels::Error> {
    let size = window.inner_size();
    let surface = SurfaceTexture::new(size.width, size.height, window);
    PixelsBuilder::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, surface)
        .present_mode(pixels::wgpu::PresentMode::Fifo)
        .build()
}

/// Convert a DMG framebuffer of shade indices (0–3) to RGBA bytes.
///
/// Writes directly into the `pixels` frame buffer (`rgba`) so no intermediate
/// allocation is needed.  Both slices must be exactly 160×144 entries long
/// (23,040 shades → 92,160 RGBA bytes).
pub fn shade_to_rgba(shades: &[u8], palette: &Palette, rgba: &mut [u8]) {
    for (i, &shade) in shades.iter().enumerate() {
        let color = palette.rgba(shade);
        let offset = i * 4;
        rgba[offset..offset + 4].copy_from_slice(&color);
    }
}
