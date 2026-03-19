//! The emulator main loop, driven by winit's event system.
//!
//! [`App`] implements winit's [`ApplicationHandler`] trait.  Each callback
//! maps to a phase of the per-frame emulator cycle:
//!
//! 1. **`resumed`** — create the window and GPU surface (once, on startup).
//! 2. **`window_event(KeyboardInput)`** — forward key presses/releases to
//!    the DMG joypad.
//! 3. **`window_event(RedrawRequested)`** — run one frame: step the CPU,
//!    push audio, convert the framebuffer, and render.
//! 4. **`about_to_wait`** — pace the frame and request the next redraw.
//!
//! This is intentionally structured like a readable main loop rather than
//! a deeply layered abstraction.

use crate::EmulatorConfig;
use crate::EmulatorResult;
use crate::audio::AudioSink;
use crate::input;
use crate::palette::{self, Palette};
use crate::renderer;
use crate::timing::FramePacer;
use pixels::Pixels;
use rgb_core::gameboy::DMG;
use rgb_core::{SCREEN_HEIGHT, SCREEN_WIDTH};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// Emulator application state, owned by the winit event loop.
///
/// On native, call [`crate::run`] which creates and drives this internally.
pub struct App {
    /// The DMG emulator instance.
    dmg: DMG,
    /// Audio output backend (cpal on native).
    audio: Box<dyn AudioSink>,
    /// Palette used to convert shade indices to RGBA.
    palette: Palette,
    /// Integer scale factor for the initial window size (e.g. 4 → 640×576).
    scale: u32,
    /// Window title.
    title: String,

    // Initialised in `resumed` — `None` until the first resume event.
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,

    /// Frame-rate limiter.
    pacer: FramePacer,
    /// Save data captured on exit for the caller.
    pub result: Option<EmulatorResult>,
}

impl App {
    /// Build the app from an [`EmulatorConfig`].
    ///
    /// The window and GPU surface are *not* created here — they are deferred
    /// to the first `resumed` callback, as recommended by winit for
    /// cross-platform compatibility.
    pub fn new(config: EmulatorConfig) -> Self {
        let mut dmg = match config.boot_rom {
            Some(rom) => DMG::new_with_boot_rom(config.cartridge, rom),
            None => DMG::new(config.cartridge),
        };

        if let Some(ref save) = config.save_data {
            dmg.load_save_data(save);
        }

        Self {
            dmg,
            audio: config.audio,
            palette: palette::CLASSIC_GREEN,
            scale: config.scale,
            title: config.title,
            window: None,
            pixels: None,
            pacer: FramePacer::new(),
            result: None,
        }
    }

    /// Return a reference to the window, if it has been created.
    pub fn window(&self) -> Option<&Arc<Window>> {
        self.window.as_ref()
    }
}

impl ApplicationHandler for App {
    /// Create the window and initialise the GPU pixel buffer.
    ///
    /// Deferred to `resumed` because some platforms (Android) do not allow
    /// surface creation before this event fires.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return; // already initialised (redundant resume)
        }

        let size = LogicalSize::new(
            SCREEN_WIDTH as u32 * self.scale,
            SCREEN_HEIGHT as u32 * self.scale,
        );

        let attrs = Window::default_attributes()
            .with_title(&self.title)
            .with_inner_size(size)
            .with_min_inner_size(LogicalSize::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32));

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );

        let pixels =
            renderer::create_pixels(Arc::clone(&window)).expect("failed to create pixel buffer");
        self.pixels = Some(pixels);
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            // ── Input ────────────────────────────────────────────────
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state,
                        repeat: false,
                        ..
                    },
                ..
            } => {
                // Escape quits the emulator.
                if key == KeyCode::Escape && state == ElementState::Pressed {
                    self.result = Some(EmulatorResult {
                        save_data: self.dmg.save_data().map(|d| d.to_vec()),
                    });
                    event_loop.exit();
                    return;
                }

                // Map host key → DMG button and forward the held state.
                if let Some(button) = input::map_key(key) {
                    match state {
                        ElementState::Pressed => self.dmg.press(button),
                        ElementState::Released => self.dmg.release(button),
                    }
                }
            }

            // ── Close ────────────────────────────────────────────────
            WindowEvent::CloseRequested => {
                self.result = Some(EmulatorResult {
                    save_data: self.dmg.save_data().map(|d| d.to_vec()),
                });
                event_loop.exit();
            }

            // ── Resize ───────────────────────────────────────────────
            WindowEvent::Resized(size) => {
                if size.width > 0
                    && size.height > 0
                    && let Some(ref mut pixels) = self.pixels
                {
                    let _ = pixels.resize_surface(size.width, size.height);
                }
            }

            // ── Render ───────────────────────────────────────────────
            WindowEvent::RedrawRequested => {
                if let Some(ref mut pixels) = self.pixels {
                    self.pacer.begin_frame();
                    self.dmg.step_frame();

                    let samples = self.dmg.drain_samples();
                    self.audio.push_samples(&samples);

                    renderer::shade_to_rgba(
                        self.dmg.framebuffer(),
                        &self.palette,
                        pixels.frame_mut(),
                    );
                    if let Err(e) = pixels.render() {
                        eprintln!("render error: {e}");
                        event_loop.exit();
                    }
                }
            }

            _ => {}
        }
    }

    /// After all events are processed, pace the frame and request the next
    /// redraw.  On native this sleeps for the remainder of the frame budget.
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.pacer.wait();
        if let Some(ref window) = self.window {
            window.request_redraw();
        }
    }
}
