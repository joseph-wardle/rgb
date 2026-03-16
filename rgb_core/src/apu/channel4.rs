//! Channel 4 — noise (LFSR).
//!
//! Ch4 generates pseudo-random noise by clocking a 15-bit linear-feedback
//! shift register (LFSR).  Writing 1 to NR43 bit 3 narrows it to 7 bits,
//! producing a coarser, more periodic buzzing tone.  The clock rate is
//! derived from a 3-bit base divisor and a 4-bit shift amount.
//!
//! ## Registers
//!
//! | Address | Name | Bits                                            |
//! |---------|------|-------------------------------------------------|
//! | FF20    | NR41 | `--LL LLLL` length load (write-only)            |
//! | FF21    | NR42 | `VVVV DPPP` env initial / dir / period          |
//! | FF22    | NR43 | `SSSS WDDD` clock shift / LFSR width / divisor  |
//! | FF23    | NR44 | `TL-- ----` trigger / length-enable             |

use super::units::{EnvDir, LengthCounter, VolumeEnvelope};

#[derive(Debug, Default)]
pub struct Channel4 {
    // — Register-backed state —
    pub envelope: VolumeEnvelope,
    pub length: LengthCounter,
    pub clock_shift: u8,   // NR43 bits 4–7: right-shift on base period
    pub lfsr_narrow: bool, // NR43 bit 3: false = 15-bit LFSR, true = 7-bit (periodic)
    pub clock_div: u8,     // NR43 bits 0–2: base divisor (0 = 0.5, else N)
    pub dac_on: bool,      // NR42 bits 3–7 != 0

    // — Internal audio state —
    pub enabled: bool,
    pub lfsr: u16,       // 15-bit (or 7-bit) shift register; bit 0 = current output
    pub freq_timer: u32, // T-cycle countdown to next LFSR clock
}

impl Channel4 {
    pub fn read(&self, address: u16) -> u8 {
        match address {
            0xFF20 => 0, // write-only (OR mask 0xFF applied by APU)
            0xFF21 => {
                (self.envelope.initial << 4)
                    | ((self.envelope.dir as u8) << 3)
                    | self.envelope.period
            }
            0xFF22 => (self.clock_shift << 4) | ((self.lfsr_narrow as u8) << 3) | self.clock_div,
            0xFF23 => (self.length.enabled as u8) << 6,
            _ => 0,
        }
    }

    pub fn write(&mut self, address: u16, value: u8, frame_seq_step: u8) {
        match address {
            0xFF20 => {
                self.length.value = 64 - (value & 0x3F) as u16;
            }
            0xFF21 => {
                self.envelope.initial = (value >> 4) & 0x0F;
                self.envelope.dir = EnvDir::from_bit(value & 0x08 != 0);
                self.envelope.period = value & 0x07;
                self.dac_on = value & 0xF8 != 0;
                if !self.dac_on {
                    self.enabled = false;
                }
            }
            0xFF22 => {
                self.clock_shift = (value >> 4) & 0x0F;
                self.lfsr_narrow = value & 0x08 != 0;
                self.clock_div = value & 0x07;
            }
            0xFF23 => {
                self.length.enabled = value & 0x40 != 0;
                if value & 0x80 != 0 {
                    self.trigger(frame_seq_step);
                }
            }
            _ => {}
        }
    }

    /// Clock the frequency timer by one T-cycle.  When it expires, clock
    /// the LFSR and reload the timer.
    pub fn tick_timer(&mut self) {
        if self.freq_timer > 0 {
            self.freq_timer -= 1;
        }
        if self.freq_timer == 0 {
            self.freq_timer = self.period_t_cycles();
            self.clock_lfsr();
        }
    }

    /// Advance the LFSR by one step.
    ///
    /// The feedback bit is XOR of bits 0 and 1.  It is shifted into bit 14
    /// (and also bit 6 in 7-bit narrow mode), then the register shifts right.
    fn clock_lfsr(&mut self) {
        let feedback = (self.lfsr ^ (self.lfsr >> 1)) & 1;
        self.lfsr = (self.lfsr >> 1) | (feedback << 14);
        if self.lfsr_narrow {
            // Replace bit 6 with the same feedback bit for a 7-bit period.
            self.lfsr = (self.lfsr & !(1 << 6)) | (feedback << 6);
        }
    }

    fn trigger(&mut self, frame_seq_step: u8) {
        self.enabled = self.dac_on;
        if self.length.value == 0 {
            self.length.value = 64;
        }
        self.envelope.volume = self.envelope.initial;
        self.envelope.timer = self.envelope.period;
        self.lfsr = 0x7FFF; // all bits set
        self.freq_timer = self.period_t_cycles();

        // Extra length clock when the frame sequencer's next step won't clock
        // length (next step is odd: 1, 3, 5, 7 — `frame_seq_step` already
        // points at the next step because it was incremented after the last tick).
        if self.length.enabled && frame_seq_step & 1 == 1 && self.length.clock() {
            self.enabled = false;
        }
    }

    /// T-cycle period between LFSR clocks.
    ///
    /// From Pan Docs: `(divisor == 0 ? 8 : divisor × 16) << clock_shift`.
    pub fn period_t_cycles(&self) -> u32 {
        let base = if self.clock_div == 0 {
            8
        } else {
            self.clock_div as u32 * 16
        };
        base << self.clock_shift
    }
}
