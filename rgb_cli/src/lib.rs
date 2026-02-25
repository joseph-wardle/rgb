//! Library entrypoint for the `rgb_cli` host application.
//!
//! Keeping CLI logic in a library makes behavior unit-testable without
//! subprocess orchestration. The binary target stays intentionally thin and
//! only handles process-level concerns like stderr printing and exit codes.

mod app;
mod config;
mod emulator;
mod error;
mod rom;
mod runner;
mod trace;

pub use config::{BootMode, CliRequest, ConfigError, RunConfig, SerialMode};
pub use error::{CliError, CliErrorKind, CliExitCode};
pub use rom::{LoadedRom, RomMetadata, load_rom};
pub use runner::{RunReport, Runner, SerialVerdict, SerialVerdictCondition, StopReason};

/// Runs the CLI application using the current process argument vector.
pub fn run() -> Result<(), CliError> {
    run_with_args(std::env::args_os())
}

/// Runs the CLI application using an explicit argument iterator.
///
/// This function is the preferred entrypoint for tests.
pub fn run_with_args<I, S>(args: I) -> Result<(), CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString>,
{
    app::App::from_args(args).run()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::{CliErrorKind, run_with_args};
    use tempfile::NamedTempFile;

    #[test]
    fn run_with_args_bubbles_up_usage_or_runtime_failures() {
        let result = run_with_args(std::iter::empty::<&str>());
        let error = result.expect_err("expected error for empty argv");
        assert_eq!(error.kind(), CliErrorKind::Runtime);
    }

    #[test]
    fn run_with_args_is_invocable_without_subprocesses() {
        let mut rom_file = valid_test_rom_file();
        rom_file.flush().expect("flush ROM file");
        let rom_path = rom_file.path().display().to_string();

        let result = run_with_args(["rgb_cli", "--frames", "1", &rom_path]);
        assert!(result.is_ok());
    }

    #[test]
    fn run_with_args_accepts_help_flow() {
        let result = run_with_args(["rgb_cli", "--help"]);
        assert!(result.is_ok());
    }

    #[test]
    fn run_with_args_reports_usage_error_for_bad_arguments() {
        let result = run_with_args(["rgb_cli", "--unknown", "rom.gb"]);
        let error = result.expect_err("expected parse failure");
        assert_eq!(error.kind(), CliErrorKind::Usage);
        assert!(
            error
                .to_string()
                .contains("unexpected argument '--unknown'")
        );
    }

    #[test]
    fn run_with_args_reports_runtime_error_for_missing_rom_path() {
        let file = NamedTempFile::new().expect("create temp file path");
        let missing = file.path().to_path_buf();
        drop(file);
        let rom_path = missing.display().to_string();

        let result = run_with_args(["rgb_cli", &rom_path]);
        let error = result.expect_err("expected missing ROM error");
        assert_eq!(error.kind(), CliErrorKind::Runtime);
        assert!(error.to_string().contains("I/O error while reading ROM"));
    }

    #[test]
    fn run_with_args_reports_unsupported_cartridge_error() {
        let mut rom_file = test_rom_file_with_cartridge_type(0xFF);
        rom_file.flush().expect("flush ROM file");
        let rom_path = rom_file.path().display().to_string();

        let result = run_with_args(["rgb_cli", "--frames", "1", &rom_path]);
        let error = result.expect_err("expected unsupported cartridge parse error");
        assert_eq!(error.kind(), CliErrorKind::Runtime);
        assert!(error.to_string().contains("failed to parse ROM"));
        assert!(error.to_string().contains("cartridge type 0xFF"));
    }

    fn valid_test_rom_file() -> NamedTempFile {
        test_rom_file_with_cartridge_type(0x00)
    }

    fn test_rom_file_with_cartridge_type(cartridge_type: u8) -> NamedTempFile {
        let mut bytes = vec![0; 0x8000];
        bytes[0x134..0x13A].copy_from_slice(b"LIBROM");
        bytes[0x147] = cartridge_type;
        bytes[0x148] = 0x00;
        bytes[0x149] = 0x00;

        let mut file = NamedTempFile::new().expect("create temp ROM file");
        file.write_all(&bytes).expect("write ROM bytes");
        file
    }
}
