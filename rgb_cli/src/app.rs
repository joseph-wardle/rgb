use std::ffi::OsString;

use crate::config::{CliRequest, RunConfig};
use crate::error::CliError;

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
                // Runtime orchestration is implemented in later Milestone 1
                // steps. By this point, arguments are fully validated and typed.
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

    #[cfg(test)]
    fn parse_run_config_for_test(&self) -> Result<RunConfig, CliError> {
        let user_args = self.raw_args.iter().skip(1).cloned();
        RunConfig::parse_cli_args(user_args).map_err(CliError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use crate::config::{BootMode, CliRequest, SerialMode};
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

    #[cfg(not(feature = "trace"))]
    #[test]
    fn app_rejects_trace_flag_without_trace_feature() {
        let result = App::from_args(["rgb_cli", "--trace", "rom.gb"]).run();
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
        let result = App::from_args(["rgb_cli", "--trace", "rom.gb"]).run();
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
}
