//! `rgb_web` — browser entry point for the `rgb` Game Boy emulator.
//!
//! Compiles to a WASM module.  JavaScript calls [`start`] with the ROM bytes
//! to boot the emulator.  Rendering uses the Canvas 2D API (`putImageData`),
//! which works on every browser including iOS Safari — no WebGPU required.
//! The frame loop is driven by `requestAnimationFrame` directly, with no winit
//! dependency.

mod audio;

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, KeyboardEvent};

use rgb_core::cartridge::CartridgeKind;
use rgb_core::gameboy::DMG;
use rgb_core::Button;
use rgb_core::{SCREEN_HEIGHT, SCREEN_WIDTH};

/// Minimal audio output trait — implemented by [`audio::WebAudioSink`] and
/// the inline [`SilentSink`] fallback.
pub trait AudioSink {
    fn push_samples(&mut self, samples: &[f32]);
}

struct SilentSink;
impl AudioSink for SilentSink {
    fn push_samples(&mut self, _: &[f32]) {}
}

// Classic DMG green palette (shade index 0 = lightest, 3 = darkest).
const PALETTE: [[u8; 4]; 4] = [
    [0xE0, 0xF8, 0xD0, 0xFF],
    [0x88, 0xC0, 0x70, 0xFF],
    [0x34, 0x68, 0x56, 0xFF],
    [0x08, 0x18, 0x20, 0xFF],
];

// ── Entry point ──────────────────────────────────────────────────────────────

/// Start the emulator with the given ROM bytes.
///
/// Called from JavaScript after the user selects a ROM file.  Sets up the
/// Canvas 2D context, installs keyboard listeners on the document, and kicks
/// off the `requestAnimationFrame` game loop.
#[wasm_bindgen]
pub fn start(rom: &[u8]) {
    console_error_panic_hook::set_once();

    let cartridge = CartridgeKind::from_bytes(rom.to_vec()).expect("failed to parse ROM");
    let dmg = Rc::new(RefCell::new(DMG::new(Box::new(cartridge))));

    let canvas = canvas();
    canvas.set_width(SCREEN_WIDTH as u32);
    canvas.set_height(SCREEN_HEIGHT as u32);

    let ctx = canvas
        .get_context("2d")
        .unwrap()
        .unwrap()
        .dyn_into::<CanvasRenderingContext2d>()
        .unwrap();

    let audio: Box<dyn AudioSink> = match audio::WebAudioSink::open() {
        Some(sink) => Box::new(sink),
        None => Box::new(SilentSink),
    };

    install_keyboard_listeners(Rc::clone(&dmg));
    start_raf_loop(dmg, ctx, audio);
}

// ── RAF game loop ─────────────────────────────────────────────────────────────

/// Kick off the `requestAnimationFrame` loop.
///
/// Runs one DMG frame per callback, paced against the Game Boy's ~59.73 Hz
/// frame period.  The deadline advances by exactly `FRAME_MS` each time a
/// frame runs (rather than snapping to the current timestamp), so the
/// long-run average stays at 59.73 fps even on 60 or 120 Hz displays.
///
/// On a 60 Hz display this runs every RAF; it skips ~1 frame in every 232 to
/// shed the small 60 Hz vs 59.73 Hz surplus — identical to the native
/// `FramePacer` logic.  On 120 Hz it runs every other RAF.
fn start_raf_loop(
    dmg: Rc<RefCell<DMG>>,
    ctx: CanvasRenderingContext2d,
    mut audio: Box<dyn AudioSink>,
) {
    // The RAF closure must be able to re-schedule itself.
    let raf_closure: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> =
        Rc::new(RefCell::new(None));
    let raf_closure_outer = Rc::clone(&raf_closure);

    // Game Boy frame period in milliseconds (1000 / 59.7275).
    const FRAME_MS: f64 = 16.742_706;

    // Initialise the deadline one frame in the past so the very first RAF
    // callback always runs a frame immediately.
    let deadline: Rc<RefCell<f64>> = Rc::new(RefCell::new(-FRAME_MS));

    *raf_closure_outer.borrow_mut() = Some(Closure::new(move |timestamp: f64| {
        let mut dl = deadline.borrow_mut();

        if timestamp >= *dl {
            // Advance by exactly one frame period to maintain accurate
            // long-run timing.
            *dl += FRAME_MS;

            // If we've fallen more than one frame behind (e.g. the tab was
            // backgrounded), clamp to avoid a burst of catch-up frames.
            if timestamp - *dl > FRAME_MS {
                *dl = timestamp;
            }
            drop(dl);

            let mut dmg = dmg.borrow_mut();
            dmg.step_frame();

            let samples = dmg.drain_samples();
            audio.push_samples(&samples);

            render_frame(dmg.framebuffer(), &ctx);
        }

        request_animation_frame(raf_closure.borrow().as_ref().unwrap());
    }));

    request_animation_frame(raf_closure_outer.borrow().as_ref().unwrap());
}

fn request_animation_frame(f: &Closure<dyn FnMut(f64)>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .unwrap();
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn render_frame(framebuffer: &[u8], ctx: &CanvasRenderingContext2d) {
    let mut rgba = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];
    for (i, &shade) in framebuffer.iter().enumerate() {
        let color = PALETTE[shade as usize & 0x03];
        rgba[i * 4..i * 4 + 4].copy_from_slice(&color);
    }
    if let Ok(image_data) = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
        wasm_bindgen::Clamped(&rgba),
        SCREEN_WIDTH as u32,
        SCREEN_HEIGHT as u32,
    ) {
        let _ = ctx.put_image_data(&image_data, 0.0, 0.0);
    }
}

// ── Keyboard input ────────────────────────────────────────────────────────────

/// Install keydown/keyup listeners on the document.
///
/// Listening at document level means the user doesn't need to click the
/// canvas first — the emulator responds to keyboard input immediately.
fn install_keyboard_listeners(dmg: Rc<RefCell<DMG>>) {
    let doc = web_sys::window().unwrap().document().unwrap();

    let dmg_down = Rc::clone(&dmg);
    let keydown = Closure::<dyn FnMut(KeyboardEvent)>::new(move |e: KeyboardEvent| {
        if let Some(btn) = map_key(&e.code()) {
            dmg_down.borrow_mut().press(btn);
        }
    });
    doc.add_event_listener_with_callback("keydown", keydown.as_ref().unchecked_ref())
        .unwrap();
    keydown.forget();

    let keyup = Closure::<dyn FnMut(KeyboardEvent)>::new(move |e: KeyboardEvent| {
        if let Some(btn) = map_key(&e.code()) {
            dmg.borrow_mut().release(btn);
        }
    });
    doc.add_event_listener_with_callback("keyup", keyup.as_ref().unchecked_ref())
        .unwrap();
    keyup.forget();
}

/// Map a Web `KeyboardEvent.code` string to a DMG [`Button`].
fn map_key(code: &str) -> Option<Button> {
    match code {
        "ArrowUp" => Some(Button::Up),
        "ArrowDown" => Some(Button::Down),
        "ArrowLeft" => Some(Button::Left),
        "ArrowRight" => Some(Button::Right),
        "KeyZ" => Some(Button::B),
        "KeyX" => Some(Button::A),
        "Enter" => Some(Button::Start),
        "ShiftLeft" | "ShiftRight" => Some(Button::Select),
        _ => None,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn canvas() -> HtmlCanvasElement {
    web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .get_element_by_id("rgb-canvas")
        .expect("no #rgb-canvas element")
        .dyn_into::<HtmlCanvasElement>()
        .unwrap()
}
