//! DMG hardware timer.
//!
//! The timer consists of two independently-clocked counters:
//!
//! **DIV** — a free-running 16-bit internal counter whose upper byte is
//! mapped to 0xFF04.  It always runs regardless of TAC, and resets to
//! zero when any value is written to 0xFF04.
//!
//! **TIMA** — an 8-bit counter that increments at a rate selected by TAC
//! bits 0–1, but only when TAC bit 2 is set.  When TIMA overflows (0xFF
//! → 0x00) it is reloaded from TMA and a timer interrupt is requested.
//!
//! ## Known hardware edge case (not yet modelled)
//!
//! On real hardware the TIMA reload and interrupt request are delayed by
//! exactly one machine cycle after the overflow.  During that window TIMA
//! reads as 0x00, and writing TMA prevents the reload from taking effect.
//! This quirk affects a small number of games; it is noted here for future
//! reference.

/// Interrupt flag bit for the timer (IF bit 2).
const TIMER_INTERRUPT_BIT: u8 = 1 << 2;

/// Number of T-cycles per machine cycle on the DMG.
const T_CYCLES_PER_M_CYCLE: u16 = 4;

/// The DIV register (0xFF04) increments every 256 machine cycles
/// (1,024 T-cycles), giving a rate of 16,384 Hz.
const DIV_PERIOD_M_CYCLES: u16 = 256;

/// TIMA periods in machine cycles for each TAC clock-select value (bits 0–1).
///
/// | Select | Period  | Rate        |
/// |--------|---------|-------------|
/// | 00     | 256 M   | 4,096 Hz    |
/// | 01     | 4 M     | 262,144 Hz  |
/// | 10     | 16 M    | 65,536 Hz   |
/// | 11     | 64 M    | 16,384 Hz   |
const TIMA_PERIOD_M_CYCLES: [u16; 4] = [256, 4, 16, 64];

#[derive(Debug, Default)]
pub(crate) struct Timer {
    pub(crate) div: u8, // DIV  (FF04): upper byte of the internal counter; always running
    pub(crate) tima: u8, // TIMA (FF05): timer counter; increments at TAC-selected rate
    pub(crate) tma: u8, // TMA  (FF06): reload value written into TIMA on overflow
    pub(crate) tac: u8, // TAC  (FF07): bit 2 = timer enable; bits 0–1 = clock select

    div_phase: u16,  // T-cycle accumulator for the DIV counter
    tima_phase: u16, // T-cycle accumulator for the TIMA counter
}

impl Timer {
    pub(crate) fn new() -> Self {
        Timer {
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            div_phase: 0,
            tima_phase: 0,
        }
    }

    pub(crate) fn step(&mut self, cycles: u16, interrupt_flag: &mut u8) {
        // DIV is the upper byte of a free-running 16-bit counter driven by T-cycles.
        let div_threshold = DIV_PERIOD_M_CYCLES * T_CYCLES_PER_M_CYCLE;
        self.div_phase = self.div_phase.wrapping_add(cycles);
        while self.div_phase >= div_threshold {
            self.div_phase -= div_threshold;
            self.div = self.div.wrapping_add(1);
        }

        // TIMA increments at the rate selected by TAC bits 0–1, but only
        // when TAC bit 2 (timer enable) is set.
        if self.timer_enabled() {
            let threshold = self.tima_period_t_cycles();
            self.tima_phase = self.tima_phase.wrapping_add(cycles);
            while self.tima_phase >= threshold {
                self.tima_phase -= threshold;
                if self.tima == 0xFF {
                    // Overflow: reload from TMA and request a timer interrupt.
                    self.tima = self.tma;
                    *interrupt_flag |= TIMER_INTERRUPT_BIT;
                } else {
                    self.tima = self.tima.wrapping_add(1);
                }
            }
        }
    }

    /// Writing any value to 0xFF04 resets both the public DIV register and
    /// the hidden phase accumulator, so the next DIV increment starts fresh.
    pub(crate) fn reset_divider(&mut self) {
        self.div = 0;
        self.div_phase = 0;
    }

    #[inline]
    fn timer_enabled(&self) -> bool {
        self.tac & 0b100 != 0
    }

    #[inline]
    fn tima_period_t_cycles(&self) -> u16 {
        let select = (self.tac & 0b11) as usize;
        TIMA_PERIOD_M_CYCLES[select] * T_CYCLES_PER_M_CYCLE
    }
}
