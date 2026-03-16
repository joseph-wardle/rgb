//! The emulator main loop, driven by winit's event system.
//!
//! [`App`] implements winit's [`ApplicationHandler`] trait.  Each callback
//! maps to a phase of the per-frame emulator cycle:
//!
//! 1. **`resumed`** — create the window and rendering surface (once, on
//!    startup).  On native this initialises the wgpu-backed [`Pixels`] buffer;
//!    on WASM it acquires a Canvas 2D rendering context from the pre-placed
//!    `<canvas>` element.
//! 2. **`window_event(KeyboardInput)`** — forward key presses/releases to
//!    the DMG joypad.
//! 3. **`window_event(RedrawRequested)`** — run one frame: step the CPU,
//!    push audio, convert the framebuffer, and render.
//! 4. **`about_to_wait`** — pace the frame and request the next redraw.

use crate::EmulatorConfig;
use crate::EmulatorResult;
use crate::audio::AudioSink;
use crate::input;
use crate::palette::{self, Palette};
use crate::renderer;
use crate::timing::FramePacer;
use rgb_core::gameboy::DMG;
use rgb_core::{SCREEN_HEIGHT, SCREEN_WIDTH};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

#[cfg(not(target_arch = "wasm32"))]
use pixels::Pixels;

#[cfg(target_arch = "wasm32")]
use web_sys::CanvasRenderingContext2d;

/// Emulator application state, owned by the winit event loop.
///
/// On native, call [`crate::run`] which creates and drives this internally.
/// On WASM, `rgb_web` creates an `App` directly and hands it to the winit
/// event loop, which drives the frame loop via `requestAnimationFrame`.
pub struct App {
    /// The DMG emulator instance.
    dmg: DMG,
    /// Audio output backend (cpal on native, Web Audio on WASM).
    audio: Box<dyn AudioSink>,
    /// Palette used to convert shade indices to RGBA.
    palette: Palette,
    /// Integer scale factor for the initial native window size (e.g. 4 → 640×576).
    /// Unused on WASM — CSS handles display scaling there.
    scale: u32,
    /// Window title.
    title: String,

    // Initialised in `resumed` — `None` until the first resume event.
    window: Option<Arc<Window>>,

    /// Native: wgpu-backed pixel buffer (initialised synchronously in `resumed`).
    #[cfg(not(target_arch = "wasm32"))]
    pixels: Option<Pixels<'static>>,

    /// WASM: Canvas 2D rendering context (acquired synchronously in `resumed`).
    /// Rendering goes through `putImageData` — no WebGPU required.
    #[cfg(target_arch = "wasm32")]
    canvas_ctx: Option<CanvasRenderingContext2d>,

    /// WASM: DOM id of the `<canvas>` element to attach to.
    #[cfg(target_arch = "wasm32")]
    canvas_id: Option<String>,

    /// Frame-rate limiter.
    pacer: FramePacer,
    /// Save data captured on exit for the caller.
    pub result: Option<EmulatorResult>,
}

impl App {
    /// Build the app from an [`EmulatorConfig`].
    ///
    /// The window and rendering surface are *not* created here — they are
    /// deferred to the first `resumed` callback, as recommended by winit.
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
            #[cfg(not(target_arch = "wasm32"))]
            pixels: None,
            #[cfg(target_arch = "wasm32")]
            canvas_ctx: None,
            #[cfg(target_arch = "wasm32")]
            canvas_id: config.canvas_id,
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
    /// Create the window and initialise the rendering surface.
    ///
    /// On **native**, `Pixels` is created synchronously here.
    ///
    /// On **WASM**, we attach winit to the pre-placed `<canvas id="rgb-canvas">`
    /// and acquire a Canvas 2D rendering context from it.  No async GPU
    /// initialisation is required — `CanvasRenderingContext2d` is always
    /// available and works on every browser including iOS Safari.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return; // already initialised (redundant resume)
        }

        let size = LogicalSize::new(
            SCREEN_WIDTH as u32 * self.scale,
            SCREEN_HEIGHT as u32 * self.scale,
        );

        #[allow(unused_mut)]
        let mut attrs = Window::default_attributes()
            .with_title(&self.title)
            .with_inner_size(size)
            .with_min_inner_size(LogicalSize::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32));

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;

            let canvas = self.canvas_id.as_deref().and_then(|id| {
                web_sys::window()
                    .and_then(|w| w.document())
                    .and_then(|d| d.get_element_by_id(id))
                    .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            });

            if let Some(ref c) = canvas {
                // Set the canvas pixel dimensions to the DMG's native
                // resolution.  CSS (width: 100%, image-rendering: pixelated)
                // handles the visual scale-up — no wgpu scaling needed.
                c.set_width(SCREEN_WIDTH as u32);
                c.set_height(SCREEN_HEIGHT as u32);

                // Acquire the 2D context for putImageData-based rendering.
                self.canvas_ctx = c
                    .get_context("2d")
                    .ok()
                    .flatten()
                    .and_then(|o| o.dyn_into::<CanvasRenderingContext2d>().ok());
            }

            if canvas.is_some() {
                attrs = attrs.with_canvas(canvas);
            } else {
                attrs = attrs.with_append(true);
            }
        }

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );

        #[cfg(not(target_arch = "wasm32"))]
        {
            let pixels = renderer::create_pixels(Arc::clone(&window))
                .expect("failed to create pixel buffer");
            self.pixels = Some(pixels);
        }

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
                if key == KeyCode::Escape && state == ElementState::Pressed {
                    self.result = Some(EmulatorResult {
                        save_data: self.dmg.save_data().map(|d| d.to_vec()),
                    });
                    event_loop.exit();
                    return;
                }

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

            // ── Resize (native only) ──────────────────────────────────
            // On WASM the canvas pixel dimensions are fixed at 160×144;
            // CSS handles visual scaling so no surface resize is needed.
            WindowEvent::Resized(size) => {
                #[cfg(not(target_arch = "wasm32"))]
                if size.width > 0 && size.height > 0 {
                    if let Some(ref mut pixels) = self.pixels {
                        let _ = pixels.resize_surface(size.width, size.height);
                    }
                }
                #[cfg(target_arch = "wasm32")]
                let _ = size;
            }

            // ── Render ───────────────────────────────────────────────
            WindowEvent::RedrawRequested => {
                // On WASM, requestAnimationFrame fires at the monitor refresh
                // rate (60/120 Hz), which is faster than the Game Boy's 59.7 Hz.
                // Skip emulation until a full frame period has elapsed.
                // On native is_frame_due() always returns true; wait() sleeps.
                if !self.pacer.is_frame_due() {
                    return;
                }

                self.pacer.begin_frame();

                // Run exactly one DMG frame (70,224 T-cycles).
                self.dmg.step_frame();

                // Push audio samples to the platform backend.
                let samples = self.dmg.drain_samples();
                self.audio.push_samples(&samples);

                // ── Native render via pixels/wgpu ─────────────────────
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(ref mut pixels) = self.pixels {
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

                // ── WASM render via Canvas 2D API ─────────────────────
                #[cfg(target_arch = "wasm32")]
                if let Some(ref ctx) = self.canvas_ctx {
                    let mut rgba = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
                    renderer::shade_to_rgba(self.dmg.framebuffer(), &self.palette, &mut rgba);
                    renderer::render_canvas_2d(ctx, &rgba);
                }
            }

            _ => {}
        }
    }

    /// After all events are processed, pace the frame and request the next
    /// redraw.  On native this sleeps for the remainder of the frame budget;
    /// on WASM the sleep is a no-op (winit uses requestAnimationFrame).
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.pacer.wait();
        if let Some(ref window) = self.window {
            window.request_redraw();
        }
    }
}
