use std::error::Error;
use std::ffi::OsString;
use std::fmt::{self, Display, Formatter};
use std::num::NonZeroU64;
use std::path::PathBuf;

/// Selects the CPU/boot initialization path used to construct `DMG`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BootMode {
    Cold,
    #[default]
    PostBios,
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

impl RunConfig {
    /// Parses CLI arguments (excluding argv[0]) into a validated configuration.
    pub fn parse_cli_args<I, S>(args: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let raw_args: Vec<OsString> = args.into_iter().map(Into::into).collect();
        let mut parser = RunConfigParser::new(raw_args);
        parser.parse()
    }
}

/// User-facing configuration validation/parsing failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    MissingRomPath,
    UnexpectedPositional(String),
    UnknownOption(String),
    MissingOptionValue { option: &'static str },
    DuplicateOption { option: &'static str },
    InvalidFrames(String),
    InvalidBootMode(String),
    InvalidSerialMode(String),
    NonUtf8Argument,
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::MissingRomPath => {
                f.write_str("missing required ROM path argument (<ROM_PATH>)")
            }
            ConfigError::UnexpectedPositional(value) => {
                write!(
                    f,
                    "unexpected positional argument '{value}'; expected only one <ROM_PATH>"
                )
            }
            ConfigError::UnknownOption(option) => write!(f, "unknown option '{option}'"),
            ConfigError::MissingOptionValue { option } => {
                write!(f, "missing value for option '{option}'")
            }
            ConfigError::DuplicateOption { option } => {
                write!(f, "option '{option}' may only be specified once")
            }
            ConfigError::InvalidFrames(value) => {
                write!(
                    f,
                    "invalid frame limit '{value}'; expected a positive integer (>= 1)"
                )
            }
            ConfigError::InvalidBootMode(value) => {
                write!(
                    f,
                    "invalid boot mode '{value}'; expected one of: cold, post-bios"
                )
            }
            ConfigError::InvalidSerialMode(value) => {
                write!(
                    f,
                    "invalid serial mode '{value}'; expected one of: off, live, final"
                )
            }
            ConfigError::NonUtf8Argument => {
                f.write_str("arguments must be valid UTF-8 for this CLI")
            }
        }
    }
}

impl Error for ConfigError {}

#[derive(Debug)]
struct RunConfigParser {
    args: Vec<OsString>,
    index: usize,
    stop_option_parsing: bool,

    rom_path: Option<PathBuf>,
    frame_limit: Option<NonZeroU64>,
    boot_mode: BootMode,
    serial_mode: SerialMode,
    quiet: bool,
    trace: bool,

    seen_frames: bool,
    seen_boot: bool,
    seen_serial: bool,
}

impl RunConfigParser {
    fn new(args: Vec<OsString>) -> Self {
        Self {
            args,
            index: 0,
            stop_option_parsing: false,
            rom_path: None,
            frame_limit: None,
            boot_mode: BootMode::default(),
            serial_mode: SerialMode::default(),
            quiet: false,
            trace: false,
            seen_frames: false,
            seen_boot: false,
            seen_serial: false,
        }
    }

    fn parse(&mut self) -> Result<RunConfig, ConfigError> {
        while let Some(current) = self.next_arg()? {
            if !self.stop_option_parsing && current == "--" {
                self.stop_option_parsing = true;
                continue;
            }

            if !self.stop_option_parsing && current.starts_with('-') {
                self.parse_option(&current)?;
            } else {
                self.parse_positional(current)?;
            }
        }

        let rom_path = self.rom_path.clone().ok_or(ConfigError::MissingRomPath)?;

        Ok(RunConfig {
            rom_path,
            frame_limit: self.frame_limit,
            boot_mode: self.boot_mode,
            serial_mode: self.serial_mode,
            quiet: self.quiet,
            trace: self.trace,
        })
    }

    fn next_arg(&mut self) -> Result<Option<String>, ConfigError> {
        if self.index >= self.args.len() {
            return Ok(None);
        }

        let value = self.args[self.index]
            .to_str()
            .map(str::to_owned)
            .ok_or(ConfigError::NonUtf8Argument)?;
        self.index += 1;
        Ok(Some(value))
    }

    fn parse_option(&mut self, option: &str) -> Result<(), ConfigError> {
        match option {
            "--frames" => {
                if self.seen_frames {
                    return Err(ConfigError::DuplicateOption { option: "--frames" });
                }
                self.seen_frames = true;
                let raw = self.next_required_value("--frames")?;
                self.frame_limit = Some(parse_frame_limit(&raw)?);
                Ok(())
            }
            "--boot" => {
                if self.seen_boot {
                    return Err(ConfigError::DuplicateOption { option: "--boot" });
                }
                self.seen_boot = true;
                let raw = self.next_required_value("--boot")?;
                self.boot_mode = parse_boot_mode(&raw)?;
                Ok(())
            }
            "--serial" => {
                if self.seen_serial {
                    return Err(ConfigError::DuplicateOption { option: "--serial" });
                }
                self.seen_serial = true;
                let raw = self.next_required_value("--serial")?;
                self.serial_mode = parse_serial_mode(&raw)?;
                Ok(())
            }
            "--quiet" => {
                self.quiet = true;
                Ok(())
            }
            "--trace" => {
                self.trace = true;
                Ok(())
            }
            _ => Err(ConfigError::UnknownOption(option.to_string())),
        }
    }

    fn parse_positional(&mut self, value: String) -> Result<(), ConfigError> {
        if self.rom_path.is_some() {
            return Err(ConfigError::UnexpectedPositional(value));
        }

        self.rom_path = Some(PathBuf::from(value));
        Ok(())
    }

    fn next_required_value(&mut self, option: &'static str) -> Result<String, ConfigError> {
        match self.next_arg()? {
            Some(value) => Ok(value),
            None => Err(ConfigError::MissingOptionValue { option }),
        }
    }
}

fn parse_frame_limit(raw: &str) -> Result<NonZeroU64, ConfigError> {
    let value = raw
        .parse::<u64>()
        .map_err(|_| ConfigError::InvalidFrames(raw.to_string()))?;

    NonZeroU64::new(value).ok_or_else(|| ConfigError::InvalidFrames(raw.to_string()))
}

fn parse_boot_mode(raw: &str) -> Result<BootMode, ConfigError> {
    match raw {
        "cold" => Ok(BootMode::Cold),
        "post-bios" => Ok(BootMode::PostBios),
        _ => Err(ConfigError::InvalidBootMode(raw.to_string())),
    }
}

fn parse_serial_mode(raw: &str) -> Result<SerialMode, ConfigError> {
    match raw {
        "off" => Ok(SerialMode::Off),
        "live" => Ok(SerialMode::Live),
        "final" => Ok(SerialMode::Final),
        _ => Err(ConfigError::InvalidSerialMode(raw.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;
    use std::path::PathBuf;

    use super::{BootMode, ConfigError, RunConfig, SerialMode};

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
    fn parser_honors_double_dash_for_positional_values() {
        let config =
            RunConfig::parse_cli_args(["--quiet", "--", "--demo.gb"]).expect("expected success");

        assert_eq!(config.rom_path, PathBuf::from("--demo.gb"));
    }

    #[test]
    fn parser_rejects_missing_rom_argument() {
        let error = RunConfig::parse_cli_args(["--quiet"]).expect_err("expected missing ROM error");
        assert_eq!(error, ConfigError::MissingRomPath);
    }

    #[test]
    fn parser_rejects_extra_positional_arguments() {
        let error =
            RunConfig::parse_cli_args(["a.gb", "b.gb"]).expect_err("expected positional error");
        assert_eq!(error, ConfigError::UnexpectedPositional("b.gb".to_string()));
    }

    #[test]
    fn parser_rejects_unknown_option() {
        let error = RunConfig::parse_cli_args(["--wat", "rom.gb"]).expect_err("expected error");
        assert_eq!(error, ConfigError::UnknownOption("--wat".to_string()));
    }

    #[test]
    fn parser_rejects_missing_value_for_frames() {
        let error = RunConfig::parse_cli_args(["--frames"])
            .expect_err("expected missing option value for frames");
        assert_eq!(
            error,
            ConfigError::MissingOptionValue { option: "--frames" }
        );
    }

    #[test]
    fn parser_rejects_duplicate_single_value_options() {
        let error = RunConfig::parse_cli_args(["--boot", "cold", "--boot", "post-bios", "rom.gb"])
            .expect_err("expected duplicate option");
        assert_eq!(error, ConfigError::DuplicateOption { option: "--boot" });
    }

    #[test]
    fn parser_rejects_invalid_frame_limit_values() {
        let zero = RunConfig::parse_cli_args(["--frames", "0", "rom.gb"])
            .expect_err("expected invalid frame value");
        assert_eq!(zero, ConfigError::InvalidFrames("0".to_string()));

        let non_numeric = RunConfig::parse_cli_args(["--frames", "abc", "rom.gb"])
            .expect_err("expected invalid frame value");
        assert_eq!(non_numeric, ConfigError::InvalidFrames("abc".to_string()));
    }

    #[test]
    fn parser_rejects_invalid_boot_and_serial_modes() {
        let boot = RunConfig::parse_cli_args(["--boot", "fast", "rom.gb"])
            .expect_err("expected invalid boot mode");
        assert_eq!(boot, ConfigError::InvalidBootMode("fast".to_string()));

        let serial = RunConfig::parse_cli_args(["--serial", "stream", "rom.gb"])
            .expect_err("expected invalid serial mode");
        assert_eq!(serial, ConfigError::InvalidSerialMode("stream".to_string()));
    }
}
