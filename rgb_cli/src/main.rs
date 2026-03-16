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
use ringbuf::traits::Producer;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use std::{env, fs, process};

mod audio;

/// Audio output sample rate in Hz.  Must match the APU's SAMPLE_RATE.
const SAMPLE_RATE: u32 = 44_100;

/// The DMG runs at 4,194,304 Hz (T-cycles) and draws 70,224 T-cycles per
/// frame, giving 4,194,304 / 70,224 ≈ 59.7273 frames per second.
const FRAME_DURATION: Duration = Duration::from_nanos(16_742_706);

/// Maps host keyboard keys to DMG joypad buttons.
///
/// All eight keys are polled every frame; the emulator is told the current
/// held state rather than individual keydown/keyup events. This matches how
/// most games read the joypad (polling FF00 rather than relying on the
/// joypad interrupt).
const KEY_MAP: &[(Key, Button)] = &[
    (Key::Right, Button::Right),
    (Key::Left, Button::Left),
    (Key::Up, Button::Up),
    (Key::Down, Button::Down),
    (Key::Z, Button::B),
    (Key::X, Button::A),
    (Key::Enter, Button::Start),
    (Key::RightShift, Button::Select),
];

/// Converts a framebuffer of shade indices (0–3) into a buffer of 32-bit
/// RGB pixels (0x00RRGGBB) for minifb.
fn shade_to_rgb(framebuffer: &[u8]) -> Vec<u32> {
    const PALETTE: [u32; 4] = [
        0xE0F8D0, // shade 0: lightest (off-white green)
        0x88C070, // shade 1: light
        0x346856, // shade 2: dark
        0x081820, // shade 3: darkest
    ];
    framebuffer
        .iter()
        .map(|&shade| PALETTE[shade as usize])
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse arguments
    let args: Vec<String> = env::args().collect();
    let rom_path = args.get(1).cloned().unwrap_or_else(|| {
        eprintln!("Usage: rgb <rom.gb> [--boot-rom <boot.bin>]");
        process::exit(1);
    });

    // Optional --boot-rom <path> argument: load a boot ROM image to run
    // before the cartridge (e.g. an open-source dmg_boot.bin).
    let boot_rom: Option<Box<[u8]>> = if let Some(pos) = args.iter().position(|a| a == "--boot-rom")
    {
        match args.get(pos + 1) {
            Some(path) => match fs::read(path) {
                Ok(bytes) => Some(bytes.into_boxed_slice()),
                Err(e) => {
                    eprintln!("error: could not read boot ROM {path}: {e}");
                    process::exit(1);
                }
            },
            None => {
                eprintln!("error: --boot-rom requires a path argument");
                process::exit(1);
            }
        }
    } else {
        None
    };

    // Load cartridge
    let rom = fs::read(&rom_path)?;
    let cartridge = CartridgeKind::from_bytes(rom)?;
    let title = cartridge.info().title.clone();
    let has_battery = cartridge.info().battery;
    let save_path = PathBuf::from(&rom_path).with_extension("sav");

    let mut dmg = match boot_rom {
        Some(rom) => DMG::new_with_boot_rom(Box::new(cartridge), rom),
        None => DMG::new(Box::new(cartridge)),
    };

    // Restore battery-backed RAM from a previous session if a save file exists.
    if has_battery && let Ok(save_bytes) = fs::read(&save_path) {
        dmg.load_save_data(&save_bytes);
    }

    // Open audio device
    let mut audio = audio::AudioOutput::open(SAMPLE_RATE);
    if audio.is_none() {
        eprintln!("warning: no audio device available; running without sound");
    }

    // Open window
    let window_title = if title.is_empty() {
        "rgb".to_string()
    } else {
        format!("rgb — {title}")
    };

    let mut window = Window::new(
        &window_title,
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        WindowOptions {
            scale: minifb::Scale::X4,
            ..WindowOptions::default()
        },
    )?;

    // Main loop
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

        // Push the audio samples produced during this frame into the ring
        // buffer so the audio callback thread can drain them.  If no audio
        // device is open the samples are simply discarded.
        if let Some(ref mut audio_out) = audio {
            for sample in dmg.drain_samples() {
                // Non-blocking push; if the buffer is full we drop the sample
                // rather than blocking the emulator thread.
                let _ = audio_out.producer.try_push(sample);
            }
        }

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

    // Persist battery-backed RAM so the next session can restore it.
    if let Some(data) = dmg.save_data()
        && let Err(e) = fs::write(&save_path, data)
    {
        eprintln!(
            "warning: could not write save file {}: {e}",
            save_path.display()
        );
    }

    Ok(())
}
