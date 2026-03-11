//! MBC3 mapper (cartridge types 0x0F–0x13).
//!
//! MBC3 supports up to 2 MiB of ROM (128 banks × 16 KiB) and up to 32 KiB
//! of RAM (4 banks × 8 KiB). Unlike MBC1 there is no banking mode — the
//! lower ROM area (0x0000–0x3FFF) is always bank 0, and the RAM bank select
//! operates independently of the ROM bank.
//!
//! MBC3 also includes an optional Real-Time Clock (RTC) mapped into the
//! RAM window. Writing 0x08–0x0C to the RAM/RTC select register maps an
//! RTC register (seconds, minutes, hours, day-low, day-high/flags) into
//! 0xA000–0xBFFF instead of RAM. The RTC requires tracking wall-clock time
//! between sessions and is **not implemented here**: RTC register reads
//! return 0xFF and writes are silently ignored. Games that use the RTC for
//! time-of-day features (Pokémon Gold/Silver day/night cycle) will still
//! boot and play; only clock-sensitive events are affected.
//!
//! ## Address map
//!
//! | CPU address   | Write target                                     |
//! |---------------|--------------------------------------------------|
//! | 0x0000–0x1FFF | RAM + RTC enable (0x0A enables, else disables)   |
//! | 0x2000–0x3FFF | 7-bit ROM bank number (0 remaps to 1)            |
//! | 0x4000–0x5FFF | RAM bank (0x00–0x03) or RTC register (0x08–0x0C) |
//! | 0x6000–0x7FFF | RTC latch strobe (ignored)                       |
//! | 0xA000–0xBFFF | Cartridge RAM or RTC register (when enabled)     |

use crate::memory::Memory;

use super::{Cartridge, CartridgeInfo};

pub struct Mbc3 {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,
    info: CartridgeInfo,
    /// 7-bit ROM bank number (1–127). Bank 0 is remapped to 1 on write.
    rom_bank: u8,
    /// Selects what 0xA000–0xBFFF maps to:
    ///   0x00–0x03 → RAM bank N
    ///   0x08–0x0C → RTC register (stubbed)
    ram_bank: u8,
    ram_enabled: bool,
}

impl Mbc3 {
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
            rom_bank: 1, // power-on default
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

impl Memory for Mbc3 {
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
                match self.ram_bank {
                    0x00..=0x03 => self.read_ram(address - 0xA000),
                    0x08..=0x0C => 0xFF, // RTC register (not implemented)
                    _ => 0xFF,
                }
            }
            _ => 0xFF,
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }
            0x2000..=0x3FFF => {
                // 7-bit ROM bank; writing 0 selects bank 1 (hardware quirk).
                let bank = value & 0x7F;
                self.rom_bank = if bank == 0 { 1 } else { bank };
            }
            0x4000..=0x5FFF => {
                // Values 0x00–0x03 select a RAM bank.
                // Values 0x08–0x0C would map an RTC register; stored verbatim
                // so reads can return the appropriate stub (0xFF).
                self.ram_bank = value;
            }
            0x6000..=0x7FFF => {
                // RTC latch strobe: write 0x00 then 0x01 to snapshot the clock.
                // Ignored — RTC is not implemented.
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled {
                    return;
                }
                match self.ram_bank {
                    0x00..=0x03 => self.write_ram(address - 0xA000, value),
                    0x08..=0x0C => {} // RTC register write (not implemented)
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl Cartridge for Mbc3 {
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
