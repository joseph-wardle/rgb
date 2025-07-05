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


pub trait MemoryBus : Memory {}