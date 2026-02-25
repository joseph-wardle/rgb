use std::borrow::Cow;
use std::io::{self, Write};
use std::num::NonZeroU64;

use rgb_core::gameboy::DMG;

use crate::config::{RunConfig, SerialMode};
use crate::error::CliError;

const PROGRESS_REPORT_INTERVAL_FRAMES: u64 = 600;
const PROGRESS_MIN_FRAME_LIMIT_FOR_PROGRESS: u64 = PROGRESS_REPORT_INTERVAL_FRAMES * 2;

/// One-shot execution report emitted when the runner exits via an explicit
/// stop condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    pub frames_executed: u64,
    pub stop_reason: StopReason,
    pub serial_output: String,
}

/// Deterministic reason why a bounded run loop stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    FrameLimitReached {
        frame_limit: NonZeroU64,
    },
    SerialVerdictReached {
        condition: SerialVerdictCondition,
        verdict: SerialVerdict,
    },
}

/// Text verdict extracted from serial output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialVerdict {
    Passed,
    Failed,
}

impl SerialVerdict {
    fn label(self) -> &'static str {
        match self {
            SerialVerdict::Passed => "passed",
            SerialVerdict::Failed => "failed",
        }
    }
}

/// Optional serial-output predicate that can terminate the run loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialVerdictCondition {
    /// Stops when serial output contains Blargg-style `Passed` or `Failed`.
    BlarggPassFail,
}

impl SerialVerdictCondition {
    fn evaluate(self, serial_output: &str) -> Option<SerialVerdict> {
        match self {
            SerialVerdictCondition::BlarggPassFail => {
                if serial_output.contains("Failed") {
                    Some(SerialVerdict::Failed)
                } else if serial_output.contains("Passed") {
                    Some(SerialVerdict::Passed)
                } else {
                    None
                }
            }
        }
    }
}

impl StopReason {
    pub fn summary_label(&self) -> String {
        match self {
            StopReason::FrameLimitReached { frame_limit } => {
                format!("frame limit reached ({frame_limit})")
            }
            StopReason::SerialVerdictReached {
                condition: SerialVerdictCondition::BlarggPassFail,
                verdict,
            } => format!("serial verdict ({})", verdict.label()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ExitConditions {
    frame_limit: Option<NonZeroU64>,
    serial_verdict: Option<SerialVerdictCondition>,
}

impl ExitConditions {
    fn from_config(config: &RunConfig) -> Self {
        Self {
            frame_limit: config.frame_limit,
            serial_verdict: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ProgressReporter {
    next_report_frame: u64,
    enabled: bool,
}

impl ProgressReporter {
    fn from_config(config: &RunConfig) -> Self {
        let long_enough = config
            .frame_limit
            .map(|limit| limit.get() >= PROGRESS_MIN_FRAME_LIMIT_FOR_PROGRESS)
            .unwrap_or(true);
        let headless = config.serial_mode == SerialMode::Off;
        let enabled = !config.quiet && headless && long_enough;

        Self {
            next_report_frame: PROGRESS_REPORT_INTERVAL_FRAMES,
            enabled,
        }
    }

    fn should_emit(&self, frames_executed: u64) -> bool {
        self.enabled && frames_executed >= self.next_report_frame
    }

    fn mark_emitted(&mut self) {
        self.next_report_frame = self
            .next_report_frame
            .saturating_add(PROGRESS_REPORT_INTERVAL_FRAMES);
    }

    fn format_line(frames_executed: u64, frame_limit: Option<NonZeroU64>) -> String {
        match frame_limit {
            Some(limit) => format!("progress: frame {frames_executed}/{}", limit.get()),
            None => format!("progress: frame {frames_executed}"),
        }
    }
}

const LIVE_SERIAL_PARTIAL_FLUSH_INTERVAL_FRAMES: u64 = 30;
const LIVE_SERIAL_PARTIAL_FLUSH_BYTE_THRESHOLD: usize = 120;

/// Incremental serial-output formatter used by `--serial live`.
///
/// Incoming bytes are buffered until a complete line (`\n`) is available.
/// If no newline arrives, a partial line is flushed after a small frame delay
/// (or once enough bytes accumulate) so output remains readable but responsive.
#[derive(Debug, Default)]
struct LiveSerialPassthrough {
    cursor: usize,
    pending_line: Vec<u8>,
    frames_since_emit: u64,
}

impl LiveSerialPassthrough {
    fn collect_lines(&mut self, serial_buffer: &[u8]) -> Vec<Vec<u8>> {
        self.collect_new_bytes(serial_buffer);

        let mut emitted = self.drain_complete_lines();
        if emitted.is_empty() {
            self.frames_since_emit = self.frames_since_emit.saturating_add(1);
            if self.should_flush_partial_line() {
                emitted.push(self.flush_partial_line());
                self.frames_since_emit = 0;
            }
        } else {
            self.frames_since_emit = 0;
        }

        emitted
    }

    fn finalize(&mut self) -> Option<Vec<u8>> {
        if self.pending_line.is_empty() {
            return None;
        }

        self.frames_since_emit = 0;
        Some(self.flush_partial_line())
    }

    fn collect_new_bytes(&mut self, serial_buffer: &[u8]) {
        // Defensive recovery in case a future core change clears the serial
        // buffer mid-run.
        if serial_buffer.len() < self.cursor {
            self.cursor = 0;
            self.pending_line.clear();
        }

        if serial_buffer.len() > self.cursor {
            self.pending_line
                .extend_from_slice(&serial_buffer[self.cursor..]);
            self.cursor = serial_buffer.len();
        }
    }

    fn drain_complete_lines(&mut self) -> Vec<Vec<u8>> {
        let mut lines = Vec::new();
        while let Some(newline_index) = self.pending_line.iter().position(|&byte| byte == b'\n') {
            let line = self
                .pending_line
                .drain(..=newline_index)
                .collect::<Vec<_>>();
            lines.push(line);
        }
        lines
    }

    fn should_flush_partial_line(&self) -> bool {
        !self.pending_line.is_empty()
            && (self.pending_line.len() >= LIVE_SERIAL_PARTIAL_FLUSH_BYTE_THRESHOLD
                || self.frames_since_emit >= LIVE_SERIAL_PARTIAL_FLUSH_INTERVAL_FRAMES)
    }

    fn flush_partial_line(&mut self) -> Vec<u8> {
        let mut line = std::mem::take(&mut self.pending_line);
        line.push(b'\n');
        line
    }
}

/// Deterministic frame-by-frame execution driver for `DMG`.
///
/// The runner owns both host config and emulator state. Exit behavior is
/// explicit and centralized:
/// 1. optional frame cap
/// 2. optional serial verdict condition
/// 3. otherwise unbounded execution until host process termination
pub struct Runner {
    config: RunConfig,
    gameboy: DMG,
    exit_conditions: ExitConditions,
    progress_reporter: ProgressReporter,
    frames_executed: u64,
    live_serial_passthrough: LiveSerialPassthrough,
}

impl Runner {
    pub fn new(config: RunConfig, gameboy: DMG) -> Self {
        let exit_conditions = ExitConditions::from_config(&config);
        let progress_reporter = ProgressReporter::from_config(&config);
        Self {
            config,
            gameboy,
            exit_conditions,
            progress_reporter,
            frames_executed: 0,
            live_serial_passthrough: LiveSerialPassthrough::default(),
        }
    }

    /// Enables serial verdict based early termination.
    pub fn with_serial_verdict_condition(mut self, condition: SerialVerdictCondition) -> Self {
        self.exit_conditions.serial_verdict = Some(condition);
        self
    }

    /// Runs until an explicit stop condition is met.
    ///
    /// When no stop condition is configured, this function intentionally runs
    /// forever and relies on host process termination (e.g. Ctrl-C).
    pub fn run(&mut self) -> Result<RunReport, CliError> {
        loop {
            self.step_one_frame()?;
            self.emit_progress_line_if_enabled()?;

            if let Some(stop_reason) = self.evaluate_exit_conditions() {
                self.emit_live_serial_tail_if_enabled()?;
                self.emit_final_serial_output_if_enabled()?;
                return Ok(self.build_report(stop_reason));
            }
        }
    }

    fn step_one_frame(&mut self) -> Result<(), CliError> {
        self.gameboy.step_frame();
        self.frames_executed += 1;
        self.emit_live_serial_delta_if_enabled()
    }

    fn evaluate_exit_conditions(&self) -> Option<StopReason> {
        if let Some(frame_limit) = self.exit_conditions.frame_limit
            && self.frames_executed >= frame_limit.get()
        {
            return Some(StopReason::FrameLimitReached { frame_limit });
        }

        let condition = self.exit_conditions.serial_verdict?;
        let serial_text: Cow<'_, str> =
            String::from_utf8_lossy(self.gameboy.serial().output_bytes());
        let verdict = condition.evaluate(serial_text.as_ref())?;
        Some(StopReason::SerialVerdictReached { condition, verdict })
    }

    fn build_report(&self, stop_reason: StopReason) -> RunReport {
        RunReport {
            frames_executed: self.frames_executed,
            stop_reason,
            serial_output: self.gameboy.serial_output(),
        }
    }

    fn emit_live_serial_delta_if_enabled(&mut self) -> Result<(), CliError> {
        if self.config.serial_mode != SerialMode::Live {
            return Ok(());
        }

        let lines = self
            .live_serial_passthrough
            .collect_lines(self.gameboy.serial().output_bytes());
        for line in lines {
            Self::write_stdout_bytes(&line)?;
        }
        Ok(())
    }

    fn emit_progress_line_if_enabled(&mut self) -> Result<(), CliError> {
        if !self.progress_reporter.should_emit(self.frames_executed) {
            return Ok(());
        }

        let line = ProgressReporter::format_line(self.frames_executed, self.config.frame_limit);
        Self::write_stdout_line(&line)?;
        self.progress_reporter.mark_emitted();
        Ok(())
    }

    fn emit_live_serial_tail_if_enabled(&mut self) -> Result<(), CliError> {
        if self.config.serial_mode != SerialMode::Live {
            return Ok(());
        }

        if let Some(partial_line) = self.live_serial_passthrough.finalize() {
            Self::write_stdout_bytes(&partial_line)?;
        }
        Ok(())
    }

    fn emit_final_serial_output_if_enabled(&self) -> Result<(), CliError> {
        if self.config.serial_mode != SerialMode::Final {
            return Ok(());
        }

        let bytes = self.gameboy.serial().output_bytes();
        if bytes.is_empty() {
            return Ok(());
        }

        Self::write_stdout_bytes(bytes)
    }

    fn write_stdout_bytes(bytes: &[u8]) -> Result<(), CliError> {
        const STDOUT_PATH: &str = "<stdout>";

        let mut stdout = io::stdout().lock();
        stdout
            .write_all(bytes)
            .map_err(|source| CliError::io("writing serial output", STDOUT_PATH, source))?;
        stdout
            .flush()
            .map_err(|source| CliError::io("flushing serial output", STDOUT_PATH, source))
    }

    fn write_stdout_line(line: &str) -> Result<(), CliError> {
        let mut bytes = line.as_bytes().to_vec();
        bytes.push(b'\n');
        Self::write_stdout_bytes(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::num::NonZeroU64;
    use std::path::Path;

    use tempfile::NamedTempFile;

    use super::{
        ExitConditions, LIVE_SERIAL_PARTIAL_FLUSH_INTERVAL_FRAMES, LiveSerialPassthrough,
        PROGRESS_MIN_FRAME_LIMIT_FOR_PROGRESS, ProgressReporter, Runner, SerialVerdict,
        SerialVerdictCondition, StopReason,
    };
    use crate::config::{BootMode, RunConfig, SerialMode};
    use crate::emulator::construct_gameboy;
    use crate::rom::load_rom;

    #[test]
    fn exit_conditions_default_to_frame_limit_only() {
        let config = run_config(
            Path::new("rom.gb"),
            Some(NonZeroU64::new(60).expect("non-zero")),
            SerialMode::Off,
        );
        let conditions = ExitConditions::from_config(&config);

        assert_eq!(conditions.frame_limit, config.frame_limit);
        assert_eq!(conditions.serial_verdict, None);
    }

    #[test]
    fn serial_verdict_condition_parses_blargg_style_output() {
        let condition = SerialVerdictCondition::BlarggPassFail;

        assert_eq!(
            condition.evaluate("01: Passed"),
            Some(SerialVerdict::Passed)
        );
        assert_eq!(
            condition.evaluate("01: Failed"),
            Some(SerialVerdict::Failed)
        );
        assert_eq!(
            condition.evaluate("Failed\nPassed"),
            Some(SerialVerdict::Failed)
        );
        assert_eq!(condition.evaluate("still running"), None);
    }

    #[test]
    fn stop_reason_summary_is_human_friendly() {
        let frame_limit = NonZeroU64::new(1200).expect("non-zero");
        let limit_reason = StopReason::FrameLimitReached { frame_limit };
        assert_eq!(limit_reason.summary_label(), "frame limit reached (1200)");

        let pass_reason = StopReason::SerialVerdictReached {
            condition: SerialVerdictCondition::BlarggPassFail,
            verdict: SerialVerdict::Passed,
        };
        assert_eq!(pass_reason.summary_label(), "serial verdict (passed)");

        let fail_reason = StopReason::SerialVerdictReached {
            condition: SerialVerdictCondition::BlarggPassFail,
            verdict: SerialVerdict::Failed,
        };
        assert_eq!(fail_reason.summary_label(), "serial verdict (failed)");
    }

    #[test]
    fn progress_reporter_is_enabled_for_long_headless_runs() {
        let mut config = run_config(
            Path::new("rom.gb"),
            Some(NonZeroU64::new(PROGRESS_MIN_FRAME_LIMIT_FOR_PROGRESS).expect("non-zero")),
            SerialMode::Off,
        );
        config.quiet = false;

        let reporter = ProgressReporter::from_config(&config);
        assert!(reporter.enabled);
        assert_eq!(reporter.next_report_frame, 600);
    }

    #[test]
    fn progress_reporter_is_disabled_for_short_or_non_headless_runs() {
        let short = run_config(
            Path::new("rom.gb"),
            Some(NonZeroU64::new(10).expect("non-zero")),
            SerialMode::Off,
        );
        assert!(!ProgressReporter::from_config(&short).enabled);

        let live_serial = run_config(
            Path::new("rom.gb"),
            Some(NonZeroU64::new(2400).expect("non-zero")),
            SerialMode::Live,
        );
        assert!(!ProgressReporter::from_config(&live_serial).enabled);
    }

    #[test]
    fn progress_reporter_formats_unbounded_and_bounded_lines() {
        let bounded = ProgressReporter::format_line(600, NonZeroU64::new(5000));
        assert_eq!(bounded, "progress: frame 600/5000");

        let unbounded = ProgressReporter::format_line(600, None);
        assert_eq!(unbounded, "progress: frame 600");
    }

    #[test]
    fn live_passthrough_emits_only_new_complete_lines() {
        let mut passthrough = LiveSerialPassthrough::default();

        let first = passthrough.collect_lines(b"line one\nline");
        assert_eq!(first, vec![b"line one\n".to_vec()]);

        let second = passthrough.collect_lines(b"line one\nline two\n");
        assert_eq!(second, vec![b"line two\n".to_vec()]);

        let third = passthrough.collect_lines(b"line one\nline two\n");
        assert!(third.is_empty());
    }

    #[test]
    fn live_passthrough_flushes_partial_lines_after_throttle_interval() {
        let mut passthrough = LiveSerialPassthrough::default();

        assert!(passthrough.collect_lines(b"partial").is_empty());

        for _ in 0..=LIVE_SERIAL_PARTIAL_FLUSH_INTERVAL_FRAMES {
            let emitted = passthrough.collect_lines(b"partial");
            if !emitted.is_empty() {
                assert_eq!(emitted, vec![b"partial\n".to_vec()]);
                return;
            }
        }

        panic!("expected throttled partial-line flush");
    }

    #[test]
    fn live_passthrough_finalize_flushes_pending_partial_line() {
        let mut passthrough = LiveSerialPassthrough::default();

        assert!(passthrough.collect_lines(b"tail").is_empty());
        assert_eq!(passthrough.finalize(), Some(b"tail\n".to_vec()));
        assert_eq!(passthrough.finalize(), None);
    }

    #[test]
    fn runner_stops_at_configured_frame_limit() {
        let mut rom_file = write_test_rom("RUNLOOP", 0x00, 0x00, 0x00);
        rom_file.flush().expect("flush ROM file");

        let frame_limit = NonZeroU64::new(2).expect("non-zero");
        let config = run_config(rom_file.path(), Some(frame_limit), SerialMode::Off);
        let loaded_rom = load_rom(rom_file.path()).expect("load ROM");
        let gameboy = construct_gameboy(config.boot_mode, loaded_rom);
        let mut runner = Runner::new(config, gameboy)
            .with_serial_verdict_condition(SerialVerdictCondition::BlarggPassFail);

        let report = runner.run().expect("runner should stop at frame limit");

        assert_eq!(report.frames_executed, frame_limit.get());
        assert_eq!(
            report.stop_reason,
            StopReason::FrameLimitReached { frame_limit }
        );
        assert!(report.serial_output.is_empty());
    }

    fn run_config(
        path: &Path,
        frame_limit: Option<NonZeroU64>,
        serial_mode: SerialMode,
    ) -> RunConfig {
        RunConfig {
            rom_path: path.to_path_buf(),
            frame_limit,
            boot_mode: BootMode::PostBios,
            serial_mode,
            quiet: true,
            trace: false,
        }
    }

    fn write_test_rom(
        title: &str,
        cartridge_type: u8,
        rom_size_code: u8,
        ram_size_code: u8,
    ) -> NamedTempFile {
        let mut bytes = vec![0; 0x8000];
        let title_bytes = title.as_bytes();
        let title_len = title_bytes.len().min(15);
        bytes[0x134..0x134 + title_len].copy_from_slice(&title_bytes[..title_len]);
        bytes[0x147] = cartridge_type;
        bytes[0x148] = rom_size_code;
        bytes[0x149] = ram_size_code;

        let mut file = NamedTempFile::new().expect("create temp ROM file");
        file.write_all(&bytes).expect("write ROM bytes");
        file
    }
}
