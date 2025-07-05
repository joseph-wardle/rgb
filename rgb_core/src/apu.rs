use crate::memory::Memory;

pub struct APU {}

impl APU {
    pub fn new() -> Self {
        APU {}
    }
}

impl Memory for APU {
    fn read_byte(&self, address: u16) -> u8 {
        todo!()
    }
    fn write_byte(&mut self, address: u16, value: u8) {
        todo!()
    }
}
