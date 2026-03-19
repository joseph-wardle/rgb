//! Channel 2 — pulse wave (no sweep).
//!
//! Identical to Ch1 except there is no frequency sweep unit.  The register
//! at 0xFF15 is unmapped (reads 0xFF, writes ignored).
//!
//! ## Registers
//!
//! | Address | Name | Bits                                            |
//! |---------|------|-------------------------------------------------|
//! | FF15    | —    | unmapped                                        |
//! | FF16    | NR21 | `DDLL LLLL` duty / length load                  |
//! | FF17    | NR22 | `VVVV DPPP` env initial / dir / period          |
//! | FF18    | NR23 | `FFFF FFFF` freq bits 0–7 (write-only)          |
//! | FF19    | NR24 | `TL-- -FFF` trigger / length-enable / freq 8–10 |

use super::units::{EnvDir, LengthCounter, VolumeEnvelope};

#[derive(Debug, Default)]
pub struct Channel2 {
    // — Register-backed state —
    pub duty: u8, // NR21 bits 6–7: wave duty (0–3 → 12.5/25/50/75 %)
    pub envelope: VolumeEnvelope,
    pub length: LengthCounter,
    pub freq: u16,    // 11-bit: NR23 = bits 0–7, NR24 = bits 8–10
    pub dac_on: bool, // DAC powered when NR22 bits 3–7 are not all zero

    // — Internal audio state —
    pub enabled: bool,
    pub freq_timer: u16, // T-cycle countdown; reloads with (2048 − freq) × 4
    pub phase: u8,       // wave position 0–7 in the current duty-cycle table
}

impl Channel2 {
    pub fn read(&self, address: u16) -> u8 {
        match address {
            0xFF15 => 0,              // unmapped (OR mask 0xFF applied by APU)
            0xFF16 => self.duty << 6, // length bits 0–5 are write-only
            0xFF17 => {
                (self.envelope.initial << 4)
                    | ((self.envelope.dir as u8) << 3)
                    | self.envelope.period
            }
            0xFF18 => 0, // write-only (OR mask 0xFF applied by APU)
            0xFF19 => (self.length.enabled as u8) << 6,
            _ => 0,
        }
    }

    pub fn write(&mut self, address: u16, value: u8, frame_seq_step: u8) {
        match address {
            0xFF15 => {} // unmapped
            0xFF16 => {
                self.duty = (value >> 6) & 0x03;
                self.length.value = 64 - (value & 0x3F) as u16;
            }
            0xFF17 => {
                self.envelope.initial = (value >> 4) & 0x0F;
                self.envelope.dir = EnvDir::from_bit(value & 0x08 != 0);
                self.envelope.period = value & 0x07;
                self.dac_on = value & 0xF8 != 0;
                if !self.dac_on {
                    self.enabled = false;
                }
            }
            0xFF18 => {
                self.freq = (self.freq & 0x700) | value as u16;
            }
            0xFF19 => {
                self.freq = (self.freq & 0x0FF) | (((value & 0x07) as u16) << 8);
                self.length.enabled = value & 0x40 != 0;
                if value & 0x80 != 0 {
                    self.trigger(frame_seq_step);
                }
            }
            _ => {}
        }
    }

    /// Clock the frequency timer by one T-cycle.
    pub fn tick_timer(&mut self) {
        if self.freq_timer > 0 {
            self.freq_timer -= 1;
        }
        if self.freq_timer == 0 {
            self.freq_timer = (2048 - self.freq) * 4;
            self.phase = (self.phase + 1) & 7;
        }
    }

    fn trigger(&mut self, frame_seq_step: u8) {
        self.enabled = self.dac_on;
        if self.length.value == 0 {
            self.length.value = 64;
        }
        self.freq_timer = (2048 - self.freq) * 4;
        self.envelope.trigger();

        // Extra length clock when the frame sequencer's next step won't clock
        // length (next step is odd: 1, 3, 5, 7 — `frame_seq_step` already
        // points at the next step because it was incremented after the last tick).
        if self.length.enabled && frame_seq_step & 1 == 1 && self.length.clock() {
            self.enabled = false;
        }
    }
}
