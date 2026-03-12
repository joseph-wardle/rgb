//! Channel 1 — pulse wave with frequency sweep.
//!
//! Ch1 generates a square wave with a programmable duty cycle.  An
//! additional frequency sweep unit can automatically slide the pitch up
//! or down at 128 Hz; the channel is silenced if the swept frequency
//! would overflow the 11-bit register (> 2047).
//!
//! ## Registers
//!
//! | Address | Name | Bits                                            |
//! |---------|------|-------------------------------------------------|
//! | FF10    | NR10 | `-PPP NSSS` sweep period / negate / shift       |
//! | FF11    | NR11 | `DDLL LLLL` duty / length load                  |
//! | FF12    | NR12 | `VVVV DPPP` env initial / dir / period          |
//! | FF13    | NR13 | `FFFF FFFF` freq bits 0–7 (write-only)          |
//! | FF14    | NR14 | `TL-- -FFF` trigger / length-enable / freq 8–10 |

use super::units::{EnvDir, LengthCounter, VolumeEnvelope};

/// Frequency sweep unit attached to Ch1.
///
/// Every `period` sweep-clocks (128 Hz), the internal frequency is shifted
/// right by `shift` bits and the delta is added to or subtracted from the
/// running frequency.  A `period` of 0 disables frequency updates (the timer
/// still runs at period 8).  A `shift` of 0 means no delta is applied.
#[derive(Debug, Default)]
pub struct FreqSweep {
    pub period: u8,   // NR10 bits 4–6 (0 = updates disabled)
    pub negate: bool, // NR10 bit 3: false = add (pitch up), true = subtract (pitch down)
    pub shift: u8,    // NR10 bits 0–2: right-shift amount applied to shadow freq
}

#[derive(Debug, Default)]
pub struct Channel1 {
    // — Register-backed state —
    pub sweep: FreqSweep,
    pub duty: u8, // NR11 bits 6–7: wave duty (0–3 → 12.5/25/50/75 %)
    pub envelope: VolumeEnvelope,
    pub length: LengthCounter,
    pub freq: u16,    // 11-bit: NR13 = bits 0–7, NR14 = bits 8–10
    pub dac_on: bool, // DAC powered when NR12 bits 3–7 are not all zero

    // — Internal audio state —
    pub enabled: bool,      // channel is producing output
    pub freq_timer: u16,    // T-cycle countdown; reloads with (2048 − freq) × 4
    pub phase: u8,          // wave position 0–7 in the current duty-cycle table
    pub sweep_freq: u16,    // shadow copy of freq used by sweep overflow checks
    pub sweep_timer: u8,    // countdown to the next sweep tick; 0 triggers a reload
    pub sweep_active: bool, // true while the sweep unit is tracking
}

impl Channel1 {
    /// Return the register value.  APU applies OR masks before handing to CPU.
    pub fn read(&self, address: u16) -> u8 {
        match address {
            0xFF10 => {
                (self.sweep.period << 4) | ((self.sweep.negate as u8) << 3) | self.sweep.shift
            }
            0xFF11 => self.duty << 6, // length bits 0–5 are write-only
            0xFF12 => {
                (self.envelope.initial << 4)
                    | ((self.envelope.dir as u8) << 3)
                    | self.envelope.period
            }
            0xFF13 => 0, // write-only (OR mask 0xFF applied by APU)
            0xFF14 => (self.length.enabled as u8) << 6,
            _ => 0,
        }
    }

    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            0xFF10 => {
                self.sweep.period = (value >> 4) & 0x07;
                self.sweep.negate = value & 0x08 != 0;
                self.sweep.shift = value & 0x07;
            }
            0xFF11 => {
                self.duty = (value >> 6) & 0x03;
                self.length.value = 64 - (value & 0x3F) as u16;
            }
            0xFF12 => {
                self.envelope.initial = (value >> 4) & 0x0F;
                self.envelope.dir = EnvDir::from_bit(value & 0x08 != 0);
                self.envelope.period = value & 0x07;
                self.dac_on = value & 0xF8 != 0;
                if !self.dac_on {
                    self.enabled = false;
                }
            }
            0xFF13 => {
                self.freq = (self.freq & 0x700) | value as u16;
            }
            0xFF14 => {
                self.freq = (self.freq & 0x0FF) | (((value & 0x07) as u16) << 8);
                self.length.enabled = value & 0x40 != 0;
                if value & 0x80 != 0 {
                    self.trigger();
                }
            }
            _ => {}
        }
    }

    fn trigger(&mut self) {
        self.enabled = self.dac_on;
        if self.length.value == 0 {
            self.length.value = 64;
        }
        self.freq_timer = (2048 - self.freq) * 4;
        self.envelope.volume = self.envelope.initial;
        self.envelope.timer = self.envelope.period;
        self.sweep_freq = self.freq;
        self.sweep_timer = if self.sweep.period != 0 {
            self.sweep.period
        } else {
            8
        };
        self.sweep_active = self.sweep.period != 0 || self.sweep.shift != 0;
        // Non-zero shift performs an immediate overflow check on trigger
        // (but does not update the frequency — that waits for the first sweep clock).
        if self.sweep.shift != 0 && self.calc_sweep_freq().is_none() {
            self.enabled = false;
        }
    }

    /// Clock the frequency timer by one T-cycle.  When it expires the wave
    /// phase advances and the timer reloads.
    pub fn tick_timer(&mut self) {
        if self.freq_timer > 0 {
            self.freq_timer -= 1;
        }
        if self.freq_timer == 0 {
            self.freq_timer = (2048 - self.freq) * 4;
            self.phase = (self.phase + 1) & 7;
        }
    }

    /// Clock the frequency sweep unit.  Called at 128 Hz (frame-sequencer
    /// steps 2 and 6).  Updates `sweep_freq` and `freq`, or disables the
    /// channel on 11-bit overflow.
    pub fn clock_sweep(&mut self) {
        if self.sweep_timer > 0 {
            self.sweep_timer -= 1;
        }
        if self.sweep_timer == 0 {
            self.sweep_timer = if self.sweep.period != 0 {
                self.sweep.period
            } else {
                8
            };
            if self.sweep_active && self.sweep.period != 0 {
                if let Some(new_freq) = self.calc_sweep_freq() {
                    if self.sweep.shift != 0 {
                        self.sweep_freq = new_freq;
                        self.freq = new_freq;
                    }
                    // Second overflow check after the update.
                    if self.calc_sweep_freq().is_none() {
                        self.enabled = false;
                    }
                } else {
                    self.enabled = false;
                }
            }
        }
    }

    /// Compute the next sweep frequency without writing it back.
    /// Returns `None` on 11-bit overflow (> 2047), which would silence the channel.
    pub fn calc_sweep_freq(&self) -> Option<u16> {
        let delta = self.sweep_freq >> self.sweep.shift;
        let next = if self.sweep.negate {
            self.sweep_freq.wrapping_sub(delta)
        } else {
            self.sweep_freq + delta
        };
        if next > 2047 { None } else { Some(next) }
    }
}
