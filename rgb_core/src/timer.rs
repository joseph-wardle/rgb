//! DMG hardware timer.
//!
//! ## Hardware overview
//!
//! All timer behaviour stems from a single free-running 16-bit internal
//! counter that increments every T-cycle.  Three registers are built on top:
//!
//! **DIV** (0xFF04) — the upper byte of the internal counter, always running.
//! Writing any value to 0xFF04 resets the counter to zero.
//!
//! **TIMA** (0xFF05) — an 8-bit counter driven by a configurable tap on the
//! internal counter (selected by TAC bits 0–1), gated by TAC bit 2.
//!
//! **TMA** (0xFF06) — the reload value: when TIMA overflows (0xFF → 0x00) it
//! is reloaded from TMA and a timer interrupt is requested.
//!
//! ## The TIMA overflow window
//!
//! When TIMA overflows the hardware does **not** immediately reload TIMA or
//! raise the interrupt.  There is a one-machine-cycle gap — the "reload
//! window" — before the reload commits:
//!
//! | Cycle | TIMA value | IF bit 2 |
//! |-------|------------|----------|
//! | A — overflow   | 0x00 | not yet set |
//! | B — reload (+1 M-cycle) | TMA  | now set |
//!
//! During cycle A the CPU can cancel both the reload and the interrupt by
//! writing any value to TIMA — that written value stays and no interrupt
//! fires.  If TMA is updated during cycle A the pending reload picks up the
//! new value.

/// IF bit for the timer interrupt.
const TIMER_INTERRUPT_BIT: u8 = 1 << 2;

/// T-cycles in one machine cycle on the DMG.
const T_CYCLES_PER_M_CYCLE: u16 = 4;

/// Machine cycles between DIV register increments.
///
/// DIV is the upper byte of a free-running 16-bit counter.  The full counter
/// increments every M-cycle; DIV (the upper byte) therefore increments every
/// 256 M-cycles (1,024 T-cycles), giving a rate of 4,096 Hz.
///
/// Note: real hardware actually clocks the internal counter every *T-cycle*,
/// making DIV increment at 16,384 Hz.  This phase-2 discrepancy is documented
/// and will be corrected when the timer is restructured around a proper 16-bit
/// system counter.
const DIV_PERIOD_M_CYCLES: u16 = 256;

/// TIMA periods in machine cycles for each TAC clock-select value (bits 0–1).
///
/// | Bits | Period | Rate       |
/// |------|--------|------------|
/// | 00   | 256 M  | 4,096 Hz   |
/// | 01   |   4 M  | 262,144 Hz |
/// | 10   |  16 M  | 65,536 Hz  |
/// | 11   |  64 M  | 16,384 Hz  |
const TIMA_PERIOD_M_CYCLES: [u16; 4] = [256, 4, 16, 64];

#[derive(Debug, Default)]
pub(crate) struct Timer {
    pub(crate) div: u8,  // DIV  (FF04): upper byte of the internal counter; always running
    pub(crate) tima: u8, // TIMA (FF05): timer counter; increments at TAC-selected rate
    pub(crate) tma: u8,  // TMA  (FF06): reload value copied into TIMA on overflow
    pub(crate) tac: u8,  // TAC  (FF07): bit 2 = timer enable; bits 0–1 = clock select

    div_phase: u16,  // T-cycle accumulator for the DIV counter
    tima_phase: u16, // T-cycle accumulator for the TIMA counter

    /// Set during the one-M-cycle reload window after a TIMA overflow.
    ///
    /// While this is true we are in cycle A: TIMA holds 0x00 and the
    /// interrupt has not yet fired.  On the very next M-cycle (cycle B) the
    /// flag is cleared, TIMA is loaded from TMA, and the interrupt fires.
    ///
    /// Writing to TIMA while this flag is set (see `write_tima`) cancels the
    /// reload entirely.
    tima_reload_pending: bool,
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
            tima_reload_pending: false,
        }
    }

    /// Advance the timer by `cycles` T-cycles.
    ///
    /// Processes one machine cycle at a time so that the TIMA reload window
    /// is honoured at M-cycle granularity.  `cycles` is always a multiple of
    /// 4 in practice because the DMG CPU executes whole machine cycles.
    pub(crate) fn step(&mut self, cycles: u16, interrupt_flag: &mut u8) {
        for _ in 0..cycles / T_CYCLES_PER_M_CYCLE {
            self.step_m_cycle(interrupt_flag);
        }
    }

    /// Write to TIMA (0xFF05).
    ///
    /// If a reload is pending (cycle A of the overflow window), writing to
    /// TIMA cancels the reload and suppresses the timer interrupt — the
    /// written value stays in TIMA.  Outside the window this is a plain write.
    pub(crate) fn write_tima(&mut self, value: u8) {
        self.tima_reload_pending = false;
        self.tima = value;
    }

    /// Write to TMA (0xFF06).
    ///
    /// If a reload is pending, the new TMA value will be used when the reload
    /// commits on the next M-cycle.  No special handling is needed: `step_m_cycle`
    /// reads `self.tma` at commit time, so updating it here is sufficient.
    pub(crate) fn write_tma(&mut self, value: u8) {
        self.tma = value;
    }

    /// Writing any value to 0xFF04 resets the internal counter to zero.
    pub(crate) fn reset_divider(&mut self) {
        self.div = 0;
        self.div_phase = 0;
    }

    // -----------------------------------------------------------------------
    // Private
    // -----------------------------------------------------------------------

    /// Advance the timer by one machine cycle (4 T-cycles).
    fn step_m_cycle(&mut self, interrupt_flag: &mut u8) {
        // Cycle B: commit a TIMA overflow that was deferred last M-cycle.
        // TIMA is loaded from TMA and the timer interrupt fires.
        if self.tima_reload_pending {
            self.tima_reload_pending = false;
            self.tima = self.tma;
            *interrupt_flag |= TIMER_INTERRUPT_BIT;
        }

        // DIV: advance the free-running phase accumulator.
        self.div_phase = self.div_phase.wrapping_add(T_CYCLES_PER_M_CYCLE);
        if self.div_phase >= DIV_PERIOD_M_CYCLES * T_CYCLES_PER_M_CYCLE {
            self.div_phase -= DIV_PERIOD_M_CYCLES * T_CYCLES_PER_M_CYCLE;
            self.div = self.div.wrapping_add(1);
        }

        // TIMA: advance only when the timer is enabled (TAC bit 2).
        if self.timer_enabled() {
            self.tima_phase = self.tima_phase.wrapping_add(T_CYCLES_PER_M_CYCLE);
            if self.tima_phase >= self.tima_period_t_cycles() {
                self.tima_phase -= self.tima_period_t_cycles();
                if self.tima == 0xFF {
                    // Cycle A: TIMA wraps to 0x00.  Defer the reload and the
                    // interrupt to the next M-cycle (see tima_reload_pending).
                    self.tima = 0x00;
                    self.tima_reload_pending = true;
                } else {
                    self.tima = self.tima.wrapping_add(1);
                }
            }
        }
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
