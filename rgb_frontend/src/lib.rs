//! `rgb_frontend` — native desktop emulator shell for `rgb`.
//!
//! This crate provides the native desktop frontend: window management
//! (winit), GPU-accelerated rendering (pixels), input mapping, frame timing,
//! and an audio trait that platform crates implement.
//!
//! `rgb_cli` uses this crate for the native binary.  The browser build
//! (`rgb_web`) implements its own lightweight loop directly via `web_sys`,
//! but may depend on this crate for the shared [`AudioSink`] trait and
//! palette definitions.

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
