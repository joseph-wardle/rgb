//! Optional tracing setup for a single CLI run.
//!
//! Responsibility boundary:
//! - configures trace subscriber state when requested
//! - isolates feature-gated behavior behind one API
//! - returns clear runtime errors when `--trace` is unavailable in the build

use crate::error::CliError;

/// Active trace session for the current CLI run.
///
/// The session owns any runtime trace-dispatch state needed for the duration
/// of the emulator loop. Keeping this as a concrete type makes the call site
/// explicit and keeps setup/teardown centralized.
#[derive(Debug)]
pub struct TraceSession {
    #[cfg(feature = "trace")]
    _dispatch: Option<tracing::Dispatch>,
    #[cfg(feature = "trace")]
    _guard: Option<tracing::dispatcher::DefaultGuard>,
}

impl TraceSession {
    fn disabled() -> Self {
        Self {
            #[cfg(feature = "trace")]
            _dispatch: None,
            #[cfg(feature = "trace")]
            _guard: None,
        }
    }

    #[cfg(feature = "trace")]
    fn enabled(dispatch: tracing::Dispatch, guard: tracing::dispatcher::DefaultGuard) -> Self {
        Self {
            _dispatch: Some(dispatch),
            _guard: Some(guard),
        }
    }
}

/// Initializes optional trace logging for this process invocation.
///
/// Behavior:
/// - `trace_requested = false`: no-op
/// - `trace_requested = true` with `trace` feature: attach a thread-local
///   subscriber for the current run
/// - `trace_requested = true` without `trace` feature: return actionable error
pub fn setup_trace(trace_requested: bool) -> Result<TraceSession, CliError> {
    if !trace_requested {
        return Ok(TraceSession::disabled());
    }

    setup_trace_enabled()
}

#[cfg(feature = "trace")]
fn setup_trace_enabled() -> Result<TraceSession, CliError> {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_target(true)
        .without_time()
        .finish();

    let dispatch = tracing::Dispatch::new(subscriber);
    let guard = tracing::dispatcher::set_default(&dispatch);
    Ok(TraceSession::enabled(dispatch, guard))
}

#[cfg(not(feature = "trace"))]
fn setup_trace_enabled() -> Result<TraceSession, CliError> {
    Err(CliError::TraceFeatureRequired)
}

#[cfg(test)]
mod tests {
    use super::setup_trace;
    #[cfg(not(feature = "trace"))]
    use crate::error::CliErrorKind;

    #[test]
    fn setup_trace_is_noop_when_not_requested() {
        let result = setup_trace(false);
        assert!(result.is_ok());
    }

    #[cfg(not(feature = "trace"))]
    #[test]
    fn setup_trace_reports_feature_gating_error_without_trace_build() {
        let error = setup_trace(true).expect_err("expected feature gating error");
        assert_eq!(error.kind(), CliErrorKind::Runtime);
        assert!(
            error
                .to_string()
                .contains("Rebuild with `--features trace`")
        );
    }

    #[cfg(feature = "trace")]
    #[test]
    fn setup_trace_can_be_enabled_for_current_run() {
        let session = setup_trace(true);
        assert!(session.is_ok());
    }
}
