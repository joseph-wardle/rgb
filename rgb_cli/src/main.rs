//! `rgb` — Game Boy emulator frontend.
//!
//! Usage: rgb <rom.gb>
//!
//! Controls:
//!   Arrow keys  → D-pad
//!   Z           → B
//!   X           → A
//!   Enter       → Start
//!   Right Shift → Select
//!   Escape      → quit

use minifb::{Key, Window, WindowOptions};
use rgb_core::cartridge::{Cartridge, CartridgeKind};
use rgb_core::gameboy::DMG;
use rgb_core::{Button, SCREEN_HEIGHT, SCREEN_WIDTH};
use std::time::{Duration, Instant};
use std::{env, fs, process};

// ---------------------------------------------------------------------------
// Frame pacing
// ---------------------------------------------------------------------------

/// The DMG runs at 4,194,304 Hz (T-cycles) and draws 70,224 T-cycles per
/// frame, giving 4,194,304 / 70,224 ≈ 59.7273 frames per second.
const FRAME_DURATION: Duration = Duration::from_nanos(16_742_706);

// ---------------------------------------------------------------------------
// Key → Button mapping
// ---------------------------------------------------------------------------

/// Maps host keyboard keys to DMG joypad buttons.
///
/// All eight keys are polled every frame; the emulator is told the current
/// held state rather than individual keydown/keyup events. This matches how
/// most games read the joypad (polling FF00 rather than relying on the
/// joypad interrupt).
const KEY_MAP: &[(Key, Button)] = &[
    (Key::Right,  Button::Right),
    (Key::Left,   Button::Left),
    (Key::Up,     Button::Up),
    (Key::Down,   Button::Down),
    (Key::Z,      Button::B),
    (Key::X,      Button::A),
    (Key::Enter,  Button::Start),
    (Key::RightShift, Button::Select),
];

// ---------------------------------------------------------------------------
// Shade palette
// ---------------------------------------------------------------------------

/// Converts a framebuffer of shade indices (0–3) into a buffer of 32-bit
/// RGB pixels (0x00RRGGBB) for minifb.
///
/// The four colours approximate the original DMG screen's yellowish-green
/// phosphor. The core only emits indices; the frontend owns the palette so
/// it can be changed here without touching the emulator.
fn shade_to_rgb(framebuffer: &[u8]) -> Vec<u32> {
    const PALETTE: [u32; 4] = [
        0xE0F8D0, // shade 0: lightest (off-white green)
        0x88C070, // shade 1: light
        0x346856, // shade 2: dark
        0x081820, // shade 3: darkest
    ];
    framebuffer.iter().map(|&shade| PALETTE[shade as usize]).collect()
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- Parse arguments ----------------------------------------------------
    let rom_path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: rgb <rom.gb>");
        process::exit(1);
    });

    // --- Load cartridge -----------------------------------------------------
    let rom = fs::read(&rom_path)?;
    let cartridge = CartridgeKind::from_bytes(rom)?;
    let title = cartridge.info().title.clone();
    let mut dmg = DMG::new(Box::new(cartridge));

    // --- Open window --------------------------------------------------------
    // The window title shows the ROM name so you can tell which game is running.
    let window_title = if title.is_empty() {
        "rgb".to_string()
    } else {
        format!("rgb — {title}")
    };

    let mut window = Window::new(
        &window_title,
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        WindowOptions::default(),
    )?;

    // --- Main loop ----------------------------------------------------------
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let frame_start = Instant::now();

        // Poll host keys and forward held state to the emulator. We call
        // press/release for every button every frame rather than tracking
        // edges because games poll the joypad register rather than waiting
        // for the joypad interrupt.
        for &(key, button) in KEY_MAP {
            if window.is_key_down(key) {
                dmg.press(button);
            } else {
                dmg.release(button);
            }
        }

        // Run exactly one DMG frame (70,224 T-cycles).
        dmg.step_frame();

        // Convert shade indices to RGB pixels and blit to the window.
        let pixels = shade_to_rgb(dmg.framebuffer());
        window.update_with_buffer(&pixels, SCREEN_WIDTH, SCREEN_HEIGHT)?;

        // Sleep for the remainder of the frame budget to pace at ~59.7 Hz.
        // If the frame took longer than the budget we skip the sleep and
        // run the next frame immediately (no frame skipping).
        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_DURATION {
            std::thread::sleep(FRAME_DURATION - elapsed);
        }
    }

    Ok(())
}
