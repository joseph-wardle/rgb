//! ROM loading and cartridge metadata bridge.
//!
//! Responsibility boundary:
//! - reads ROM bytes from disk
//! - converts bytes into `CartridgeKind`
//! - exposes concise metadata used by CLI startup output

use std::fs;
use std::path::{Path, PathBuf};

use rgb_core::cartridge::{Cartridge, CartridgeInfo, CartridgeKind, MapperKind};

use crate::error::CliError;

/// Immutable cartridge metadata surfaced by the CLI host.
///
/// The fields intentionally mirror the `rgb_core` header parse output so users
/// can see exactly what the emulator believes about the loaded ROM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomMetadata {
    pub title: String,
    pub mapper: MapperKind,
    pub rom_size_bytes: usize,
    pub ram_size_bytes: usize,
    pub rom_banks: usize,
    pub ram_banks: usize,
}

impl RomMetadata {
    #[must_use]
    pub fn mapper_label(&self) -> &'static str {
        mapper_label(self.mapper)
    }

    #[must_use]
    pub fn display_title(&self) -> &str {
        if self.title.is_empty() {
            "<untitled>"
        } else {
            &self.title
        }
    }
}

/// Fully loaded ROM payload plus precomputed metadata.
///
/// `LoadedRom` keeps the parsed `CartridgeKind` so later runtime steps can
/// hand it directly to the `DMG` constructor without reparsing bytes.
#[derive(Debug)]
pub struct LoadedRom {
    path: PathBuf,
    cartridge: CartridgeKind,
    metadata: RomMetadata,
}

impl LoadedRom {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn metadata(&self) -> &RomMetadata {
        &self.metadata
    }

    #[must_use]
    pub fn cartridge(&self) -> &CartridgeKind {
        &self.cartridge
    }

    #[must_use]
    pub fn into_cartridge(self) -> CartridgeKind {
        self.cartridge
    }
}

/// Loads and parses a ROM image from disk.
///
/// IO and cartridge-parse failures are converted into `CliError` variants so
/// the CLI boundary can print precise, user-facing diagnostics.
///
/// # Errors
///
/// Returns `CliError::Io` if the file cannot be read and `CliError::RomParse`
/// if header/mapper parsing fails.
pub fn load_rom(path: impl AsRef<Path>) -> Result<LoadedRom, CliError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|source| CliError::io("reading ROM", path, source))?;
    let cartridge =
        CartridgeKind::from_bytes(bytes).map_err(|source| CliError::rom_parse(path, source))?;
    let metadata = metadata_from_info(cartridge.info());

    Ok(LoadedRom {
        path: path.to_path_buf(),
        cartridge,
        metadata,
    })
}

pub fn mapper_label(mapper: MapperKind) -> &'static str {
    match mapper {
        MapperKind::RomOnly => "ROM-only",
        MapperKind::Mbc1 => "MBC1",
    }
}

fn metadata_from_info(info: &CartridgeInfo) -> RomMetadata {
    RomMetadata {
        title: info.title.clone(),
        mapper: info.mapper,
        rom_size_bytes: info.rom_size,
        ram_size_bytes: info.ram_size,
        rom_banks: info.rom_banks,
        ram_banks: info.ram_banks,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use rgb_core::cartridge::{CartridgeError, MapperKind};
    use tempfile::NamedTempFile;

    use super::load_rom;
    use crate::error::{CliError, CliErrorKind};

    #[test]
    fn load_rom_reports_header_metadata() {
        let mut file = write_test_rom(base_test_rom("TETRIS", 0x00, 0x00, 0x00));
        file.flush().expect("flush ROM file");

        let loaded = load_rom(file.path()).expect("expected ROM to load");
        let metadata = loaded.metadata();

        assert_eq!(loaded.path(), file.path());
        assert_eq!(metadata.title, "TETRIS");
        assert_eq!(metadata.mapper, MapperKind::RomOnly);
        assert_eq!(metadata.rom_size_bytes, 32 * 1024);
        assert_eq!(metadata.ram_size_bytes, 0);
        assert_eq!(metadata.rom_banks, 2);
        assert_eq!(metadata.ram_banks, 0);
        assert_eq!(metadata.mapper_label(), "ROM-only");
    }

    #[test]
    fn load_rom_maps_read_failures_to_io_error() {
        let file = NamedTempFile::new().expect("create temp file path");
        let missing = file.path().to_path_buf();
        drop(file);
        let error = load_rom(&missing).expect_err("expected missing file error");

        assert_eq!(error.kind(), CliErrorKind::Runtime);
        assert!(error.to_string().contains("I/O error while reading ROM"));
        assert!(error.to_string().contains(&missing.display().to_string()));
    }

    #[test]
    fn load_rom_maps_parse_failures_to_rom_parse_error() {
        let mut file = write_test_rom(base_test_rom("BADTYPE", 0xFF, 0x00, 0x00));
        file.flush().expect("flush ROM file");

        let error = load_rom(file.path()).expect_err("expected parse failure");

        assert_eq!(error.kind(), CliErrorKind::Runtime);
        match &error {
            CliError::RomParse { source, .. } => {
                assert!(matches!(
                    source,
                    CartridgeError::UnsupportedCartridgeType(0xFF)
                ));
            }
            _ => panic!("expected ROM parse error"),
        }
        assert!(error.to_string().contains("failed to parse ROM"));
        assert!(error.to_string().contains("cartridge type 0xFF"));
    }

    fn write_test_rom(bytes: Vec<u8>) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("create temp ROM file");
        file.write_all(&bytes).expect("write ROM bytes");
        file
    }

    fn base_test_rom(
        title: &str,
        cartridge_type: u8,
        rom_size_code: u8,
        ram_size_code: u8,
    ) -> Vec<u8> {
        let mut bytes = vec![0; 0x8000];
        let title_bytes = title.as_bytes();
        let title_len = title_bytes.len().min(15);
        bytes[0x134..0x134 + title_len].copy_from_slice(&title_bytes[..title_len]);
        bytes[0x147] = cartridge_type;
        bytes[0x148] = rom_size_code;
        bytes[0x149] = ram_size_code;
        bytes
    }
}
