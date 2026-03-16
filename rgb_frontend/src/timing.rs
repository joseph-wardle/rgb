//! Frame pacing.
//!
//! The DMG runs at 4,194,304 T-cycles per second and draws one frame every
//! 70,224 T-cycles, giving a refresh rate of ≈59.7273 Hz.  [`FramePacer`]
//! sleeps the emulator thread for the remainder of each frame budget so the
//! game runs at the correct speed.
//!
//! On native, the pacer sleeps via `std::thread::sleep`.
//!
//! On WASM, `std::time::Instant` is not available and winit drives redraws via
//! `requestAnimationFrame` which fires at the *monitor* refresh rate — which
//! may be 60, 120, or 144 Hz, all faster than the Game Boy's 59.7 Hz.  The
//! pacer tracks elapsed time using `js_sys::Date::now()` (millisecond wall
//! clock) and exposes [`FramePacer::is_frame_due`]: the app calls this before
//! stepping the emulator and skips the work when not enough time has elapsed.

use std::time::Duration;

/// Duration of one DMG frame: 70,224 T-cycles ÷ 4,194,304 Hz ≈ 16.743 ms.
pub const FRAME_DURATION: Duration = Duration::from_nanos(16_742_706);

/// Frame duration as a floating-point number of milliseconds, for the WASM
/// clock which returns `f64` milliseconds.
#[cfg(target_arch = "wasm32")]
const FRAME_MS: f64 = 16.742_706;

/// Sleeps to maintain the correct frame rate on native targets; uses a
/// wall-clock gate on WASM where `std::thread::sleep` is unavailable.
pub struct FramePacer {
    /// Native only: timestamp of the last `begin_frame` call.
    #[cfg(not(target_arch = "wasm32"))]
    frame_start: std::time::Instant,

    /// WASM only: `js_sys::Date::now()` value at the last `begin_frame` call.
    #[cfg(target_arch = "wasm32")]
    frame_start_ms: f64,
}

impl FramePacer {
    pub fn new() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            frame_start: std::time::Instant::now(),

            #[cfg(target_arch = "wasm32")]
            frame_start_ms: js_sys::Date::now(),
        }
    }

    /// Mark the beginning of a new frame.
    ///
    /// On native, records the current instant.  On WASM, advances the
    /// deadline by exactly one frame period rather than snapping to `now()` —
    /// this carries any overshoot forward so the long-run frame rate stays
    /// accurate even when individual frames are slightly late.
    pub fn begin_frame(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.frame_start = std::time::Instant::now();
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.frame_start_ms += FRAME_MS;
            // If the deadline has drifted far into the past (e.g. tab was
            // backgrounded), clamp it to one frame behind now so we don't
            // emit a burst of frames on resume.
            let now = js_sys::Date::now();
            if now - self.frame_start_ms > FRAME_MS {
                self.frame_start_ms = now - FRAME_MS;
            }
        }
    }

    /// Returns `true` when at least one Game Boy frame period has elapsed
    /// since the last `begin_frame` call.
    ///
    /// On native this always returns `true` — the emulator runs on a
    /// dedicated thread paced by `wait()`.  On WASM, `requestAnimationFrame`
    /// may fire faster than 59.7 Hz, so the app polls this to avoid running
    /// the emulator too fast.
    pub fn is_frame_due(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            true // native pacing is handled by wait()
        }
        #[cfg(target_arch = "wasm32")]
        {
            js_sys::Date::now() - self.frame_start_ms >= FRAME_MS
        }
    }

    /// Sleep for the remainder of the frame budget.  If the frame already
    /// took longer than [`FRAME_DURATION`] the call returns immediately
    /// (no frame skipping — the next frame runs right away).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn wait(&self) {
        let elapsed = self.frame_start.elapsed();
        if elapsed < FRAME_DURATION {
            std::thread::sleep(FRAME_DURATION - elapsed);
        }
    }

    /// No-op on WASM: `requestAnimationFrame` handles pacing; `is_frame_due`
    /// provides the time gate instead.
    #[cfg(target_arch = "wasm32")]
    pub fn wait(&self) {}
}
