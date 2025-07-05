pub struct Timer {
    // This register is incremented at a rate of 16384Hz (~16779Hz on SGB). Writing any value to
    // this register resets it to $00. This register is reset when executing the stop instruction,
    // and begins ticking again once stop mode ends. This also occurs during a speed switch.
    pub div: u8, // Divider Register

    // This timer is incremented at the clock frequency specified by the TAC register ($FF07). When
    // it overflows it is reset to the value specified in TMA (FF06) and an interrupt is requested
    pub tima: u8, // Timer Counter

    // When TIMA overflows, it is reset to the value in this register and an interrupt is requested.
    pub tma: u8, // Timer Modulo

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
    pub tac: u8, // Timer Control
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
        }
    }
}
