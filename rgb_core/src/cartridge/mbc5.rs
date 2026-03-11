//! MBC5 mapper (cartridge types 0x19–0x1E).
//!
//! MBC5 supports up to 8 MiB of ROM (512 banks × 16 KiB) and up to 128 KiB
//! of RAM (16 banks × 8 KiB). The extra ROM capacity comes from a 9-bit bank
//! register spread across two write addresses.
//!
//! ## Key differences from MBC1 and MBC3
//!
//! - **9-bit ROM bank**: bits 0–7 go to 0x2000–0x2FFF; bit 8 (the ninth bit)
//!   goes to 0x3000–0x3FFF. This gives access to 512 × 16 KiB = 8 MiB.
//! - **Bank 0 is valid**: writing 0 to the ROM bank register selects bank 0
//!   in the upper window, not bank 1. There is no zero-remap quirk.
//! - **4-bit RAM bank**: 16 banks × 8 KiB = 128 KiB RAM maximum.
//! - **No RTC**: MBC5 has no clock hardware.
//! - **Rumble**: some MBC5 cartridges drive a rumble motor via bit 3 of the
//!   RAM bank register. That bit is masked away and ignored here.
//!
//! ## Address map
//!
//! | CPU address   | Write target                                     |
//! |---------------|--------------------------------------------------|
//! | 0x0000–0x1FFF | RAM enable (0x0A enables, else disables)          |
//! | 0x2000–0x2FFF | Lower 8 bits of the 9-bit ROM bank number        |
//! | 0x3000–0x3FFF | Bit 8 (MSB) of the ROM bank number              |
//! | 0x4000–0x5FFF | 4-bit RAM bank number (bit 3 = rumble, ignored)  |
//! | 0xA000–0xBFFF | Cartridge RAM (when enabled)                     |

use crate::memory::Memory;

use super::{Cartridge, CartridgeInfo};

pub struct Mbc5 {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,
    info: CartridgeInfo,
    /// 9-bit ROM bank number (0–511). Bank 0 is a valid selection — no remap.
    rom_bank: u16,
    /// 4-bit RAM bank number (0–15).
    ram_bank: u8,
    ram_enabled: bool,
}

impl Mbc5 {
    pub(super) fn new(rom: Vec<u8>, info: CartridgeInfo) -> Self {
        let ram = if info.ram_size == 0 {
            None
        } else {
            Some(vec![0u8; info.ram_size])
        };
        Self {
            rom,
            ram,
            info,
            rom_bank: 0,
            ram_bank: 0,
            ram_enabled: false,
        }
    }

    fn ram_bank_count(&self) -> usize {
        self.info.ram_banks.max(1)
    }

    fn read_ram(&self, offset: u16) -> u8 {
        self.ram
            .as_ref()
            .and_then(|ram| {
                let bank_size = ram.len() / self.ram_bank_count();
                let index = self.ram_bank as usize * bank_size + offset as usize;
                ram.get(index).copied()
            })
            .unwrap_or(0xFF)
    }

    fn write_ram(&mut self, offset: u16, value: u8) {
        let bank_count = self.ram_bank_count();
        if let Some(ram) = self.ram.as_mut() {
            let bank_size = ram.len() / bank_count;
            let index = self.ram_bank as usize * bank_size + offset as usize;
            if let Some(cell) = ram.get_mut(index) {
                *cell = value;
            }
        }
    }
}

impl Memory for Mbc5 {
    fn read_byte(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => {
                // Fixed ROM bank 0.
                self.rom.get(address as usize).copied().unwrap_or(0xFF)
            }
            0x4000..=0x7FFF => {
                let offset = self.rom_bank as usize * 0x4000 + (address - 0x4000) as usize;
                self.rom.get(offset).copied().unwrap_or(0xFF)
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled {
                    return 0xFF;
                }
                self.read_ram(address - 0xA000)
            }
            _ => 0xFF,
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }
            0x2000..=0x2FFF => {
                // Lower 8 bits of the ROM bank number.
                self.rom_bank = (self.rom_bank & 0x100) | value as u16;
            }
            0x3000..=0x3FFF => {
                // Bit 8 — the ninth bit of the ROM bank number.
                // Only bit 0 of the written value contributes.
                self.rom_bank = (self.rom_bank & 0x0FF) | ((value as u16 & 0x01) << 8);
            }
            0x4000..=0x5FFF => {
                // Lower 4 bits select the RAM bank.
                // Bit 3 drives the rumble motor on RUMBLE cartridges; we mask
                // it away since emulating physical vibration is out of scope.
                self.ram_bank = value & 0x0F;
            }
            0xA000..=0xBFFF => {
                if self.ram_enabled {
                    self.write_ram(address - 0xA000, value);
                }
            }
            _ => {}
        }
    }
}

impl Cartridge for Mbc5 {
    fn info(&self) -> &CartridgeInfo {
        &self.info
    }

    fn save_data(&self) -> Option<&[u8]> {
        if self.info.battery { self.ram.as_deref() } else { None }
    }

    fn load_save_data(&mut self, data: &[u8]) {
        if let Some(ram) = self.ram.as_mut() {
            let len = ram.len().min(data.len());
            ram[..len].copy_from_slice(&data[..len]);
        }
    }
}
