use std::ffi::OsString;

use crate::error::CliError;

/// Thin application object that owns the process arguments.
///
/// Milestone 1 Step 2 intentionally keeps runtime behavior minimal while
/// establishing a testable, library-driven execution path.
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

        // Argument parsing and runtime orchestration are implemented in later
        // Milestone 1 steps. Returning success here keeps this step focused on
        // crate structure and testability.
        Ok(())
    }

    fn ensure_program_name_is_present(&self) -> Result<(), CliError> {
        if self.raw_args.is_empty() {
            return Err(CliError::runtime(
                "process argument vector was unexpectedly empty",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use crate::error::CliErrorKind;

    #[test]
    fn app_requires_program_name_argument() {
        let result = App::from_args(std::iter::empty::<&str>()).run();
        let error = result.expect_err("expected an error for empty argv");

        assert_eq!(error.kind(), CliErrorKind::Runtime);
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn app_accepts_standard_process_argument_vector() {
        let result = App::from_args(["rgb_cli"]);
        assert!(result.run().is_ok());
    }
}
