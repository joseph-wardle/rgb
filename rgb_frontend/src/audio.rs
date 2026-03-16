//! Audio output abstraction.
//!
//! [`AudioSink`] is the trait that platform-specific audio backends implement.
//! The emulator loop calls [`AudioSink::push_samples`] once per frame with
//! interleaved stereo samples; the backend is responsible for buffering and
//! feeding them to the host audio device.
//!
//! Implementations live in consumer crates (`rgb_cli` uses cpal, `rgb_web`
//! uses the Web Audio API) so that `rgb_frontend` itself carries no
//! platform-specific audio dependencies.

/// Accepts audio samples from the emulator and delivers them to the host.
///
/// Implementations must be non-blocking: if the output buffer is full, excess
/// samples should be silently dropped rather than stalling the emulator thread.
pub trait AudioSink {
    /// Push interleaved stereo f32 samples (left, right, left, right, …).
    ///
    /// Sample values are in the range −1.0 to +1.0.  Called once per frame
    /// with approximately 1,000 sample pairs (~16.7 ms at 44,100 Hz).
    fn push_samples(&mut self, samples: &[f32]);
}

/// A no-op audio sink that discards all samples.
///
/// Used as a fallback when no audio device is available.
pub struct SilentSink;

impl AudioSink for SilentSink {
    fn push_samples(&mut self, _samples: &[f32]) {}
}
