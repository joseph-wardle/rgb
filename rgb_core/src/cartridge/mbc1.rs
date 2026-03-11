//! MBC1 mapper (cartridge types 0x01–0x03).
//!
//! MBC1 supports up to 2 MiB of ROM (128 banks × 16 KiB) and up to 32 KiB
//! of RAM (4 banks × 8 KiB). A banking mode switch controls whether the
//! upper two address bits are applied to ROM or to RAM.
//!
//! ## Address map
//!
//! | CPU address  | Write target                                    |
//! |--------------|--------------------------------------------------|
//! | 0x0000–0x1FFF | RAM enable (write 0x0A to enable, else disable) |
//! | 0x2000–0x3FFF | Lower 5 bits of ROM bank number                 |
//! | 0x4000–0x5FFF | Upper 2 bits (ROM in mode 0, RAM in mode 1)     |
//! | 0x6000–0x7FFF | Banking mode: 0 = ROM mode, 1 = RAM mode        |
//! | 0xA000–0xBFFF | Cartridge RAM (when enabled)                    |

use crate::memory::Memory;

use super::{Cartridge, CartridgeError, CartridgeInfo};

/// The two banking modes MBC1 can operate in.
///
/// In **ROM mode** (default) all seven bank bits drive the upper ROM window,
/// and the RAM is locked to bank 0. This is what most games use.
///
/// In **RAM mode** the upper two bits drive the RAM bank select and also
/// control which 512 KiB "half" of ROM the lower window maps into — useful
/// for large-ROM cartridges but rare in practice.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BankingMode {
    Rom,
    Ram,
}

pub struct Mbc1 {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,
    info: CartridgeInfo,
    rom_bank_low5: u8,    // lower 5 bits of ROM bank; written to 0x2000–0x3FFF
    rom_bank_high2: u8,   // upper 2 bits; written to 0x4000–0x5FFF
    ram_enabled: bool,
    banking_mode: BankingMode,
}

impl Mbc1 {
    pub(super) fn new(rom: Vec<u8>, info: CartridgeInfo) -> Result<Self, CartridgeError> {
        let ram = if info.ram_size == 0 {
            None
        } else {
            Some(vec![0u8; info.ram_size])
        };
        Ok(Self {
            rom,
            ram,
            info,
            rom_bank_low5: 1, // power-on default: bank 1
            rom_bank_high2: 0,
            ram_enabled: false,
            banking_mode: BankingMode::Rom,
        })
    }

    // --- Bank number computation ---

    fn rom_bank_count(&self) -> usize {
        self.info.rom_banks.max(1)
    }

    fn ram_bank_count(&self) -> usize {
        self.info.ram_banks.max(1)
    }

    /// Bank mapped into 0x0000–0x3FFF. Fixed to 0 in ROM mode; in RAM mode
    /// the high two bits shift which 512 KiB block the lower window reads.
    fn lower_rom_bank(&self) -> usize {
        match self.banking_mode {
            BankingMode::Rom => 0,
            BankingMode::Ram => self.wrap_rom((self.rom_bank_high2 as usize) << 5),
        }
    }

    /// Bank mapped into 0x4000–0x7FFF. In ROM mode uses all 7 bits; in RAM
    /// mode only the low 5 bits are used (high 2 bits feed the RAM bank).
    fn upper_rom_bank(&self) -> usize {
        let bank = match self.banking_mode {
            BankingMode::Rom => (self.rom_bank_high2 as usize) << 5 | self.rom_bank_low5 as usize,
            BankingMode::Ram => self.rom_bank_low5 as usize,
        };
        let bank = self.wrap_rom(bank);
        // Bank 0 in the upper window maps to bank 1 (hardware quirk).
        if bank == 0 && self.rom_bank_count() > 1 { 1 } else { bank }
    }

    fn current_ram_bank(&self) -> usize {
        match self.banking_mode {
            BankingMode::Rom => 0,
            BankingMode::Ram => (self.rom_bank_high2 as usize) % self.ram_bank_count(),
        }
    }

    fn wrap_rom(&self, bank: usize) -> usize {
        bank % self.rom_bank_count()
    }

    // --- Memory access helpers ---

    fn read_rom(&self, bank: usize, offset: u16) -> u8 {
        let index = bank * 0x4000 + offset as usize;
        self.rom.get(index).copied().unwrap_or(0xFF)
    }

    fn read_ram(&self, bank: usize, offset: u16) -> u8 {
        self.ram
            .as_ref()
            .and_then(|ram| {
                let bank_size = ram.len() / self.ram_bank_count().max(1);
                ram.get(bank * bank_size + offset as usize).copied()
            })
            .unwrap_or(0xFF)
    }

    fn write_ram(&mut self, bank: usize, offset: u16, value: u8) {
        let bank_count = self.ram_bank_count().max(1);
        if let Some(ram) = self.ram.as_mut() {
            let bank_size = ram.len() / bank_count;
            if let Some(cell) = ram.get_mut(bank * bank_size + offset as usize) {
                *cell = value;
            }
        }
    }
}

impl Memory for Mbc1 {
    fn read_byte(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => self.read_rom(self.lower_rom_bank(), address),
            0x4000..=0x7FFF => self.read_rom(self.upper_rom_bank(), address - 0x4000),
            0xA000..=0xBFFF => {
                if !self.ram_enabled {
                    return 0xFF;
                }
                self.read_ram(self.current_ram_bank(), address - 0xA000)
            }
            _ => 0xFF,
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                // 0x0A enables RAM; any other nibble disables it.
                self.ram_enabled = (value & 0x0F) == 0x0A && self.ram.is_some();
            }
            0x2000..=0x3FFF => {
                // Lower 5 bits of ROM bank; 0 is treated as 1 (hardware quirk).
                let v = value & 0x1F;
                self.rom_bank_low5 = if v == 0 { 1 } else { v };
            }
            0x4000..=0x5FFF => {
                self.rom_bank_high2 = value & 0x03;
            }
            0x6000..=0x7FFF => {
                self.banking_mode = if value & 0x01 == 0 {
                    BankingMode::Rom
                } else {
                    BankingMode::Ram
                };
            }
            0xA000..=0xBFFF => {
                if self.ram_enabled {
                    let bank = self.current_ram_bank();
                    self.write_ram(bank, address - 0xA000, value);
                }
            }
            _ => {}
        }
    }
}

impl Cartridge for Mbc1 {
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
