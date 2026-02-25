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

    fn valid_test_rom_file() -> NamedTempFile {
        let mut bytes = vec![0; 0x8000];
        bytes[0x134..0x13A].copy_from_slice(b"LIBROM");
        bytes[0x147] = 0x00;
        bytes[0x148] = 0x00;
        bytes[0x149] = 0x00;

        let mut file = NamedTempFile::new().expect("create temp ROM file");
        file.write_all(&bytes).expect("write ROM bytes");
        file
    }
}
