//! Channel 3 — arbitrary wave (PCM).
//!
//! Ch3 reads 4-bit samples from a 16-byte (32-nibble) wave RAM at
//! 0xFF30–0xFF3F and outputs them at a rate controlled by the frequency
//! register.  The DAC is controlled by a dedicated bit (NR30 bit 7) instead
//! of an envelope register; output volume is set by a 2-bit shift field.
//!
//! ## Registers
//!
//! | Address   | Name | Bits                                           |
//! |-----------|------|------------------------------------------------|
//! | FF1A      | NR30 | `D--- ----` DAC on/off                         |
//! | FF1B      | NR31 | `LLLL LLLL` length load (write-only)           |
//! | FF1C      | NR32 | `-LL- ----` output level (00/01/10/11 → mute/100%/50%/25%) |
//! | FF1D      | NR33 | `FFFF FFFF` freq bits 0–7 (write-only)         |
//! | FF1E      | NR34 | `TL-- -FFF` trigger / length-enable / freq 8–10|
//! | FF30–FF3F | —    | Wave RAM: 32 × 4-bit samples, packed two per byte |

use super::units::LengthCounter;

#[derive(Debug, Default)]
pub struct Channel3 {
    // — Register-backed state —
    pub dac_on: bool,     // NR30 bit 7: powers the DAC
    pub output_level: u8, // NR32 bits 5–6: 0=mute, 1=100%, 2=50%, 3=25%
    pub length: LengthCounter,
    pub freq: u16,          // 11-bit: NR33 = bits 0–7, NR34 = bits 8–10
    pub wave_ram: [u8; 16], // 0xFF30–0xFF3F: 32 packed 4-bit samples

    // — Internal audio state —
    pub enabled: bool,
    pub freq_timer: u16, // T-cycle countdown; reloads with (2048 − freq) × 2
    pub phase: u8,       // nibble index 0–31 into wave_ram
}

impl Channel3 {
    pub fn read(&self, address: u16) -> u8 {
        match address {
            0xFF1A => (self.dac_on as u8) << 7, // bits 0–6 unused (OR mask 0x7F by APU)
            0xFF1B => 0,                        // write-only (OR mask 0xFF by APU)
            0xFF1C => self.output_level << 5,   // bits 0–4 and 7 unused (OR mask 0x9F)
            0xFF1D => 0,                        // write-only (OR mask 0xFF by APU)
            0xFF1E => (self.length.enabled as u8) << 6,
            0xFF30..=0xFF3F => self.wave_ram[(address - 0xFF30) as usize],
            _ => 0,
        }
    }

    pub fn write(&mut self, address: u16, value: u8, frame_seq_step: u8) {
        match address {
            0xFF1A => {
                self.dac_on = value & 0x80 != 0;
                if !self.dac_on {
                    self.enabled = false;
                }
            }
            0xFF1B => {
                self.length.value = 256 - value as u16;
            }
            0xFF1C => {
                self.output_level = (value >> 5) & 0x03;
            }
            0xFF1D => {
                self.freq = (self.freq & 0x700) | value as u16;
            }
            0xFF1E => {
                self.freq = (self.freq & 0x0FF) | (((value & 0x07) as u16) << 8);
                self.length.enabled = value & 0x40 != 0;
                if value & 0x80 != 0 {
                    self.trigger(frame_seq_step);
                }
            }
            0xFF30..=0xFF3F => {
                self.wave_ram[(address - 0xFF30) as usize] = value;
            }
            _ => {}
        }
    }

    /// Apply DMG-specific wave RAM corruption that occurs when Ch3 is triggered
    /// while it is actively reading samples.  CGB does not exhibit this behaviour.
    ///
    /// On DMG, if Ch3 is currently enabled at the moment of a trigger write, the
    /// byte being read from wave RAM is "accidentally" re-latched, overwriting the
    /// first bytes with data from the current wave position:
    ///
    /// - If the wave position is in the **first 4 bytes** (nibble positions 0–7),
    ///   only the byte currently being read is copied to position 0.
    /// - Otherwise, all 4 bytes of the **aligned 4-byte block** containing the
    ///   current position are copied to bytes 0–3.
    ///
    /// The APU must call this *before* routing the trigger write to `write()`.
    pub fn apply_dmg_trigger_corruption(&mut self) {
        if !self.enabled {
            return;
        }
        let byte_pos = (self.phase / 2) as usize;
        if byte_pos < 4 {
            // The single byte being read is re-latched into position 0.
            self.wave_ram[0] = self.wave_ram[byte_pos];
        } else {
            // The entire aligned 4-byte block containing the current position
            // is copied over bytes 0–3.
            let block_start = byte_pos & !3;
            self.wave_ram.copy_within(block_start..block_start + 4, 0);
        }
    }

    /// Clock the frequency timer by one T-cycle.  Ch3 fires twice as often
    /// as Ch1/Ch2 for the same frequency value, stepping through 32 nibbles.
    pub fn tick_timer(&mut self) {
        if self.freq_timer > 0 {
            self.freq_timer -= 1;
        }
        if self.freq_timer == 0 {
            self.freq_timer = (2048 - self.freq) * 2;
            self.phase = (self.phase + 1) & 31;
        }
    }

    /// The current 4-bit PCM sample at the wave phase position.
    /// The wave RAM holds 32 nibbles packed two-per-byte: high nibble first.
    pub fn current_sample(&self) -> u8 {
        let byte = self.wave_ram[(self.phase / 2) as usize];
        if self.phase & 1 == 0 {
            byte >> 4
        } else {
            byte & 0x0F
        }
    }

    fn trigger(&mut self, frame_seq_step: u8) {
        self.enabled = self.dac_on;
        if self.length.value == 0 {
            self.length.value = 256;
        }
        // Note: phase is intentionally NOT reset here.  On real hardware, triggering
        // Ch3 does not reset the wave position; the channel resumes from wherever it
        // left off.  Only the frequency timer is reloaded.  The +3 T-cycle startup
        // delay is a hardware quirk: Ch3 takes 3 extra T-cycles before it begins
        // reading samples after a trigger.
        self.freq_timer = (2048 - self.freq) * 2 + 3;

        // Extra length clock when the frame sequencer's next step won't clock
        // length (next step is odd: 1, 3, 5, 7 — `frame_seq_step` already
        // points at the next step because it was incremented after the last tick).
        if self.length.enabled && frame_seq_step & 1 == 1 && self.length.clock() {
            self.enabled = false;
        }
    }
}
