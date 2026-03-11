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
    /// Bit 7 / bit 3 route the cartridge's analogue VIN pin; not emulated.
    pub nr50: u8,

    /// NR51 — stereo panning.
    /// Bit (N+4) = Ch(N+1) to left output; bit N = Ch(N+1) to right output.
    pub nr51: u8,

    /// NR52 bit 7 — master power switch.
    /// When false all APU registers are cleared and reads return 0xFF.
    pub on: bool,
}

impl APU {
    pub(crate) fn new() -> Self {
        Self {
            ch1:  Channel1::default(),
            ch2:  Channel2::default(),
            ch3:  Channel3::default(),
            ch4:  Channel4::default(),
            nr50: 0,
            nr51: 0,
            on:   false,
        }
    }

    /// Advance the APU by `cycles` machine cycles (1 machine cycle = 4 T-cycles).
    ///
    /// Frame-sequencer clocking and sample generation are added in the next
    /// phase; this stub wires the APU into the master clock without producing
    /// any output.
    pub(crate) fn step(&mut self, _cycles: u16) {}

    /// Return and clear all queued audio samples as interleaved stereo f32
    /// values in the range −1.0 to +1.0 (left, right, left, right, …).
    ///
    /// The CLI calls this once per frame and pushes the slice into the audio
    /// ring buffer.  This stub always returns an empty vec until sample
    /// generation is implemented in the next phase.
    pub(crate) fn drain_samples(&mut self) -> Vec<f32> {
        Vec::new()
    }

    fn or_mask(address: u16) -> u8 {
        match address {
            0xFF10..=0xFF26 => OR_MASK[(address - 0xFF10) as usize],
            _ => 0xFF,
        }
    }

    /// Clear all channel and control registers.  Called when NR52 bit 7 → 0.
    fn power_off(&mut self) {
        self.ch1  = Channel1::default();
        self.ch2  = Channel2::default();
        self.ch3  = Channel3::default();
        self.ch4  = Channel4::default();
        self.nr50 = 0;
        self.nr51 = 0;
    }
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
                (self.on as u8) << 7 | ch_bits | 0x70  // 0x70 = OR mask for NR52
            }

            // All other registers read as 0xFF while the APU is off.
            _ if !self.on => 0xFF,

            // Channel and control registers — OR mask applied before returning.
            0xFF10..=0xFF14 => self.ch1.read(address) | Self::or_mask(address),
            0xFF15..=0xFF19 => self.ch2.read(address) | Self::or_mask(address),
            0xFF1A..=0xFF1E => self.ch3.read(address) | Self::or_mask(address),
            0xFF1F..=0xFF23 => self.ch4.read(address) | Self::or_mask(address),
            0xFF24           => self.nr50,
            0xFF25           => self.nr51,

            _ => 0xFF,
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        // Wave RAM is always writable.
        if matches!(address, 0xFF30..=0xFF3F) {
            self.ch3.write(address, value);
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
            0xFF10..=0xFF14 => self.ch1.write(address, value),
            0xFF15..=0xFF19 => self.ch2.write(address, value),
            0xFF1A..=0xFF1E => self.ch3.write(address, value),
            0xFF1F..=0xFF23 => self.ch4.write(address, value),
            0xFF24           => self.nr50 = value,
            0xFF25           => self.nr51 = value,
            _ => {}
        }
    }
}
