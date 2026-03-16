pub trait Memory {
    fn read_byte(&self, address: u16) -> u8;
    fn write_byte(&mut self, address: u16, value: u8);

    fn read_word(&self, address: u16) -> u16 {
        let lo = self.read_byte(address) as u16;
        let hi = self.read_byte(address.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    fn write_word(&mut self, address: u16, value: u16) {
        let lo = value as u8;
        let hi = (value >> 8) as u8;
        self.write_byte(address, lo);
        self.write_byte(address.wrapping_add(1), hi);
    }
}

pub trait MemoryBus: Memory {
    /// Advance all clocked hardware (timer, PPU, APU) by one machine cycle (4 T-cycles).
    ///
    /// The CPU calls this after each M-cycle of execution — opcode fetch, operand
    /// fetch, memory read, memory write, or an internal delay cycle.  Stepping
    /// devices M-cycle-by-M-cycle, interleaved with instruction execution, is
    /// what makes timer and PPU register reads/writes see the correct device state
    /// at the precise cycle the CPU accesses them.
    ///
    /// The default implementation is a no-op so that test stubs do not need to
    /// implement it.
    fn tick_m_cycle(&mut self) {}
}
