/// Number of CPU clock cycles per machine cycle on the DMG.
const CPU_CYCLES_PER_MACHINE_CYCLE: u16 = 4;
/// The divider register (`DIV`) increments every 256 machine cycles.
const DIV_PERIOD_MACHINE_CYCLES: u16 = 256;
/// TIMA periods expressed in machine cycles for each TAC clock select value.
const TIMA_PERIODS_MACHINE_CYCLES: [u16; 4] = [
    256, // 00: 4096 Hz
    4,   // 01: 262_144 Hz
    16,  // 10: 65_536 Hz
    64,  // 11: 16_384 Hz
];

#[derive(Debug, Default)]
pub(crate) struct Timer {
    // This register is incremented at a rate of 16384Hz (~16779Hz on SGB). Writing any value to
    // this register resets it to $00. This register is reset when executing the stop instruction,
    // and begins ticking again once stop mode ends. This also occurs during a speed switch.
    pub(crate) div: u8, // Divider Register

    // This timer is incremented at the clock frequency specified by the TAC register ($FF07). When
    // it overflows it is reset to the value specified in TMA (FF06) and an interrupt is requested
    pub(crate) tima: u8, // Timer Counter

    // When TIMA overflows, it is reset to the value in this register and an interrupt is requested.
    pub(crate) tma: u8, // Timer Modulo

    // This register is used to control the timer frequency.
    // | 7  6  5  4  3 |   2    |     1  0     |
    // | ------------- | ------ | ------------ |
    // |               | Enable | Clock select |
    //
    // - Enable: Controls whether TIMA is incremented. Note that DIV is always counting, regardless
    //   of this bit.
    // - Clock select: Controls the frequency at which TIMA is incremented, as follows:
    //
    // | Clock select | Increment every | DMG, SGB2, CGB 1x mode | SGB1       | CGB 2x mode |
    // | ------------ | --------------- | ---------------------- | ---------- | ----------- |
    // | 00           | 256 M-cycles    | 4096 hz                | ~4194 hz   | 8192 hz     |
    // | 01           | 4 M-cycles      | 262144 hz              | ~268400 hz | 524288 hz   |
    // | 10           | 16 M-cycles     | 65536 hz               | ~67110 hz  | 131072 hz   |
    // | 11           | 64 M-cycles     | 16384 hz               | ~16780 hz  | 32768 hz    |
    //
    // Note that writing to this register may increase TIMA once!
    pub(crate) tac: u8, // Timer Control

    div_counter: u16,
    tima_counter: u16,
}

impl Timer {
    pub(crate) fn new() -> Self {
        Timer {
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            div_counter: 0,
            tima_counter: 0,
        }
    }

    pub(crate) fn step(&mut self, cycles: u16, interrupt_flag: &mut u8) {
        self.div_counter = self.div_counter.wrapping_add(cycles);
        let div_threshold = DIV_PERIOD_MACHINE_CYCLES * CPU_CYCLES_PER_MACHINE_CYCLE;
        while self.div_counter >= div_threshold {
            self.div_counter -= div_threshold;
            self.div = self.div.wrapping_add(1);
        }

        if self.timer_enabled() {
            let threshold = self.tima_period_cpu_cycles();

            self.tima_counter = self.tima_counter.wrapping_add(cycles);
            while self.tima_counter >= threshold {
                self.tima_counter -= threshold;
                if self.tima == 0xFF {
                    self.tima = self.tma;
                    *interrupt_flag |= 0x04;
                } else {
                    self.tima = self.tima.wrapping_add(1);
                }
            }
        }
    }

    #[inline]
    fn timer_enabled(&self) -> bool {
        (self.tac & 0b100) != 0
    }

    #[inline]
    fn tima_period_cpu_cycles(&self) -> u16 {
        let index = (self.tac & 0b11) as usize;
        TIMA_PERIODS_MACHINE_CYCLES[index] * CPU_CYCLES_PER_MACHINE_CYCLE
    }

    /// Writing to the DIV register clears the entire divider. This resets both the public register
    /// and the hidden cycle counter that feeds it so the next increment starts from a clean phase.
    pub(crate) fn reset_divider(&mut self) {
        self.div = 0;
        self.div_counter = 0;
    }
}
