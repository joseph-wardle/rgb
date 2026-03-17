//! Audio Processing Unit (APU).
//!
//! The DMG APU contains four sound channels mixed into a stereo output:
//!
//! | Ch | Type          | Notes                                    |
//! |----|---------------|------------------------------------------|
//! |  1 | Pulse + sweep | frequency sweep, duty cycle, envelope    |
//! |  2 | Pulse         | duty cycle and envelope, no sweep        |
//! |  3 | Wave          | 4-bit PCM from programmable wave RAM     |
//! |  4 | Noise         | LFSR-based pseudo-random noise           |
//!
//! ## Frame sequencer
//!
//! An internal 512 Hz clock (one tick every 8,192 T-cycles) drives a
//! modulo-8 step counter that gates the slower clocking units:
//!
//! | Step | Length (256 Hz) | Envelope (64 Hz) | Sweep (128 Hz) |
//! |------|-----------------|------------------|----------------|
//! |  0   | ✓               |                  |                |
//! |  1   |                 |                  |                |
//! |  2   | ✓               |                  | ✓              |
//! |  3   |                 |                  |                |
//! |  4   | ✓               |                  |                |
//! |  5   |                 |                  |                |
//! |  6   | ✓               |                  | ✓              |
//! |  7   |                 | ✓                |                |
//!
//! ## Register map
//!
//! | Range     | Contents                          |
//! |-----------|-----------------------------------|
//! | FF10–FF14 | Channel 1 (pulse + sweep)         |
//! | FF15–FF19 | Channel 2 (pulse)                 |
//! | FF1A–FF1E | Channel 3 (wave)                  |
//! | FF1F–FF23 | Channel 4 (noise)                 |
//! | FF24      | NR50 master volume / VIN routing  |
//! | FF25      | NR51 stereo panning               |
//! | FF26      | NR52 power / channel status       |
//! | FF30–FF3F | Wave RAM                          |

use crate::memory::Memory;

mod channel1;
mod channel2;
mod channel3;
mod channel4;
mod units;

use channel1::Channel1;
use channel2::Channel2;
use channel3::Channel3;
use channel4::Channel4;

// ---------------------------------------------------------------------------
// OR read masks
// ---------------------------------------------------------------------------
//
// These bits always read back as 1 regardless of the stored value.  They
// cover write-only fields (e.g. frequency bits, trigger) and unused bits.
// Indexed by (address − 0xFF10); entries 0x00–0x16 cover 0xFF10–0xFF26.
// Wave RAM (0xFF30–0xFF3F) has no masks and is handled separately.

#[rustfmt::skip]
const OR_MASK: [u8; 0x17] = [
    // FF10  FF11  FF12  FF13  FF14
       0x80, 0x3F, 0x00, 0xFF, 0xBF,
    // FF15  FF16  FF17  FF18  FF19
       0xFF, 0x3F, 0x00, 0xFF, 0xBF,
    // FF1A  FF1B  FF1C  FF1D  FF1E
       0x7F, 0xFF, 0x9F, 0xFF, 0xBF,
    // FF1F  FF20  FF21  FF22  FF23
       0xFF, 0xFF, 0x00, 0x00, 0xBF,
    // FF24  FF25  FF26
       0x00, 0x00, 0x70,
];

// ---------------------------------------------------------------------------
// Pulse duty-cycle waveform
// ---------------------------------------------------------------------------
//
// Each byte encodes 8 phase steps: bit N = 1 means phase N outputs "high".
// The four duty cycles:  12.5%, 25%, 50%, 75%.
//
//   Duty 0 (12.5%): -------X   only phase 7 is high
//   Duty 1 (25%):   X------X   phases 0 and 7
//   Duty 2 (50%):   X---XXXX   phases 0 and 4–7
//   Duty 3 (75%):   -XXXXXX-   phases 1–6
//
// Access: `(DUTY_PATTERNS[duty] >> phase) & 1`
#[rustfmt::skip]
const DUTY_PATTERNS: [u8; 4] = [0x01, 0x81, 0xF1, 0x7E];

// ---------------------------------------------------------------------------
// Timing constants
// ---------------------------------------------------------------------------

/// DMG APU master clock rate (T-cycles per second).
const APU_CLOCK_HZ: f32 = 4_194_304.0;

/// Audio output sample rate (Hz).
pub(crate) const SAMPLE_RATE: u32 = 44_100;

/// T-cycles per audio output sample.
const SAMPLE_PERIOD_T: f32 = APU_CLOCK_HZ / SAMPLE_RATE as f32; // ≈ 95.1085

/// System counter bit 12: the frame sequencer fires on each falling edge,
/// producing a 512 Hz clock (one tick every 8,192 T-cycles).
///
/// This bit is also part of DIV (DIV bit 4, since DIV = counter bits 8–15),
/// so a DIV write can trigger an immediate frame-sequencer tick — see
/// [`APU::notify_div_reset`].
const FRAME_SEQ_BIT: u16 = 1 << 12;

// ---------------------------------------------------------------------------
// APU
// ---------------------------------------------------------------------------

#[expect(clippy::upper_case_acronyms)]
pub(crate) struct APU {
    pub ch1: Channel1,
    pub ch2: Channel2,
    pub ch3: Channel3,
    pub ch4: Channel4,

    /// NR50 — master volume.
    /// Bits 6–4 = left volume (0–7), bits 2–0 = right volume (0–7).
    /// Hardware adds 1 before scaling: effective amplitude = (field + 1) / 8.
    /// Bit 7 / bit 3 route the cartridge's analogue VIN pin; not emulated.
    pub nr50: u8,

    /// NR51 — stereo panning.
    /// Bit (N+4) = Ch(N+1) to left output; bit N = Ch(N+1) to right output.
    pub nr51: u8,

    /// NR52 bit 7 — master power switch.
    /// When false all APU registers are cleared and reads return 0xFF.
    pub on: bool,

    // --- Timing state ---
    /// Current step of the 8-step frame-sequencer cycle (0–7).
    frame_seq_step: u8,
    /// Fractional T-cycle accumulator for sample generation.
    sample_timer: f32,
    /// Queued interleaved stereo samples (left, right, left, right, …).
    samples: Vec<f32>,
}

impl APU {
    pub(crate) fn new() -> Self {
        Self {
            ch1: Channel1::default(),
            ch2: Channel2::default(),
            ch3: Channel3::default(),
            ch4: Channel4::default(),
            nr50: 0,
            nr51: 0,
            on: false,
            frame_seq_step: 0,
            sample_timer: 0.0,
            samples: Vec::new(),
        }
    }

    /// Advance the APU by `cycles` T-cycles.
    ///
    /// `prev_counter` is the system counter value at the start of this step
    /// (read from the timer before it advances).  The APU uses it to detect
    /// falling edges on the frame-sequencer bit (bit 12) without owning the
    /// system counter directly.
    ///
    /// Channel frequency timers are clocked once per T-cycle.  The frame
    /// sequencer fires at 512 Hz via falling-edge detection on counter bit 12.
    /// Audio samples are pushed at roughly 44,100 Hz.
    pub(crate) fn step(&mut self, cycles: u16, prev_counter: u16) {
        if !self.on {
            return;
        }
        let mut counter = prev_counter;
        // Process one machine cycle (4 T-cycles) at a time so the frame
        // sequencer edge can be detected at M-cycle granularity.
        for _ in 0..cycles / 4 {
            let next_counter = counter.wrapping_add(4);

            // Frame sequencer: 512 Hz clock from system counter bit 12.
            // One tick fires on each falling edge (bit transitions 1 → 0).
            if counter & FRAME_SEQ_BIT != 0 && next_counter & FRAME_SEQ_BIT == 0 {
                self.clock_frame_sequencer();
            }

            counter = next_counter;

            // Clock channel timers and sample output four times (one per T-cycle).
            self.tick_t();
            self.tick_t();
            self.tick_t();
            self.tick_t();
        }
    }

    /// Called by the MMU when the game writes to 0xFF04 (DIV reset).
    ///
    /// Resetting the system counter to zero creates a falling edge on any
    /// counter bit that was previously 1.  If bit 12 was set, the frame
    /// sequencer fires immediately — exactly as it would from a natural
    /// falling edge during [`step`].
    pub(crate) fn notify_div_reset(&mut self, old_counter: u16) {
        if self.on && old_counter & FRAME_SEQ_BIT != 0 {
            self.clock_frame_sequencer();
        }
    }

    /// Return and clear all queued audio samples as interleaved stereo f32
    /// values in the range −1.0 to +1.0 (left, right, left, right, …).
    ///
    /// The CLI calls this once per frame and pushes the slice into the audio
    /// ring buffer.
    pub(crate) fn drain_samples(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.samples)
    }

    // -----------------------------------------------------------------------
    // Private: per-T-cycle tick
    // -----------------------------------------------------------------------

    /// Clock all channel frequency timers and advance sample output by one T-cycle.
    fn tick_t(&mut self) {
        self.ch1.tick_timer();
        self.ch2.tick_timer();
        self.ch3.tick_timer();
        self.ch4.tick_timer();

        // Sample output: accumulate until we've consumed one sample's worth.
        self.sample_timer += 1.0;
        if self.sample_timer >= SAMPLE_PERIOD_T {
            self.sample_timer -= SAMPLE_PERIOD_T;
            self.push_sample();
        }
    }

    // -----------------------------------------------------------------------
    // Private: frame sequencer
    // -----------------------------------------------------------------------

    fn clock_frame_sequencer(&mut self) {
        // Length counters clock at 256 Hz: steps 0, 2, 4, 6.
        if self.frame_seq_step & 1 == 0 {
            self.clock_length_counters();
        }
        // Frequency sweep clocks at 128 Hz: steps 2 and 6.
        if self.frame_seq_step == 2 || self.frame_seq_step == 6 {
            self.ch1.clock_sweep();
        }
        // Volume envelopes clock at 64 Hz: step 7 only.
        if self.frame_seq_step == 7 {
            self.clock_volume_envelopes();
        }
        self.frame_seq_step = (self.frame_seq_step + 1) & 7;
    }

    fn clock_length_counters(&mut self) {
        if self.ch1.length.clock() {
            self.ch1.enabled = false;
        }
        if self.ch2.length.clock() {
            self.ch2.enabled = false;
        }
        if self.ch3.length.clock() {
            self.ch3.enabled = false;
        }
        if self.ch4.length.clock() {
            self.ch4.enabled = false;
        }
    }

    fn clock_volume_envelopes(&mut self) {
        self.ch1.envelope.clock();
        self.ch2.envelope.clock();
        self.ch4.envelope.clock();
        // Ch3 has no volume envelope; its level is set by NR32.
    }

    // -----------------------------------------------------------------------
    // Private: sample generation
    // -----------------------------------------------------------------------

    fn push_sample(&mut self) {
        let ch1 = self.ch1_dac_output();
        let ch2 = self.ch2_dac_output();
        let ch3 = self.ch3_dac_output();
        let ch4 = self.ch4_dac_output();

        // NR50: master volume for left (bits 6–4) and right (bits 2–0).
        // Pan Docs: "0 equals volume of 1 and 7 equals no reduction."
        // The hardware adds 1 before scaling, so 0 → 1/8 (quiet) and 7 → 8/8 (full).
        // Dividing by 7 instead would make volume 0 silent, which is incorrect.
        let left_vol = (((self.nr50 >> 4) & 0x07) + 1) as f32 / 8.0;
        let right_vol = ((self.nr50 & 0x07) + 1) as f32 / 8.0;

        // NR51: which channels are panned to each output.
        // Bit 7/6/5/4 = Ch4/3/2/1 to left; bit 3/2/1/0 = Ch4/3/2/1 to right.
        let left = {
            let mut s = 0.0f32;
            if self.nr51 & 0x80 != 0 {
                s += ch4;
            }
            if self.nr51 & 0x40 != 0 {
                s += ch3;
            }
            if self.nr51 & 0x20 != 0 {
                s += ch2;
            }
            if self.nr51 & 0x10 != 0 {
                s += ch1;
            }
            s * left_vol / 4.0
        };
        let right = {
            let mut s = 0.0f32;
            if self.nr51 & 0x08 != 0 {
                s += ch4;
            }
            if self.nr51 & 0x04 != 0 {
                s += ch3;
            }
            if self.nr51 & 0x02 != 0 {
                s += ch2;
            }
            if self.nr51 & 0x01 != 0 {
                s += ch1;
            }
            s * right_vol / 4.0
        };

        self.samples.push(left);
        self.samples.push(right);
    }

    // DAC conversion: amplitude 0–15 → float −1.0 to +1.0.
    // When the channel or its DAC is off, the output is 0.0 (DC centre).
    fn ch1_dac_output(&self) -> f32 {
        if !self.ch1.enabled || !self.ch1.dac_on {
            return 0.0;
        }
        let high = (DUTY_PATTERNS[self.ch1.duty as usize] >> self.ch1.phase) & 1;
        dac(high * self.ch1.envelope.volume)
    }

    fn ch2_dac_output(&self) -> f32 {
        if !self.ch2.enabled || !self.ch2.dac_on {
            return 0.0;
        }
        let high = (DUTY_PATTERNS[self.ch2.duty as usize] >> self.ch2.phase) & 1;
        dac(high * self.ch2.envelope.volume)
    }

    fn ch3_dac_output(&self) -> f32 {
        if !self.ch3.enabled || !self.ch3.dac_on {
            return 0.0;
        }
        // output_level: 0 = mute (shift 4), 1 = 100% (shift 0), 2 = 50% (shift 1), 3 = 25% (shift 2)
        let shift = [4u8, 0, 1, 2][self.ch3.output_level as usize];
        dac(self.ch3.current_sample() >> shift)
    }

    fn ch4_dac_output(&self) -> f32 {
        if !self.ch4.enabled || !self.ch4.dac_on {
            return 0.0;
        }
        // LFSR bit 0 inverted: 0 → high (volume), 1 → low (silence)
        let high = (self.ch4.lfsr & 1) ^ 1;
        dac(high as u8 * self.ch4.envelope.volume)
    }

    // -----------------------------------------------------------------------
    // Private: helpers
    // -----------------------------------------------------------------------

    fn or_mask(address: u16) -> u8 {
        match address {
            0xFF10..=0xFF26 => OR_MASK[(address - 0xFF10) as usize],
            _ => 0xFF,
        }
    }

    /// Clear all channel and control registers.  Called when NR52 bit 7 → 0.
    ///
    /// On DMG, length counters survive APU power-off — only control, frequency,
    /// and envelope registers are cleared.  This allows games to pre-load length
    /// values before powering the APU on.
    fn power_off(&mut self) {
        let lengths = [
            self.ch1.length.value,
            self.ch2.length.value,
            self.ch3.length.value,
            self.ch4.length.value,
        ];
        self.ch1 = Channel1::default();
        self.ch2 = Channel2::default();
        self.ch3 = Channel3::default();
        self.ch4 = Channel4::default();
        self.ch1.length.value = lengths[0];
        self.ch2.length.value = lengths[1];
        self.ch3.length.value = lengths[2];
        self.ch4.length.value = lengths[3];
        self.nr50 = 0;
        self.nr51 = 0;
    }
}

/// Convert a 4-bit DAC amplitude (0–15) to a normalised float (−1.0 to +1.0).
///
/// The DMG DAC is a simple resistor ladder.  Amplitude 0 maps to the negative
/// rail and 15 to the positive rail.
#[inline]
fn dac(amplitude: u8) -> f32 {
    (amplitude as f32 / 7.5) - 1.0
}

impl Memory for APU {
    fn read_byte(&self, address: u16) -> u8 {
        match address {
            // Wave RAM is always accessible, regardless of APU power state.
            0xFF30..=0xFF3F => self.ch3.read(address),

            // NR52 is always readable: reports power state and per-channel enable flags.
            0xFF26 => {
                let ch_bits = (self.ch4.enabled as u8) << 3
                    | (self.ch3.enabled as u8) << 2
                    | (self.ch2.enabled as u8) << 1
                    | (self.ch1.enabled as u8);
                (self.on as u8) << 7 | ch_bits | 0x70 // 0x70 = OR mask for NR52
            }

            // All other registers read as 0xFF while the APU is off.
            _ if !self.on => 0xFF,

            // Channel and control registers — OR mask applied before returning.
            0xFF10..=0xFF14 => self.ch1.read(address) | Self::or_mask(address),
            0xFF15..=0xFF19 => self.ch2.read(address) | Self::or_mask(address),
            0xFF1A..=0xFF1E => self.ch3.read(address) | Self::or_mask(address),
            0xFF1F..=0xFF23 => self.ch4.read(address) | Self::or_mask(address),
            0xFF24 => self.nr50,
            0xFF25 => self.nr51,

            _ => 0xFF,
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        // Wave RAM is always writable (no trigger possible here; frame_seq_step unused).
        if matches!(address, 0xFF30..=0xFF3F) {
            self.ch3.write(address, value, 0);
            return;
        }

        // NR52 is always writable (controls the master power switch).
        if address == 0xFF26 {
            let was_on = self.on;
            self.on = value & 0x80 != 0;
            if was_on && !self.on {
                self.power_off();
            }
            return;
        }

        // All other APU registers are gated behind the master power switch.
        if !self.on {
            return;
        }

        match address {
            0xFF10..=0xFF14 => self.ch1.write(address, value, self.frame_seq_step),
            0xFF15..=0xFF19 => self.ch2.write(address, value, self.frame_seq_step),
            0xFF1A..=0xFF1E => {
                // Ch3 trigger on DMG: apply wave RAM corruption *before* triggering.
                // While Ch3 is active, the byte currently being read is accidentally
                // re-latched into the first bytes of wave RAM (DMG hardware only).
                if address == 0xFF1E && value & 0x80 != 0 {
                    self.ch3.apply_dmg_trigger_corruption();
                }
                self.ch3.write(address, value, self.frame_seq_step);
            }
            0xFF1F..=0xFF23 => self.ch4.write(address, value, self.frame_seq_step),
            0xFF24 => self.nr50 = value,
            0xFF25 => self.nr51 = value,
            _ => {}
        }
    }
}
