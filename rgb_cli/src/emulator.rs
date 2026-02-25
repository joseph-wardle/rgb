use rgb_core::cartridge::Cartridge;
use rgb_core::gameboy::DMG;

use crate::config::BootMode;
use crate::rom::LoadedRom;

/// Constructs a `DMG` instance from a parsed ROM and selected boot mode.
///
/// This function is the single source of truth for boot-mode mapping:
/// - `cold` -> `DMG::new`
/// - `post-bios` -> `DMG::new_post_bios`
pub fn construct_gameboy(boot_mode: BootMode, loaded_rom: LoadedRom) -> DMG {
    let cartridge: Box<dyn Cartridge> = Box::new(loaded_rom.into_cartridge());
    BootStrategy::from_boot_mode(boot_mode).construct(cartridge)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BootStrategy {
    Cold,
    PostBios,
}

impl BootStrategy {
    fn from_boot_mode(boot_mode: BootMode) -> Self {
        match boot_mode {
            BootMode::Cold => Self::Cold,
            BootMode::PostBios => Self::PostBios,
        }
    }

    fn construct(self, cartridge: Box<dyn Cartridge>) -> DMG {
        match self {
            Self::Cold => DMG::new(cartridge),
            Self::PostBios => DMG::new_post_bios(cartridge),
        }
    }

    #[cfg(test)]
    fn constructor_name(self) -> &'static str {
        match self {
            Self::Cold => "DMG::new",
            Self::PostBios => "DMG::new_post_bios",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::{BootStrategy, construct_gameboy};
    use crate::config::BootMode;
    use crate::rom::load_rom;

    #[test]
    fn boot_mode_maps_to_the_expected_dmg_constructor() {
        assert_eq!(
            BootStrategy::from_boot_mode(BootMode::Cold).constructor_name(),
            "DMG::new"
        );
        assert_eq!(
            BootStrategy::from_boot_mode(BootMode::PostBios).constructor_name(),
            "DMG::new_post_bios"
        );
    }

    #[test]
    fn constructor_supports_both_boot_modes_for_valid_roms() {
        let mut rom_file = write_test_rom("BOOTMAP", 0x00, 0x00, 0x00);
        rom_file.flush().expect("flush ROM file");

        for boot_mode in [BootMode::Cold, BootMode::PostBios] {
            let loaded_rom = load_rom(rom_file.path()).expect("expected ROM to load");
            let gameboy = construct_gameboy(boot_mode, loaded_rom);
            assert_eq!(gameboy.serial_output(), "");
        }
    }

    fn write_test_rom(
        title: &str,
        cartridge_type: u8,
        rom_size_code: u8,
        ram_size_code: u8,
    ) -> NamedTempFile {
        let mut bytes = vec![0; 0x8000];
        let title_bytes = title.as_bytes();
        let title_len = title_bytes.len().min(15);
        bytes[0x134..0x134 + title_len].copy_from_slice(&title_bytes[..title_len]);
        bytes[0x147] = cartridge_type;
        bytes[0x148] = rom_size_code;
        bytes[0x149] = ram_size_code;

        let mut file = NamedTempFile::new().expect("create temp ROM file");
        file.write_all(&bytes).expect("write ROM bytes");
        file
    }
}
