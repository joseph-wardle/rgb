//! DMG hardware timer.
//!
//! ## Hardware overview
//!
//! All timer behaviour stems from a single free-running 16-bit **system
//! counter** that increments every T-cycle.  Three registers are built on top:
//!
//! **DIV** (0xFF04) — the upper byte of the system counter, always running.
//! Writing any value to 0xFF04 resets the counter to zero.
//!
//! **TIMA** (0xFF05) — an 8-bit counter clocked by a falling edge on a
//! specific bit of the system counter (selected by TAC bits 0–1), gated by
//! TAC bit 2 (timer enable).
//!
//! | TAC bits 0–1 | Counter bit | Period      | Rate         |
//! |--------------|-------------|-------------|--------------|
//! | 00           | bit 9       | 1,024 T     |    4,096 Hz  |
//! | 01           | bit 3       |    16 T     |  262,144 Hz  |
//! | 10           | bit 5       |    64 T     |   65,536 Hz  |
//! | 11           | bit 7       |   256 T     |   16,384 Hz  |
//!
//! **TMA** (0xFF06) — the reload value: when TIMA overflows (0xFF → 0x00) it
//! is reloaded from TMA one M-cycle later and a timer interrupt is requested.
//!
//! ## The TIMA overflow window
//!
//! When TIMA overflows the hardware does **not** immediately reload TIMA or
//! raise the interrupt.  There is a one-machine-cycle gap — the "reload
//! window" — before the reload commits:
//!
//! | Cycle | TIMA value | IF bit 2    |
//! |-------|------------|-------------|
//! | A — overflow      | 0x00 | not yet set |
//! | B — reload (+1 M) | TMA  | now set     |
//!
//! During cycle A the CPU can cancel both the reload and the interrupt by
//! writing any value to TIMA — that written value stays and no interrupt
//! fires.  If TMA is updated during cycle A the pending reload picks up the
//! new value.
//!
//! ## DIV-write falling edges
//!
//! Resetting the system counter (writing to 0xFF04) can create a falling edge
//! on the TIMA bit tap if that bit was 1 at the moment of the write.  This
//! gives TIMA an extra increment exactly as if the counter had counted through
//! naturally.  The APU frame sequencer (bit 12) is subject to the same effect;
//! see [`reset_divider`].

/// IF bit for the timer interrupt.
const TIMER_INTERRUPT_BIT: u8 = 1 << 2;

/// T-cycles in one machine cycle on the DMG.
const T_CYCLES_PER_M_CYCLE: u16 = 4;

/// System counter bit tapped for each TAC clock-select value (bits 0–1).
///
/// TIMA increments on the **falling edge** of the selected bit in the 16-bit
/// system counter.  A falling edge on bit N occurs every 2^(N+1) T-cycles.
///
/// | TAC | Bit | Period      | Rate         |
/// |-----|-----|-------------|--------------|
/// | 00  |  9  | 1,024 T     |    4,096 Hz  |
/// | 01  |  3  |    16 T     |  262,144 Hz  |
/// | 10  |  5  |    64 T     |   65,536 Hz  |
/// | 11  |  7  |   256 T     |   16,384 Hz  |
const TIMA_BIT: [u16; 4] = [1 << 9, 1 << 3, 1 << 5, 1 << 7];

#[derive(Debug, Default)]
pub(crate) struct Timer {
    pub(crate) tima: u8, // TIMA (FF05): timer counter; increments at TAC-selected rate
    pub(crate) tma: u8,  // TMA  (FF06): reload value copied into TIMA on overflow
    pub(crate) tac: u8,  // TAC  (FF07): bit 2 = timer enable; bits 0–1 = clock select

    /// Free-running 16-bit system counter; increments every T-cycle.
    ///
    /// `DIV` (0xFF04) is the upper byte: `(system_counter >> 8) as u8`.
    /// The APU frame sequencer is also derived from this counter (bit 12).
    system_counter: u16,

    /// Set during the one-M-cycle reload window after a TIMA overflow.
    ///
    /// While this is true we are in cycle A: TIMA holds 0x00 and the
    /// interrupt has not yet fired.  On the very next M-cycle (cycle B) the
    /// flag is cleared, TIMA is loaded from TMA, and the interrupt fires.
    ///
    /// Writing to TIMA while this flag is set (see [`write_tima`]) cancels
    /// the reload entirely.
    tima_reload_pending: bool,
}

impl Timer {
    pub(crate) fn new() -> Self {
        Timer::default()
    }

    /// DIV register (0xFF04): the upper byte of the system counter.
    pub(crate) fn div(&self) -> u8 {
        (self.system_counter >> 8) as u8
    }

    /// The raw 16-bit system counter.
    ///
    /// The MMU reads this before each [`step`] call and forwards it to the APU
    /// so the APU can detect falling edges on the frame-sequencer bit (bit 12)
    /// without owning the counter directly.
    pub(crate) fn system_counter(&self) -> u16 {
        self.system_counter
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

    /// Writing any value to 0xFF04 resets the system counter to zero.
    ///
    /// Returns the old counter value so the MMU can forward it to the APU for
    /// frame-sequencer edge detection (see [`crate::apu::APU::notify_div_reset`]).
    ///
    /// Also checks for a TIMA falling edge: if the selected tap bit was 1
    /// before the reset, the counter dropping to 0 is a falling edge and TIMA
    /// gets an extra increment.  The deferred interrupt (if TIMA overflowed)
    /// fires on the next [`step_m_cycle`] via the `tima_reload_pending` flag.
    pub(crate) fn reset_divider(&mut self) -> u16 {
        let old = self.system_counter;
        self.system_counter = 0;

        // Resetting to zero creates a falling edge on the TIMA bit tap
        // if that bit was set — give TIMA its extra increment.
        if self.timer_enabled() && old & self.tima_bit() != 0 {
            self.advance_tima();
        }

        old
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

        // Advance the system counter by 4 T-cycles (one machine cycle).
        let old = self.system_counter;
        self.system_counter = self.system_counter.wrapping_add(T_CYCLES_PER_M_CYCLE);

        // TIMA: increment on the falling edge of the selected counter bit,
        // but only when the timer is enabled (TAC bit 2).
        if self.timer_enabled() {
            let bit = self.tima_bit();
            if old & bit != 0 && self.system_counter & bit == 0 {
                self.advance_tima();
            }
        }
    }

    /// Increment TIMA, entering the reload window on overflow (cycle A).
    ///
    /// The deferred interrupt and TMA reload commit on the next M-cycle
    /// when [`step_m_cycle`] processes the `tima_reload_pending` flag.
    fn advance_tima(&mut self) {
        if self.tima == 0xFF {
            // Cycle A: TIMA wraps to 0x00.  Defer reload and interrupt.
            self.tima = 0x00;
            self.tima_reload_pending = true;
        } else {
            self.tima = self.tima.wrapping_add(1);
        }
    }

    #[inline]
    fn timer_enabled(&self) -> bool {
        self.tac & 0b100 != 0
    }

    /// The system counter bit tapped to clock TIMA (set by TAC bits 0–1).
    #[inline]
    fn tima_bit(&self) -> u16 {
        TIMA_BIT[(self.tac & 0b11) as usize]
    }
}
