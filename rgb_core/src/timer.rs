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
//! raise the interrupt.  There is a **two-machine-cycle** gap before the
//! reload commits:
//!
//! | M-cycle        | TIMA  | IF bit 2  | CPU write effect               |
//! |----------------|-------|-----------|--------------------------------|
//! | overflow tick  | 0x00  | not set   | write before tick: no overflow |
//! | cycle A (+1 M) | 0x00  | not set   | write cancels reload entirely  |
//! | cycle B (+2 M) | TMA   | now set   | write is overridden by TMA     |
//!
//! During cycle A (the M-cycle *after* the overflow tick) the CPU can cancel
//! both the reload and the interrupt by writing any value to TIMA.
//!
//! During cycle B (one M-cycle after cycle A) the TMA reload commits at the
//! **start** of the tick — after any CPU write to TIMA in that M-cycle — so
//! the reload overrides the CPU write.  If TMA is updated during cycle A, the
//! pending reload picks up the new value.
//!
//! ## Falling edges from register writes
//!
//! Both DIV writes and TAC writes can create a falling edge on the mux signal
//! that clocks TIMA — `timer_enabled AND (system_counter & selected_bit != 0)`:
//!
//! - **DIV write** — resets the system counter to zero, so any previously-set
//!   tap bit drops to 0 while the enable may still be 1.  See [`reset_divider`].
//! - **TAC write** — changes the enable flag or the selected bit, so the mux
//!   output can fall from 1 to 0 even though the counter has not moved.
//!   See [`write_tac`].
//!
//! The APU frame sequencer (bit 12) is subject to the same DIV-write effect;
//! see [`crate::apu::APU::notify_div_reset`].

/// IF bit for the timer interrupt.
const TIMER_INTERRUPT_BIT: u8 = 1 << 2;

/// System counter value at the moment the boot ROM hands control to the
/// cartridge (PC = 0x0100 on real hardware).
///
/// The boot ROM executes for a fixed number of T-cycles before writing 0x01
/// to 0xFF50 and jumping to the cartridge entry point at PC = 0x0100.
/// Starting the counter here ensures DIV, TIMA, and the APU frame sequencer
/// all have the correct phase from the very first instruction the game executes.
///
/// The value 0xABD0 was determined empirically: it gives DIV = 0xAB (matching
/// documented DMG boot state) and places the sub-DIV phase so that Blargg's
/// `instr_timing` test — which calibrates a cycle-accurate timer and checks
/// the exact M-cycle at which TIMA overflows — passes its init_timer sanity
/// check.  The phase must be correct to within a single M-cycle (4 T-cycles)
/// for that test to succeed.
///
/// When the emulator boots *with* a boot ROM image the counter starts at 0
/// (power-on state) and reaches this value naturally as the boot ROM runs.
const SYSTEM_COUNTER_POST_BOOT: u16 = 0xABD0;

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

    /// Set on the M-cycle whose tick causes TIMA to overflow (0xFF → 0x00).
    ///
    /// This is the first stage of the two-cycle reload sequence.  Cleared by
    /// the next [`step_m_cycle`] call, which promotes it to
    /// `tima_reload_pending`.  Writing to TIMA while this flag is set (see
    /// [`write_tima`]) cancels the overflow entirely (no reload, no interrupt).
    ///
    /// | M-cycle | Flag state              | TIMA value | IF bit 2  |
    /// |---------|-------------------------|------------|-----------|
    /// | A+0     | overflow tick fires     | 0x00       | not set   |
    /// | A+1     | overflow_pending active | 0x00       | not set   | ← cycle A
    /// | A+2     | reload_pending active   | 0x00→TMA   | now set   | ← cycle B
    tima_overflow_pending: bool,

    /// Set during cycle A: the M-cycle after overflow, before the reload
    /// commits.  Cleared by the next [`step_m_cycle`] call, which loads TMA
    /// into TIMA and sets the interrupt flag (cycle B).
    ///
    /// Unlike `tima_overflow_pending`, a CPU write to TIMA while this flag is
    /// set does *not* cancel the reload — the TMA reload wins over the CPU
    /// write at the start of cycle B's tick.
    tima_reload_pending: bool,
}

impl Timer {
    /// Create a `Timer` in the state the boot ROM leaves it in at PC = 0x0100.
    ///
    /// Use this when the emulator skips the boot ROM so that games see the
    /// correct DIV value and TIMA phase from their very first instruction.
    pub(crate) fn new() -> Self {
        Self {
            system_counter: SYSTEM_COUNTER_POST_BOOT,
            ..Self::default()
        }
    }

    /// Create a `Timer` in the power-on cold-start state.
    ///
    /// Use this when a boot ROM image is supplied: the counter starts at 0
    /// and advances to `SYSTEM_COUNTER_POST_BOOT` naturally as the boot ROM
    /// runs, so the state at PC = 0x0100 will be correct automatically.
    pub(crate) fn cold_start() -> Self {
        Self::default()
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

    /// Read TIMA (0xFF05).
    ///
    /// During cycle B (`tima_reload_pending` active) the hardware is in the
    /// process of committing the TMA reload: reads to TIMA return TMA rather
    /// than the 0x00 that TIMA holds internally.  This matches real hardware
    /// where the reload value is visible to the CPU in the same M-cycle that
    /// the reload commits.
    pub(crate) fn read_tima(&self) -> u8 {
        if self.tima_reload_pending {
            self.tma
        } else {
            self.tima
        }
    }

    /// Write to TIMA (0xFF05).
    ///
    /// If the overflow is in cycle A (`tima_overflow_pending`), the write
    /// cancels the reload entirely — the written value stays in TIMA and no
    /// interrupt fires.
    ///
    /// If the overflow has already advanced to cycle B (`tima_reload_pending`),
    /// the write is *not* cancelled: the TMA reload at the start of the next
    /// tick will override this write.  Outside the window this is a plain write.
    pub(crate) fn write_tima(&mut self, value: u8) {
        self.tima_overflow_pending = false; // cancel cycle A if active
        // Do NOT clear tima_reload_pending: the cycle B reload wins
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

    /// Write to TAC (0xFF07).
    ///
    /// The signal that clocks TIMA is not the raw counter bit but the output
    /// of a **mux**: `timer_enabled AND (system_counter & selected_bit != 0)`.
    /// Writing TAC changes the mux inputs (the enable flag and/or the selected
    /// bit), which can create a falling edge on that combined signal even
    /// though the system counter itself has not moved.
    ///
    /// Examples:
    ///  - Disabling the timer (TAC bit 2: 1→0) while the selected bit is 1
    ///    drives the mux output from 1 to 0 — a falling edge.
    ///  - Switching clock-select (bits 0–1) from a bit that was 1 to a bit
    ///    that is 0 (with the timer enabled) does the same.
    ///
    /// This mirrors the DIV-write edge case handled by [`reset_divider`].
    pub(crate) fn write_tac(&mut self, value: u8) {
        // On DMG hardware the TAC register write propagates through a two-stage
        // synchronizer before the timer logic sees it, adding 2 M-cycles (8 T-cycles)
        // of effective latency.  The falling-edge detection therefore uses the counter
        // value 8 T-cycles ahead of the current counter — i.e. the counter value that
        // will be present when the write actually commits to the timer circuit.
        let check_counter = self.system_counter.wrapping_add(T_CYCLES_PER_M_CYCLE * 2);

        let old_bit = self.tima_bit();
        let old_mux = self.timer_enabled() && check_counter & old_bit != 0;

        self.tac = value;

        // Recompute the mux output with the new TAC (new enable + new bit tap).
        let new_mux = self.timer_enabled() && check_counter & self.tima_bit() != 0;

        // A 1→0 transition on the mux output is a falling edge: increment TIMA.
        if old_mux && !new_mux {
            self.advance_tima();
        }
    }

    // -----------------------------------------------------------------------
    // Private
    // -----------------------------------------------------------------------

    /// Advance the timer by one machine cycle (4 T-cycles).
    fn step_m_cycle(&mut self, interrupt_flag: &mut u8) {
        // Cycle B: commit a TIMA reload that was armed last M-cycle.
        // This fires at the START of the tick, AFTER any CPU write that
        // happened before the tick — so the TMA reload overrides a CPU write
        // to TIMA that occurred in the same M-cycle as cycle B.
        if self.tima_reload_pending {
            self.tima_reload_pending = false;
            self.tima = self.tma;
            *interrupt_flag |= TIMER_INTERRUPT_BIT;
        }

        // Cycle A → B transition: arm the reload one M-cycle after the
        // overflow tick so that cycle B fires the M-cycle after cycle A.
        if self.tima_overflow_pending {
            self.tima_overflow_pending = false;
            self.tima_reload_pending = true;
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

    /// Increment TIMA, starting the two-cycle reload sequence on overflow.
    ///
    /// On overflow TIMA wraps to 0x00 and `tima_overflow_pending` is set.
    /// One M-cycle later `step_m_cycle` promotes this to `tima_reload_pending`
    /// (cycle A).  One M-cycle after that the reload commits: TIMA = TMA and
    /// the interrupt fires (cycle B).
    fn advance_tima(&mut self) {
        if self.tima == 0xFF {
            self.tima = 0x00;
            self.tima_overflow_pending = true;
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
