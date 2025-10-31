use crate::memory::Memory;

#[derive(Debug, Default)]
pub struct APU {}

impl APU {
    pub fn new() -> Self {
        APU {}
    }
}

impl Memory for APU {
    #[expect(
        unused_variables,
        reason = "APU is not fully implemented yet, but will be used in the future when audio is added"
    )]
    fn read_byte(&self, address: u16) -> u8 {
        0 // todo!()
    }

    #[expect(
        unused_variables,
        reason = "APU is not fully implemented yet, but will be used in the future when audio is added"
    )]
    fn write_byte(&mut self, address: u16, value: u8) {
        // todo!()
    }
}
