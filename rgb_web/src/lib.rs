//! `rgb_web` — browser entry point for the `rgb` Game Boy emulator.
//!
//! This crate compiles to a WASM module that JavaScript calls to start the
//! emulator.  It handles ROM loading from JS, audio via the Web Audio API,
//! and async GPU initialisation (required because `Pixels::new` is not
//! available on WASM — only `Pixels::new_async`).
//!
//! ## Usage from JavaScript
//!
//! ```js
//! import init, { start } from './rgb_web.js';
//!
//! const rom = new Uint8Array(/* ROM bytes from file picker */);
//! await init();
//! start(rom);
//! ```

mod audio;

use rgb_frontend::app::App;
use rgb_frontend::{Cartridge, CartridgeKind, EmulatorConfig, SilentSink};
use wasm_bindgen::prelude::*;

/// Start the emulator with the given ROM bytes.
///
/// Called from JavaScript after the user selects a ROM file.  This function
/// creates the emulator, opens the window (backed by a `<canvas>`), and
/// enters the main loop.  It does not return.
#[wasm_bindgen]
pub fn start(rom: &[u8]) {
    console_error_panic_hook::set_once();

    let cartridge = CartridgeKind::from_bytes(rom.to_vec())
        .expect("failed to parse ROM");

    let title = cartridge.info().title.clone();
    let window_title = if title.is_empty() {
        "rgb".to_string()
    } else {
        format!("rgb — {title}")
    };

    let audio_sink: Box<dyn rgb_frontend::AudioSink> =
        match audio::WebAudioSink::open() {
            Some(sink) => Box::new(sink),
            None => Box::new(SilentSink),
        };

    let config = EmulatorConfig {
        cartridge: Box::new(cartridge),
        boot_rom: None,
        save_data: None,
        audio: audio_sink,
        title: window_title,
        scale: 4,
    };

    let mut app = App::new(config);

    // On WASM, winit creates its own event loop and drives the application
    // via requestAnimationFrame.  The `resumed` callback creates the window
    // and the canvas element.  Pixels is initialised asynchronously after
    // the window exists — see the about_to_wait callback below.
    //
    // We use winit's built-in WASM support: it backs the Window with a
    // <canvas> element automatically.
    let event_loop = winit::event_loop::EventLoop::new()
        .expect("failed to create event loop");

    // run_app does not return on WASM (the browser event loop takes over).
    let _ = event_loop.run_app(&mut app);
}
