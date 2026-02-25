use std::error::Error;
use std::ffi::OsString;
use std::fmt::{self, Display, Formatter};
use std::num::NonZeroU64;
use std::path::PathBuf;

use clap::builder::ValueParser;
use clap::error::ErrorKind;
use clap::{Arg, ArgAction, Command, ValueHint};

/// Selects the CPU/boot initialization path used to construct `DMG`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BootMode {
    Cold,
    #[default]
    PostBios,
}

impl BootMode {
    fn from_cli_value(value: &str) -> Option<Self> {
        match value {
            "cold" => Some(BootMode::Cold),
            "post-bios" => Some(BootMode::PostBios),
            _ => None,
        }
    }
}

impl Display for BootMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            BootMode::Cold => f.write_str("cold"),
            BootMode::PostBios => f.write_str("post-bios"),
        }
    }
}

/// Controls how serial output is exposed to the host terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SerialMode {
    #[default]
    Off,
    Live,
    Final,
}

impl SerialMode {
    fn from_cli_value(value: &str) -> Option<Self> {
        match value {
            "off" => Some(SerialMode::Off),
            "live" => Some(SerialMode::Live),
            "final" => Some(SerialMode::Final),
            _ => None,
        }
    }
}

impl Display for SerialMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SerialMode::Off => f.write_str("off"),
            SerialMode::Live => f.write_str("live"),
            SerialMode::Final => f.write_str("final"),
        }
    }
}

/// Immutable runtime configuration for the `rgb_cli` host executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunConfig {
    pub rom_path: PathBuf,
    pub frame_limit: Option<NonZeroU64>,
    pub boot_mode: BootMode,
    pub serial_mode: SerialMode,
    pub quiet: bool,
    pub trace: bool,
}

/// Top-level CLI request derived from parsed user arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliRequest {
    Run(RunConfig),
    Help(String),
    Version(String),
}

impl RunConfig {
    /// Parses CLI arguments (excluding argv[0]) into a validated run configuration.
    pub fn parse_cli_args<I, S>(args: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        match Self::parse_cli_request(args)? {
            CliRequest::Run(config) => Ok(config),
            CliRequest::Help(text) => Err(ConfigError::new(text)),
            CliRequest::Version(text) => Err(ConfigError::new(text)),
        }
    }

    /// Parses CLI arguments (excluding argv[0]) into a high-level request.
    pub fn parse_cli_request<I, S>(args: I) -> Result<CliRequest, ConfigError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let argv = std::iter::once(OsString::from("rgb_cli"))
            .chain(args.into_iter().map(Into::into))
            .collect::<Vec<_>>();

        let mut command = build_cli_command();
        match command.try_get_matches_from_mut(argv) {
            Ok(matches) => Ok(CliRequest::Run(run_config_from_matches(matches)?)),
            Err(error) => match error.kind() {
                ErrorKind::DisplayHelp => Ok(CliRequest::Help(error.to_string())),
                ErrorKind::DisplayVersion => Ok(CliRequest::Version(error.to_string())),
                _ => Err(ConfigError::new(error.to_string())),
            },
        }
    }
}

/// User-facing CLI parsing or validation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    message: String,
}

impl ConfigError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for ConfigError {}

fn build_cli_command() -> Command {
    Command::new("rgb_cli")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Educational Game Boy emulator host runner.")
        .override_usage("rgb_cli [OPTIONS] <ROM_PATH>")
        .after_help(
            "EXAMPLES:
  rgb_cli ./roms/tetris.gb
  rgb_cli --boot cold --frames 1800 ./roms/tetris.gb
  rgb_cli --serial live ./roms/cpu_instrs.gb
  cargo run -p rgb_cli -- --trace ./roms/tetris.gb",
        )
        .arg(
            Arg::new("rom_path")
                .value_name("ROM_PATH")
                .help("Path to a Game Boy ROM file")
                .value_hint(ValueHint::FilePath)
                .required(true),
        )
        .arg(
            Arg::new("frames")
                .long("frames")
                .value_name("N")
                .help("Stop after N frames (N >= 1)")
                .num_args(1)
                .value_parser(ValueParser::new(parse_frame_limit))
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("boot")
                .long("boot")
                .value_name("MODE")
                .help("Boot mode: cold | post-bios")
                .num_args(1)
                .value_parser(["cold", "post-bios"])
                .default_value("post-bios")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("serial")
                .long("serial")
                .value_name("MODE")
                .help("Serial output: off | live | final")
                .num_args(1)
                .value_parser(["off", "live", "final"])
                .default_value("off")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("quiet")
                .long("quiet")
                .help("Suppress startup/status logs")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("trace")
                .long("trace")
                .help("Enable trace logging (requires trace-enabled build)")
                .action(ArgAction::SetTrue),
        )
}

fn run_config_from_matches(matches: clap::ArgMatches) -> Result<RunConfig, ConfigError> {
    let rom_path = matches
        .get_one::<String>("rom_path")
        .map(PathBuf::from)
        .ok_or_else(|| ConfigError::new("missing required ROM path argument (<ROM_PATH>)"))?;

    let frame_limit = matches
        .get_one::<u64>("frames")
        .copied()
        .and_then(NonZeroU64::new);

    let boot_mode = matches
        .get_one::<String>("boot")
        .and_then(|value| BootMode::from_cli_value(value))
        .ok_or_else(|| ConfigError::new("invalid boot mode"))?;

    let serial_mode = matches
        .get_one::<String>("serial")
        .and_then(|value| SerialMode::from_cli_value(value))
        .ok_or_else(|| ConfigError::new("invalid serial mode"))?;

    Ok(RunConfig {
        rom_path,
        frame_limit,
        boot_mode,
        serial_mode,
        quiet: matches.get_flag("quiet"),
        trace: matches.get_flag("trace"),
    })
}

fn parse_frame_limit(raw: &str) -> Result<u64, String> {
    let value = raw
        .parse::<u64>()
        .map_err(|_| "expected a positive integer".to_string())?;

    if value == 0 {
        return Err("expected a value >= 1".to_string());
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;
    use std::path::PathBuf;

    use super::{BootMode, CliRequest, RunConfig, SerialMode};

    #[test]
    fn defaults_apply_when_optional_flags_are_absent() {
        let config =
            RunConfig::parse_cli_args(["roms/tetris.gb"]).expect("expected config to parse");

        assert_eq!(config.rom_path, PathBuf::from("roms/tetris.gb"));
        assert_eq!(config.frame_limit, None);
        assert_eq!(config.boot_mode, BootMode::PostBios);
        assert_eq!(config.serial_mode, SerialMode::Off);
        assert!(!config.quiet);
        assert!(!config.trace);
    }

    #[test]
    fn parser_accepts_all_supported_run_options() {
        let config = RunConfig::parse_cli_args([
            "--frames", "1440", "--boot", "cold", "--serial", "live", "--quiet", "--trace",
            "rom.gb",
        ])
        .expect("expected config to parse");

        assert_eq!(config.rom_path, PathBuf::from("rom.gb"));
        assert_eq!(config.frame_limit, NonZeroU64::new(1440));
        assert_eq!(config.boot_mode, BootMode::Cold);
        assert_eq!(config.serial_mode, SerialMode::Live);
        assert!(config.quiet);
        assert!(config.trace);
    }

    #[test]
    fn parser_rejects_missing_rom_argument() {
        let error = RunConfig::parse_cli_args(["--quiet"]).expect_err("expected error");
        assert!(
            error
                .to_string()
                .contains("the following required arguments were not provided")
        );
    }

    #[test]
    fn parser_rejects_unknown_option() {
        let error = RunConfig::parse_cli_args(["--wat", "rom.gb"]).expect_err("expected error");
        assert!(error.to_string().contains("unexpected argument '--wat'"));
        assert!(error.to_string().contains("to pass '--wat' as a value"));
    }

    #[test]
    fn parser_rejects_missing_value_for_frames() {
        let error = RunConfig::parse_cli_args(["--frames"]).expect_err("expected error");
        assert!(
            error
                .to_string()
                .contains("a value is required for '--frames <N>'")
        );
    }

    #[test]
    fn parser_rejects_missing_value_when_next_token_is_another_option() {
        let error = RunConfig::parse_cli_args(["--frames", "--boot", "cold", "rom.gb"])
            .expect_err("expected error");
        assert!(
            error
                .to_string()
                .contains("a value is required for '--frames <N>'")
        );
    }

    #[test]
    fn parser_rejects_duplicate_single_value_options() {
        let error = RunConfig::parse_cli_args(["--boot", "cold", "--boot", "post-bios", "rom.gb"])
            .expect_err("expected duplicate option error");
        assert!(
            error
                .to_string()
                .contains("the argument '--boot <MODE>' cannot be used multiple times")
        );
    }

    #[test]
    fn parser_rejects_invalid_frame_limit_values() {
        let zero = RunConfig::parse_cli_args(["--frames", "0", "rom.gb"])
            .expect_err("expected invalid frame value");
        assert!(zero.to_string().contains("expected a value >= 1"));

        let non_numeric = RunConfig::parse_cli_args(["--frames", "abc", "rom.gb"])
            .expect_err("expected invalid frame value");
        assert!(
            non_numeric
                .to_string()
                .contains("expected a positive integer")
        );
    }

    #[test]
    fn parser_rejects_invalid_boot_and_serial_modes() {
        let boot = RunConfig::parse_cli_args(["--boot", "fast", "rom.gb"])
            .expect_err("expected invalid boot mode");
        assert!(boot.to_string().contains("invalid value 'fast'"));

        let serial = RunConfig::parse_cli_args(["--serial", "stream", "rom.gb"])
            .expect_err("expected invalid serial mode");
        assert!(serial.to_string().contains("invalid value 'stream'"));
    }

    #[test]
    fn cli_request_parser_recognizes_help_flags() {
        let long = RunConfig::parse_cli_request(["--help"]).expect("expected help request");
        let CliRequest::Help(text) = long else {
            panic!("expected help request");
        };
        assert!(text.contains("Usage:"));
        assert!(text.contains("EXAMPLES:"));

        let short = RunConfig::parse_cli_request(["-h"]).expect("expected help request");
        assert!(matches!(short, CliRequest::Help(_)));
    }

    #[test]
    fn parse_cli_args_surfaces_help_text_as_config_error() {
        let error = RunConfig::parse_cli_args(["--help"]).expect_err("expected help text error");
        assert!(error.to_string().contains("Usage:"));
        assert!(error.to_string().contains("EXAMPLES:"));
    }

    #[test]
    fn cli_request_parser_recognizes_version_flags() {
        let long = RunConfig::parse_cli_request(["--version"]).expect("expected version request");
        let CliRequest::Version(text) = long else {
            panic!("expected version request");
        };
        assert!(text.contains(env!("CARGO_PKG_VERSION")));

        let short = RunConfig::parse_cli_request(["-V"]).expect("expected version request");
        assert!(matches!(short, CliRequest::Version(_)));
    }

    #[test]
    fn cli_request_parser_returns_run_config_for_standard_invocation() {
        let request = RunConfig::parse_cli_request(["--quiet", "rom.gb"]).expect("expected run");
        let CliRequest::Run(config) = request else {
            panic!("expected run request");
        };

        assert_eq!(config.rom_path, PathBuf::from("rom.gb"));
        assert!(config.quiet);
    }
}
