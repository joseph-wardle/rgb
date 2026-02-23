use std::error::Error;
use std::fmt::{self, Display, Formatter};

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

/// Top-level error type for `rgb_cli`.
///
/// The type intentionally carries:
/// - a small, explicit failure category (`Usage` vs `Runtime`)
/// - a human-readable message suitable for stderr output
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    kind: CliErrorKind,
    message: String,
}

impl CliError {
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            kind: CliErrorKind::Usage,
            message: message.into(),
        }
    }

    pub fn runtime(message: impl Into<String>) -> Self {
        Self {
            kind: CliErrorKind::Runtime,
            message: message.into(),
        }
    }

    pub const fn kind(&self) -> CliErrorKind {
        self.kind
    }

    pub const fn exit_code(&self) -> i32 {
        match self.kind {
            CliErrorKind::Usage => CliExitCode::UsageFailure.as_i32(),
            CliErrorKind::Runtime => CliExitCode::RuntimeFailure.as_i32(),
        }
    }
}

impl Display for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for CliError {}
