use std::path::Path;

use thiserror::Error;

use crate::config::ConfigError;
use rgb_core::cartridge::CartridgeError;

/// Process exit code constants used by `rgb_cli`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum CliExitCode {
    Success = 0,
    RuntimeFailure = 1,
    UsageFailure = 2,
}

impl CliExitCode {
    pub const fn as_i32(self) -> i32 {
        self as i32
    }
}

/// High-level category for user-facing CLI failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliErrorKind {
    /// Invalid invocation, missing arguments, or malformed option values.
    Usage,
    /// Runtime/setup failure while executing a valid invocation.
    Runtime,
}

/// Boundary error type for `rgb_cli`.
///
/// This enum intentionally distinguishes between:
/// - argument/usage failures
/// - runtime/setup/IO failures
/// - ROM parsing failures
/// - compile-time feature gating issues
#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Args(#[from] ConfigError),

    #[error("I/O error while {action} '{path}': {source}")]
    Io {
        action: &'static str,
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse ROM '{path}': {source}")]
    RomParse {
        path: String,
        #[source]
        source: CartridgeError,
    },

    #[error("runtime setup error: {0}")]
    RuntimeSetup(String),

    #[error("--trace requires a trace-enabled build. Rebuild with `--features trace`.")]
    TraceFeatureRequired,
}

impl CliError {
    pub fn io(action: &'static str, path: impl AsRef<Path>, source: std::io::Error) -> Self {
        Self::Io {
            action,
            path: path.as_ref().display().to_string(),
            source,
        }
    }

    pub fn rom_parse(path: impl AsRef<Path>, source: CartridgeError) -> Self {
        Self::RomParse {
            path: path.as_ref().display().to_string(),
            source,
        }
    }

    pub fn runtime_setup(message: impl Into<String>) -> Self {
        Self::RuntimeSetup(message.into())
    }

    pub const fn kind(&self) -> CliErrorKind {
        match self {
            CliError::Args(_) => CliErrorKind::Usage,
            CliError::Io { .. }
            | CliError::RomParse { .. }
            | CliError::RuntimeSetup(_)
            | CliError::TraceFeatureRequired => CliErrorKind::Runtime,
        }
    }

    pub const fn exit_code(&self) -> i32 {
        match self.kind() {
            CliErrorKind::Usage => CliExitCode::UsageFailure.as_i32(),
            CliErrorKind::Runtime => CliExitCode::RuntimeFailure.as_i32(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CliError, CliErrorKind};
    use crate::config::RunConfig;

    #[test]
    fn args_errors_map_to_usage_exit_code() {
        let parse_error =
            RunConfig::parse_cli_args(["--unknown", "rom.gb"]).expect_err("expected parse failure");
        let error = CliError::from(parse_error);
        assert_eq!(error.kind(), CliErrorKind::Usage);
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn runtime_setup_errors_map_to_runtime_exit_code() {
        let error = CliError::runtime_setup("bootstrapping failed");
        assert_eq!(error.kind(), CliErrorKind::Runtime);
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn trace_feature_error_has_clear_message() {
        let error = CliError::TraceFeatureRequired;
        assert!(
            error
                .to_string()
                .contains("Rebuild with `--features trace`")
        );
    }
}
