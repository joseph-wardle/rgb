//! `rgb_frontend` — shared emulator shell for `rgb`.
//!
//! This crate provides the platform-independent frontend: window management
//! (winit), GPU-accelerated rendering (pixels), input mapping, frame timing,
//! and an audio trait that platform crates implement.
//!
//! Consumer crates (`rgb_cli` for native, `rgb_web` for the browser) provide
//! a thin entry point that loads the ROM, creates an [`AudioSink`], and calls
//! [`run`] to hand control to the emulator loop.

pub mod app;
pub mod audio;
pub mod input;
pub mod palette;
pub mod renderer;
pub mod scaling;
pub mod timing;

pub use audio::{AudioSink, SilentSink};
pub use rgb_core::cartridge::{Cartridge, CartridgeKind};
pub use rgb_core::{Button, SCREEN_HEIGHT, SCREEN_WIDTH};

use winit::event_loop::EventLoop;

/// Everything a platform crate must provide to start the emulator.
pub struct EmulatorConfig {
    /// The loaded cartridge (ROM + mapper).
    pub cartridge: Box<dyn Cartridge>,
    /// Optional boot ROM image (e.g. dmg_boot.bin). When `None` the emulator
    /// starts in the post-boot state (PC = 0x0100).
    pub boot_rom: Option<Box<[u8]>>,
    /// Previously saved battery-backed RAM, if any.  Restored into the
    /// cartridge before the first frame.
    pub save_data: Option<Vec<u8>>,
    /// Audio output backend.  Use [`SilentSink`] when no device is available.
    pub audio: Box<dyn AudioSink>,
    /// Window title (typically "rgb — <game title>").
    pub title: String,
    /// Integer scale factor for the initial window size (e.g. 4 → 640×576).
    pub scale: u32,
    /// On WASM: if provided, winit attaches to the existing `<canvas>` with
    /// this DOM id rather than creating a new element and appending it to
    /// `document.body`.  Use this to place the emulator canvas at a specific
    /// location on the page (e.g. inside a portfolio article embed).
    #[cfg(target_arch = "wasm32")]
    pub canvas_id: Option<String>,
}

/// Returned by [`run`] after the window is closed so the caller can persist
/// battery-backed RAM.
pub struct EmulatorResult {
    /// Battery-backed RAM contents, or `None` if the cartridge has no battery.
    pub save_data: Option<Vec<u8>>,
}

/// Start the emulator and run until the window is closed.
///
/// This function creates the winit event loop, opens the window, and enters
/// the frame loop.  It returns when the user closes the window (or presses
/// Escape), providing an [`EmulatorResult`] with any save data to persist.
pub fn run(config: EmulatorConfig) -> Result<EmulatorResult, Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = app::App::new(config);
    event_loop.run_app(&mut app)?;

    Ok(app.result.unwrap_or(EmulatorResult { save_data: None }))
}
