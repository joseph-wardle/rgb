//! Library entrypoint for the `rgb_cli` host application.
//!
//! Keeping CLI logic in a library makes behavior unit-testable without
//! subprocess orchestration. The binary target stays intentionally thin and
//! only handles process-level concerns like stderr printing and exit codes.

mod app;
mod config;
mod error;

pub use config::{BootMode, CliRequest, ConfigError, RunConfig, SerialMode};
pub use error::{CliError, CliErrorKind, CliExitCode};

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
    use super::{CliErrorKind, run_with_args};

    #[test]
    fn run_with_args_bubbles_up_usage_or_runtime_failures() {
        let result = run_with_args(std::iter::empty::<&str>());
        let error = result.expect_err("expected error for empty argv");
        assert_eq!(error.kind(), CliErrorKind::Runtime);
    }

    #[test]
    fn run_with_args_is_invocable_without_subprocesses() {
        let result = run_with_args(["rgb_cli", "rom.gb"]);
        assert!(result.is_ok());
    }

    #[test]
    fn run_with_args_accepts_help_flow() {
        let result = run_with_args(["rgb_cli", "--help"]);
        assert!(result.is_ok());
    }
}
