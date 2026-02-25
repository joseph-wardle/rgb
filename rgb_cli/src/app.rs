use std::ffi::OsString;
use std::num::NonZeroU64;

use crate::config::{CliRequest, RunConfig};
use crate::emulator::construct_gameboy;
use crate::error::CliError;
use crate::rom::{LoadedRom, load_rom};

/// Thin application object that owns the process arguments.
///
/// The binary entrypoint calls into this type so the CLI behavior can be
/// tested directly without subprocess orchestration.
#[derive(Debug, Clone)]
pub(crate) struct App {
    raw_args: Vec<OsString>,
}

impl App {
    pub(crate) fn from_args<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        Self {
            raw_args: args.into_iter().map(Into::into).collect(),
        }
    }

    pub(crate) fn run(self) -> Result<(), CliError> {
        self.ensure_program_name_is_present()?;
        match self.parse_cli_request()? {
            CliRequest::Help(output) => {
                print!("{output}");
                Ok(())
            }
            CliRequest::Version(output) => {
                print!("{output}");
                Ok(())
            }
            CliRequest::Run(config) => {
                self.validate_feature_gates(&config)?;
                let loaded_rom = load_rom(&config.rom_path)?;
                if let Some(summary) = Self::build_startup_summary(&config, &loaded_rom) {
                    println!("{summary}");
                }
                let _gameboy = construct_gameboy(config.boot_mode, loaded_rom);
                // Runtime orchestration is implemented in later Milestone 1
                // steps. By this point, arguments are fully validated, ROM data
                // is parsed, and hardware state is constructed.
                Ok(())
            }
        }
    }

    fn ensure_program_name_is_present(&self) -> Result<(), CliError> {
        if self.raw_args.is_empty() {
            return Err(CliError::runtime_setup(
                "process argument vector was unexpectedly empty",
            ));
        }

        Ok(())
    }

    fn parse_cli_request(&self) -> Result<CliRequest, CliError> {
        let user_args = self.raw_args.iter().skip(1).cloned();
        RunConfig::parse_cli_request(user_args).map_err(CliError::from)
    }

    fn validate_feature_gates(&self, config: &RunConfig) -> Result<(), CliError> {
        if config.trace {
            #[cfg(not(feature = "trace"))]
            {
                return Err(CliError::TraceFeatureRequired);
            }
        }

        Ok(())
    }

    fn build_startup_summary(config: &RunConfig, loaded_rom: &LoadedRom) -> Option<String> {
        if config.quiet {
            return None;
        }

        let metadata = loaded_rom.metadata();
        let rom_size_kib = kibibytes(metadata.rom_size_bytes);
        let ram_size_kib = kibibytes(metadata.ram_size_bytes);
        let frame_limit = frame_limit_label(config.frame_limit);
        let trace_mode = if config.trace { "enabled" } else { "disabled" };

        Some(format!(
            "ROM: {} | title: {} | mapper: {} | size: {rom_size_kib} KiB ROM / {ram_size_kib} KiB RAM | boot: {} | frames: {frame_limit} | serial: {} | trace: {trace_mode}",
            loaded_rom.path().display(),
            metadata.display_title(),
            metadata.mapper_label(),
            config.boot_mode,
            config.serial_mode,
        ))
    }

    #[cfg(test)]
    fn parse_run_config_for_test(&self) -> Result<RunConfig, CliError> {
        let user_args = self.raw_args.iter().skip(1).cloned();
        RunConfig::parse_cli_args(user_args).map_err(CliError::from)
    }
}

fn frame_limit_label(frame_limit: Option<NonZeroU64>) -> String {
    frame_limit
        .map(|value| value.get().to_string())
        .unwrap_or_else(|| "unbounded".to_string())
}

fn kibibytes(bytes: usize) -> usize {
    bytes / 1024
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::App;
    use crate::config::{BootMode, CliRequest, SerialMode};
    use crate::error::CliErrorKind;
    use crate::rom::load_rom;
    use tempfile::NamedTempFile;

    #[test]
    fn app_requires_program_name_argument() {
        let result = App::from_args(std::iter::empty::<&str>()).run();
        let error = result.expect_err("expected an error for empty argv");

        assert_eq!(error.kind(), CliErrorKind::Runtime);
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn app_requires_user_rom_path_after_program_name() {
        let result = App::from_args(["rgb_cli"]).run();
        let error = result.expect_err("expected usage error");

        assert_eq!(error.kind(), CliErrorKind::Usage);
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn app_accepts_standard_process_argument_vector_with_rom() {
        let mut rom_file = write_test_rom("APPBOOT", 0x00, 0x00, 0x00);
        rom_file.flush().expect("flush ROM file");
        let rom_path = rom_file.path().display().to_string();

        let result = App::from_args(["rgb_cli", &rom_path]);
        assert!(result.run().is_ok());
    }

    #[cfg(not(feature = "trace"))]
    #[test]
    fn app_rejects_trace_flag_without_trace_feature() {
        let mut rom_file = write_test_rom("TRACETST", 0x00, 0x00, 0x00);
        rom_file.flush().expect("flush ROM file");
        let rom_path = rom_file.path().display().to_string();

        let result = App::from_args(["rgb_cli", "--trace", &rom_path]).run();
        let error = result.expect_err("expected trace feature-gate error");

        assert_eq!(error.kind(), CliErrorKind::Runtime);
        assert_eq!(error.exit_code(), 1);
        assert!(
            error
                .to_string()
                .contains("Rebuild with `--features trace`")
        );
    }

    #[cfg(feature = "trace")]
    #[test]
    fn app_accepts_trace_flag_with_trace_feature_enabled() {
        let mut rom_file = write_test_rom("TRACETST", 0x00, 0x00, 0x00);
        rom_file.flush().expect("flush ROM file");
        let rom_path = rom_file.path().display().to_string();

        let result = App::from_args(["rgb_cli", "--trace", &rom_path]).run();
        assert!(result.is_ok());
    }

    #[test]
    fn app_accepts_help_and_version_requests() {
        assert!(App::from_args(["rgb_cli", "--help"]).run().is_ok());
        assert!(App::from_args(["rgb_cli", "-h"]).run().is_ok());
        assert!(App::from_args(["rgb_cli", "--version"]).run().is_ok());
        assert!(App::from_args(["rgb_cli", "-V"]).run().is_ok());
    }

    #[test]
    fn app_returns_actionable_clap_errors_for_invalid_invocation() {
        let app = App::from_args(["rgb_cli", "--unknown", "rom.gb"]);
        let error = app
            .parse_cli_request()
            .expect_err("expected parsing to fail");

        assert_eq!(error.kind(), CliErrorKind::Usage);
        assert!(
            error
                .to_string()
                .contains("unexpected argument '--unknown'")
        );
        assert!(error.to_string().contains("For more information"));
    }

    #[test]
    fn app_exposes_typed_run_config_for_valid_invocation() {
        let app = App::from_args([
            "rgb_cli", "--boot", "cold", "--serial", "final", "--quiet", "rom.gb",
        ]);
        let config = app
            .parse_run_config_for_test()
            .expect("expected run config for valid invocation");

        assert_eq!(config.boot_mode, BootMode::Cold);
        assert_eq!(config.serial_mode, SerialMode::Final);
        assert!(config.quiet);
    }

    #[test]
    fn app_parse_cli_request_returns_help_version_and_run_commands() {
        let help = App::from_args(["rgb_cli", "-h"])
            .parse_cli_request()
            .expect("expected help request");
        assert!(matches!(help, CliRequest::Help(_)));

        let version = App::from_args(["rgb_cli", "-V"])
            .parse_cli_request()
            .expect("expected version request");
        assert!(matches!(version, CliRequest::Version(_)));

        let run = App::from_args(["rgb_cli", "rom.gb"])
            .parse_cli_request()
            .expect("expected run request");
        assert!(matches!(run, CliRequest::Run(_)));
    }

    #[test]
    fn app_surfaces_runtime_error_when_rom_file_is_missing() {
        let file = NamedTempFile::new().expect("create temp file path");
        let missing = file.path().to_path_buf();
        drop(file);
        let rom_path = missing.display().to_string();

        let result = App::from_args(["rgb_cli", &rom_path]).run();
        let error = result.expect_err("expected runtime error for missing ROM");

        assert_eq!(error.kind(), CliErrorKind::Runtime);
        assert!(error.to_string().contains("I/O error while reading ROM"));
    }

    #[test]
    fn startup_summary_includes_rom_and_run_configuration_metadata() {
        let mut rom_file = write_test_rom("TETRIS", 0x00, 0x00, 0x00);
        rom_file.flush().expect("flush ROM file");
        let rom_path = rom_file.path().display().to_string();

        let app = App::from_args([
            "rgb_cli", "--boot", "cold", "--frames", "1440", "--serial", "live", &rom_path,
        ]);
        let config = app
            .parse_run_config_for_test()
            .expect("expected valid run config");
        let loaded_rom = load_rom(&config.rom_path).expect("expected ROM to load");

        let summary = App::build_startup_summary(&config, &loaded_rom).expect("expected summary");

        assert!(summary.contains(&format!("ROM: {}", config.rom_path.display())));
        assert!(summary.contains("title: TETRIS"));
        assert!(summary.contains("mapper: ROM-only"));
        assert!(summary.contains("size: 32 KiB ROM / 0 KiB RAM"));
        assert!(summary.contains("boot: cold"));
        assert!(summary.contains("frames: 1440"));
        assert!(summary.contains("serial: live"));
        assert!(summary.contains("trace: disabled"));
    }

    #[test]
    fn startup_summary_is_suppressed_in_quiet_mode() {
        let mut rom_file = write_test_rom("QUIET", 0x00, 0x00, 0x00);
        rom_file.flush().expect("flush ROM file");
        let rom_path = rom_file.path().display().to_string();

        let app = App::from_args(["rgb_cli", "--quiet", &rom_path]);
        let config = app
            .parse_run_config_for_test()
            .expect("expected valid run config");
        let loaded_rom = load_rom(&config.rom_path).expect("expected ROM to load");

        assert!(App::build_startup_summary(&config, &loaded_rom).is_none());
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
