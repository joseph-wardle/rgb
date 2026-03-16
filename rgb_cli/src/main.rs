//! `rgb` — Game Boy emulator frontend.
//!
//! Usage: rgb <rom.gb> [--boot-rom <boot.bin>]
//!
//! Controls:
//!   Arrow keys  → D-pad
//!   Z           → B
//!   X           → A
//!   Enter       → Start
//!   Right Shift → Select
//!   Escape      → quit

use rgb_frontend::{Cartridge, CartridgeKind};
use rgb_frontend::{EmulatorConfig, SilentSink};
use std::path::PathBuf;
use std::{env, fs, process};

mod audio;

/// Audio output sample rate in Hz.  Must match the APU's SAMPLE_RATE.
const SAMPLE_RATE: u32 = 44_100;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Parse arguments ──────────────────────────────────────────────
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

    // ── Load cartridge ───────────────────────────────────────────────
    let rom = fs::read(&rom_path)?;
    let cartridge = CartridgeKind::from_bytes(rom)?;
    let title = cartridge.info().title.clone();
    let has_battery = cartridge.info().battery;
    let save_path = PathBuf::from(&rom_path).with_extension("sav");

    // Restore battery-backed RAM from a previous session.
    let save_data = if has_battery {
        fs::read(&save_path).ok()
    } else {
        None
    };

    // ── Open audio ───────────────────────────────────────────────────
    let audio: Box<dyn rgb_frontend::AudioSink> =
        match audio::NativeAudioSink::open(SAMPLE_RATE) {
            Some(sink) => Box::new(sink),
            None => {
                eprintln!("warning: no audio device available; running without sound");
                Box::new(SilentSink)
            }
        };

    // ── Build config and run ─────────────────────────────────────────
    let window_title = if title.is_empty() {
        "rgb".to_string()
    } else {
        format!("rgb — {title}")
    };

    let config = EmulatorConfig {
        cartridge: Box::new(cartridge),
        boot_rom,
        save_data,
        audio,
        title: window_title,
        scale: 4,
    };

    let result = rgb_frontend::run(config)?;

    // ── Persist save data ────────────────────────────────────────────
    if let Some(data) = result.save_data {
        if let Err(e) = fs::write(&save_path, &data) {
            eprintln!(
                "warning: could not write save file {}: {e}",
                save_path.display()
            );
        }
    }

    Ok(())
}
