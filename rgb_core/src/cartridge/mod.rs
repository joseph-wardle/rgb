//! Cartridge mappers for the DMG Game Boy.
//!
//! A physical cartridge contains ROM (and optionally RAM) plus a Memory Bank
//! Controller (MBC) chip. Because the CPU address bus only exposes a 16 KiB
//! window for switchable ROM (0x4000–0x7FFF) and an 8 KiB window for RAM
//! (0xA000–0xBFFF), the MBC extends effective storage by swapping which
//! "bank" of ROM or RAM is visible in each window.
//!
//! ## Dispatch
//!
//! [`CartridgeKind`] is a plain enum that owns one concrete mapper. The rest
//! of the emulator holds a `Box<dyn Cartridge>` and routes all reads and
//! writes through the [`Cartridge`] trait without knowing the mapper type.
//!
//! ## Mappers implemented
//!
//! | Mapper   | Type bytes  | Notable games                        |
//! |----------|-------------|--------------------------------------|
//! | ROM-only | 0x00        | Tetris, Dr. Mario                    |
//! | MBC1     | 0x01–0x03   | Super Mario Land, Kirby's Dream Land |
//! | MBC3     | 0x0F–0x13   | Pokémon Red/Blue, Link's Awakening   |

use crate::memory::Memory;
use std::fmt;

mod mbc1;
mod mbc3;
mod rom_only;

use mbc1::Mbc1;
use mbc3::Mbc3;
use rom_only::RomOnly;

// ---------------------------------------------------------------------------
// Public API: errors, metadata, trait
// ---------------------------------------------------------------------------

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
            CartridgeError::UnsupportedCartridgeType(t) => {
                write!(f, "cartridge type 0x{t:02X} is not supported")
            }
            CartridgeError::UnsupportedRomSizeCode(c) => {
                write!(f, "ROM size code 0x{c:02X} is not supported")
            }
            CartridgeError::UnsupportedRamSizeCode(c) => {
                write!(f, "RAM size code 0x{c:02X} is not supported")
            }
        }
    }
}

impl std::error::Error for CartridgeError {}

/// Metadata decoded from the cartridge header.
#[derive(Debug, Clone)]
pub struct CartridgeInfo {
    pub title:     String,
    pub mapper:    MapperKind,
    pub rom_size:  usize,
    pub ram_size:  usize,
    pub rom_banks: usize,
    pub ram_banks: usize,
}

/// Trait implemented by all mapper backends.
///
/// Extends [`Memory`] with cartridge-specific metadata. The rest of the
/// emulator holds a `Box<dyn Cartridge>` so it never needs to name a
/// specific mapper type.
pub trait Cartridge: Memory {
    fn info(&self) -> &CartridgeInfo;
}

// ---------------------------------------------------------------------------
// Mapper kind
// ---------------------------------------------------------------------------

/// Mapper families recognised by the cartridge loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapperKind {
    RomOnly,
    Mbc1,
    Mbc3,
}

// ---------------------------------------------------------------------------
// CartridgeKind — enum dispatch over all mappers
// ---------------------------------------------------------------------------

/// A loaded cartridge. Owns the underlying mapper and dispatches reads and
/// writes to it. Use [`CartridgeKind::from_bytes`] to build one from a ROM.
pub enum CartridgeKind {
    RomOnly(RomOnly),
    Mbc1(Mbc1),
    Mbc3(Mbc3),
}

impl CartridgeKind {
    /// Decode a ROM blob into the appropriate mapper, as determined by the
    /// cartridge type byte at 0x0147.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, CartridgeError> {
        let header = Header::parse(&data)?;
        let info = header.to_info()?;

        if data.len() < info.rom_size {
            return Err(CartridgeError::RomTooSmall(data.len()));
        }

        match info.mapper {
            MapperKind::RomOnly => Ok(Self::RomOnly(RomOnly::new(data, info))),
            MapperKind::Mbc1    => Ok(Self::Mbc1(Mbc1::new(data, info)?)),
            MapperKind::Mbc3    => Ok(Self::Mbc3(Mbc3::new(data, info))),
        }
    }
}

impl Memory for CartridgeKind {
    fn read_byte(&self, address: u16) -> u8 {
        match self {
            CartridgeKind::RomOnly(c) => c.read_byte(address),
            CartridgeKind::Mbc1(c)    => c.read_byte(address),
            CartridgeKind::Mbc3(c)    => c.read_byte(address),
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        match self {
            CartridgeKind::RomOnly(c) => c.write_byte(address, value),
            CartridgeKind::Mbc1(c)    => c.write_byte(address, value),
            CartridgeKind::Mbc3(c)    => c.write_byte(address, value),
        }
    }
}

impl Cartridge for CartridgeKind {
    fn info(&self) -> &CartridgeInfo {
        match self {
            CartridgeKind::RomOnly(c) => c.info(),
            CartridgeKind::Mbc1(c)    => c.info(),
            CartridgeKind::Mbc3(c)    => c.info(),
        }
    }
}

// ---------------------------------------------------------------------------
// Header parsing
// ---------------------------------------------------------------------------

struct Header {
    title:          String,
    cartridge_type: u8,
    rom_size_code:  u8,
    ram_size_code:  u8,
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
            rom_size_code:  rom[0x148],
            ram_size_code:  rom[0x149],
        })
    }

    fn to_info(&self) -> Result<CartridgeInfo, CartridgeError> {
        let mapper    = MapperKind::from_type_byte(self.cartridge_type)?;
        let rom_banks = rom_bank_count(self.rom_size_code)?;
        let (ram_banks, ram_bank_size) = ram_bank_config(self.ram_size_code)?;

        Ok(CartridgeInfo {
            title:     self.title.clone(),
            mapper,
            rom_size:  rom_banks * 0x4000,
            ram_size:  ram_banks * ram_bank_size,
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
            0x0F | 0x10 | 0x11 | 0x12 | 0x13 => Ok(MapperKind::Mbc3),
            other => Err(CartridgeError::UnsupportedCartridgeType(other)),
        }
    }
}

// ---------------------------------------------------------------------------
// Bank size tables (Pan Docs § "ROM Size" and "RAM Size")
// ---------------------------------------------------------------------------

fn rom_bank_count(code: u8) -> Result<usize, CartridgeError> {
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

/// Returns `(bank_count, bytes_per_bank)`.
fn ram_bank_config(code: u8) -> Result<(usize, usize), CartridgeError> {
    match code {
        0x00 => Ok((0,  0x2000)), // no RAM
        0x01 => Ok((1,  0x0800)), // 2 KiB (MBC2-style nibble RAM)
        0x02 => Ok((1,  0x2000)), // 8 KiB
        0x03 => Ok((4,  0x2000)), // 32 KiB
        0x04 => Ok((16, 0x2000)), // 128 KiB
        0x05 => Ok((8,  0x2000)), // 64 KiB
        other => Err(CartridgeError::UnsupportedRamSizeCode(other)),
    }
}
