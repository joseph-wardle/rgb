//! Pixel buffer setup and framebuffer conversion.
//!
//! Bridges the PPU's shade-index output (0–3) and the platform rendering
//! backend.
//!
//! On **native** targets, [`create_pixels`] builds a wgpu-backed GPU surface
//! and [`shade_to_rgba`] fills its frame buffer directly.
//!
//! On **WASM** targets, [`render_canvas_2d`] converts the shade buffer to an
//! [`ImageData`] and paints it onto a `<canvas>` element via the Canvas 2D
//! API — no WebGPU required, works on every browser including iOS Safari.

use crate::palette::Palette;
use rgb_core::{SCREEN_HEIGHT, SCREEN_WIDTH};

// ── Native: pixels / wgpu ────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use winit::window::Window;

/// Create a [`Pixels`] instance backed by the given window (native only).
///
/// The pixel buffer has the DMG's native resolution (160×144); the `pixels`
/// crate handles GPU-accelerated scaling to the window's actual size.
#[cfg(not(target_arch = "wasm32"))]
pub fn create_pixels(window: Arc<Window>) -> Result<Pixels<'static>, pixels::Error> {
    let size = window.inner_size();
    let surface = SurfaceTexture::new(size.width, size.height, window);
    PixelsBuilder::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, surface)
        .present_mode(pixels::wgpu::PresentMode::AutoVsync)
        .build()
}

// ── WASM: Canvas 2D API ──────────────────────────────────────────────────────

/// Paint the emulator framebuffer onto a `<canvas>` using the Canvas 2D API.
///
/// `rgba` must be exactly `SCREEN_WIDTH × SCREEN_HEIGHT × 4` bytes (the
/// output of [`shade_to_rgba`]).  The image is written at pixel-perfect 1:1
/// resolution; CSS `width: 100%` + `image-rendering: pixelated` on the canvas
/// element handles the visual scale-up.
#[cfg(target_arch = "wasm32")]
pub fn render_canvas_2d(ctx: &web_sys::CanvasRenderingContext2d, rgba: &[u8]) {
    use wasm_bindgen::Clamped;
    if let Ok(image_data) = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
        Clamped(rgba),
        SCREEN_WIDTH as u32,
        SCREEN_HEIGHT as u32,
    ) {
        let _ = ctx.put_image_data(&image_data, 0.0, 0.0);
    }
}

// ── Shared ───────────────────────────────────────────────────────────────────

/// Convert a DMG framebuffer of shade indices (0–3) to RGBA bytes.
///
/// Writes directly into the provided `rgba` slice so no intermediate
/// allocation is needed.  Both slices must be exactly 160×144 entries long
/// (23,040 shades → 92,160 RGBA bytes).
pub fn shade_to_rgba(shades: &[u8], palette: &Palette, rgba: &mut [u8]) {
    for (i, &shade) in shades.iter().enumerate() {
        let color = palette.rgba(shade);
        let offset = i * 4;
        rgba[offset..offset + 4].copy_from_slice(&color);
    }
}
