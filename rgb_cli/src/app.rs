use std::ffi::OsString;

use crate::config::RunConfig;
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
        let _config = self.parse_run_config()?;

        // Runtime orchestration is implemented in later Milestone 1 steps.
        // This step is focused on producing a validated typed configuration.
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

    fn parse_run_config(&self) -> Result<RunConfig, CliError> {
        let user_args = self.raw_args.iter().skip(1).cloned();
        RunConfig::parse_cli_args(user_args).map_err(|error| CliError::usage(error.to_string()))
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
    fn app_requires_user_rom_path_after_program_name() {
        let result = App::from_args(["rgb_cli"]).run();
        let error = result.expect_err("expected usage error");

        assert_eq!(error.kind(), CliErrorKind::Usage);
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn app_accepts_standard_process_argument_vector_with_rom() {
        let result = App::from_args(["rgb_cli", "rom.gb"]);
        assert!(result.run().is_ok());
    }
}
