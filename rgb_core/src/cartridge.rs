//! Cartridge mappers for the DMG Game Boy.
//!
//! The behaviour documented here closely follows <https://gbdev.io/pandocs/#the-cartridge-header>
//! and the accompanying mapper chapters. Each mapper encapsulates its own banking rules so the
//! rest of the emulator can treat a cartridge as a simple chunk of addressable memory.

use crate::memory::Memory;
use std::fmt;

/// Errors produced while parsing or constructing a cartridge.
#[derive(Debug)]
pub enum CartridgeError {
    RomTooSmall(usize),
    UnsupportedCartridgeType(u8),
    UnsupportedRomSizeCode(u8),
    UnsupportedRamSizeCode(u8),
}

impl fmt::Display for CartridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CartridgeError::RomTooSmall(len) => write!(f, "ROM image too small ({len} bytes)"),
            CartridgeError::UnsupportedCartridgeType(kind) => {
                write!(f, "cartridge type 0x{kind:02X} is not supported")
            }
            CartridgeError::UnsupportedRomSizeCode(code) => {
                write!(f, "ROM size code 0x{code:02X} is not supported")
            }
            CartridgeError::UnsupportedRamSizeCode(code) => {
                write!(f, "RAM size code 0x{code:02X} is not supported")
            }
        }
    }
}

impl std::error::Error for CartridgeError {}

/// Metadata extracted from the header. Helpful for debugging and future UI hooks.
#[derive(Debug, Clone)]
pub struct CartridgeInfo {
    pub title: String,
    pub mapper: MapperKind,
    pub rom_size: usize,
    pub ram_size: usize,
    pub rom_banks: usize,
    pub ram_banks: usize,
}

/// Trait implemented by all mapper backends. Currently this adds metadata access on top of the
/// generic [`Memory`] interface the rest of the emulator expects.
pub trait Cartridge: Memory {
    fn info(&self) -> &CartridgeInfo;
}

/// Cartridge variants supported by the core emulator.
#[derive(Debug)]
pub enum CartridgeKind {
    RomOnly(RomOnly),
    Mbc1(Mbc1),
}

impl CartridgeKind {
    /// Decodes a ROM blob into one of the supported mapper implementations.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, CartridgeError> {
        let header = Header::parse(&data)?;
        let info = header.to_info()?;

        if data.len() < info.rom_size {
            return Err(CartridgeError::RomTooSmall(data.len()));
        }

        match info.mapper {
            MapperKind::RomOnly => Ok(Self::RomOnly(RomOnly::new(data, info))),
            MapperKind::Mbc1 => Ok(Self::Mbc1(Mbc1::new(data, info)?)),
        }
    }
}

impl Memory for CartridgeKind {
    fn read_byte(&self, address: u16) -> u8 {
        match self {
            CartridgeKind::RomOnly(inner) => inner.read_byte(address),
            CartridgeKind::Mbc1(inner) => inner.read_byte(address),
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        match self {
            CartridgeKind::RomOnly(inner) => inner.write_byte(address, value),
            CartridgeKind::Mbc1(inner) => inner.write_byte(address, value),
        }
    }
}

impl Cartridge for CartridgeKind {
    fn info(&self) -> &CartridgeInfo {
        match self {
            CartridgeKind::RomOnly(inner) => inner.info(),
            CartridgeKind::Mbc1(inner) => inner.info(),
        }
    }
}

/// Mapper families recognised by the loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapperKind {
    RomOnly,
    Mbc1,
}

// -------------------------------------------------------------------------------------------------
// Header parsing
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Header {
    title: String,
    cartridge_type: u8,
    rom_size_code: u8,
    ram_size_code: u8,
}

impl Header {
    fn parse(rom: &[u8]) -> Result<Self, CartridgeError> {
        if rom.len() < 0x150 {
            return Err(CartridgeError::RomTooSmall(rom.len()));
        }

        let title_bytes = &rom[0x134..=0x142];
        let title_end = title_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(title_bytes.len());
        let title = String::from_utf8_lossy(&title_bytes[..title_end])
            .trim()
            .to_string();

        Ok(Self {
            title,
            cartridge_type: rom[0x147],
            rom_size_code: rom[0x148],
            ram_size_code: rom[0x149],
        })
    }

    fn to_info(&self) -> Result<CartridgeInfo, CartridgeError> {
        let mapper = MapperKind::from_type_byte(self.cartridge_type)?;
        let rom_banks = rom_bank_count(self.rom_size_code)?;
        let rom_size = rom_banks * 0x4000;
        let (ram_banks, ram_bank_size) = ram_bank_configuration(self.ram_size_code)?;
        let ram_size = ram_banks * ram_bank_size;

        Ok(CartridgeInfo {
            title: self.title.clone(),
            mapper,
            rom_size,
            ram_size,
            rom_banks,
            ram_banks,
        })
    }
}

impl MapperKind {
    fn from_type_byte(byte: u8) -> Result<Self, CartridgeError> {
        match byte {
            0x00 => Ok(MapperKind::RomOnly),
            #[allow(clippy::manual_range_patterns)]
            0x01 | 0x02 | 0x03 => Ok(MapperKind::Mbc1),
            other => Err(CartridgeError::UnsupportedCartridgeType(other)),
        }
    }
}

fn rom_bank_count(code: u8) -> Result<usize, CartridgeError> {
    // Values follow Pan Docs § "ROM Size".
    let count = match code {
        0x00 => 2,
        0x01 => 4,
        0x02 => 8,
        0x03 => 16,
        0x04 => 32,
        0x05 => 64,
        0x06 => 128,
        0x07 => 256,
        0x08 => 512,
        0x52 => 72,
        0x53 => 80,
        0x54 => 96,
        other => return Err(CartridgeError::UnsupportedRomSizeCode(other)),
    };
    Ok(count)
}

fn ram_bank_configuration(code: u8) -> Result<(usize, usize), CartridgeError> {
    // Values follow Pan Docs § "RAM Size".
    match code {
        0x00 => Ok((0, 0x2000)),
        0x01 => Ok((1, 0x0800)), // 2 KiB (used by MBC2, but easy to support generically)
        0x02 => Ok((1, 0x2000)), // 8 KiB
        0x03 => Ok((4, 0x2000)), // 32 KiB total, 4 banks
        0x04 => Ok((16, 0x2000)), // 128 KiB total
        0x05 => Ok((8, 0x2000)), // 64 KiB total
        other => Err(CartridgeError::UnsupportedRamSizeCode(other)),
    }
}

// -------------------------------------------------------------------------------------------------
// ROM-only mapper
// -------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct RomOnly {
    rom: Vec<u8>,
    info: CartridgeInfo,
}

impl RomOnly {
    fn new(rom: Vec<u8>, info: CartridgeInfo) -> Self {
        Self { rom, info }
    }

    fn info(&self) -> &CartridgeInfo {
        &self.info
    }
}

impl Memory for RomOnly {
    fn read_byte(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x7FFF => {
                let addr = address as usize;
                self.rom.get(addr).copied().unwrap_or(0xFF)
            }
            0xA000..=0xBFFF => 0xFF, // no RAM present in the ROM-only configuration
            _ => 0xFF,
        }
    }

    fn write_byte(&mut self, _address: u16, _value: u8) {
        // Pure ROM cartridges ignore writes.
    }
}

impl Cartridge for RomOnly {
    fn info(&self) -> &CartridgeInfo {
        self.info()
    }
}

// -------------------------------------------------------------------------------------------------
// MBC1 mapper
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BankingMode {
    Rom,
    Ram,
}

#[derive(Debug)]
pub struct Mbc1 {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,
    info: CartridgeInfo,
    rom_bank_low5: u8,
    rom_bank_high2: u8,
    ram_enabled: bool,
    banking_mode: BankingMode,
}

impl Mbc1 {
    fn new(rom: Vec<u8>, info: CartridgeInfo) -> Result<Self, CartridgeError> {
        let ram = if info.ram_size == 0 {
            None
        } else {
            Some(vec![0; info.ram_size])
        };

        Ok(Self {
            rom,
            ram,
            info,
            rom_bank_low5: 1, // default to bank 1 per hardware power-on behaviour
            rom_bank_high2: 0,
            ram_enabled: false,
            banking_mode: BankingMode::Rom,
        })
    }

    fn info(&self) -> &CartridgeInfo {
        &self.info
    }

    fn rom_bank_count(&self) -> usize {
        self.info.rom_banks.max(1)
    }

    fn ram_bank_count(&self) -> usize {
        self.info.ram_banks.max(1)
    }

    fn current_lower_rom_bank(&self) -> usize {
        match self.banking_mode {
            BankingMode::Rom => 0,
            BankingMode::Ram => {
                let bank = (self.rom_bank_high2 as usize) << 5;
                self.wrap_rom_bank(bank)
            }
        }
    }

    fn current_upper_rom_bank(&self) -> usize {
        let mut bank = self.rom_bank_low5 as usize;
        match self.banking_mode {
            BankingMode::Rom => {
                bank |= (self.rom_bank_high2 as usize) << 5;
            }
            BankingMode::Ram => {
                // In RAM banking mode the upper bits feed the RAM bank, so the ROM bank uses only
                // the low five bits.
            }
        }

        let bank = self.wrap_rom_bank(bank);
        if bank == 0 && self.rom_bank_count() > 1 {
            1
        } else {
            bank
        }
    }

    fn current_ram_bank(&self) -> usize {
        if self.info.ram_size == 0 {
            return 0;
        }

        match self.banking_mode {
            BankingMode::Rom => 0,
            BankingMode::Ram => {
                let bank = self.rom_bank_high2 as usize;
                bank % self.ram_bank_count()
            }
        }
    }

    fn wrap_rom_bank(&self, bank: usize) -> usize {
        if self.rom_bank_count() == 0 {
            0
        } else {
            bank % self.rom_bank_count()
        }
    }

    fn rom_bank_offset(&self, bank: usize) -> usize {
        bank * 0x4000
    }

    fn read_rom(&self, bank: usize, offset: u16) -> u8 {
        let base = self.rom_bank_offset(bank);
        let index = base + offset as usize;
        self.rom.get(index).copied().unwrap_or(0xFF)
    }

    fn read_ram(&self, bank: usize, offset: u16) -> u8 {
        self.ram
            .as_ref()
            .and_then(|ram| {
                let bank_size = ram.len() / self.ram_bank_count().max(1);
                let base = bank_size * bank;
                let index = base + offset as usize;
                ram.get(index).copied()
            })
            .unwrap_or(0xFF)
    }

    fn write_ram(&mut self, bank: usize, offset: u16, value: u8) {
        let bank_count = self.ram_bank_count().max(1);
        if let Some(ram) = self.ram.as_mut() {
            let bank_size = ram.len() / bank_count;
            let base = bank_size * bank;
            let index = base + offset as usize;
            if let Some(cell) = ram.get_mut(index) {
                *cell = value;
            }
        }
    }
}

impl Memory for Mbc1 {
    fn read_byte(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => {
                let bank = self.current_lower_rom_bank();
                self.read_rom(bank, address)
            }
            0x4000..=0x7FFF => {
                let bank = self.current_upper_rom_bank();
                let offset = address - 0x4000;
                self.read_rom(bank, offset)
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled {
                    return 0xFF;
                }
                let bank = self.current_ram_bank();
                let offset = address - 0xA000;
                self.read_ram(bank, offset)
            }
            _ => 0xFF,
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                // RAM enable register (0x0A enables when RAM present).
                self.ram_enabled = (value & 0x0F) == 0x0A && self.ram.is_some();
            }
            0x2000..=0x3FFF => {
                // Lower five bits of the ROM bank number.
                let value = value & 0x1F;
                self.rom_bank_low5 = if value == 0 { 1 } else { value };
            }
            0x4000..=0x5FFF => {
                // High two bits of ROM bank (mode 0) or RAM bank (mode 1).
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
                    let offset = address - 0xA000;
                    self.write_ram(bank, offset, value);
                }
            }
            _ => {} // Unused ranges are ignored.
        }
    }
}

impl Cartridge for Mbc1 {
    fn info(&self) -> &CartridgeInfo {
        self.info()
    }
}
