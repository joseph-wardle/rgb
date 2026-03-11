//! ROM-only mapper (cartridge type 0x00).
//!
//! The simplest possible cartridge: up to 32 KiB of ROM, no RAM, no banking.
//! The CPU sees the full ROM directly at 0x0000–0x7FFF. Reads outside the
//! ROM or at the cartridge RAM window (0xA000–0xBFFF) return open-bus 0xFF.
//! Writes are silently ignored.

use crate::memory::Memory;

use super::{Cartridge, CartridgeInfo};

pub struct RomOnly {
    rom: Vec<u8>,
    info: CartridgeInfo,
}

impl RomOnly {
    pub(super) fn new(rom: Vec<u8>, info: CartridgeInfo) -> Self {
        Self { rom, info }
    }
}

impl Memory for RomOnly {
    fn read_byte(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x7FFF => self.rom.get(address as usize).copied().unwrap_or(0xFF),
            0xA000..=0xBFFF => 0xFF, // no RAM
            _ => 0xFF,
        }
    }

    fn write_byte(&mut self, _address: u16, _value: u8) {
        // ROM-only cartridges ignore all writes.
    }
}

impl Cartridge for RomOnly {
    fn info(&self) -> &CartridgeInfo {
        &self.info
    }
}
