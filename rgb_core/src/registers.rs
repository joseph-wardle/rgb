#[expect(clippy::upper_case_acronyms, non_camel_case_types)]
pub(crate) enum Flag {
    ZERO = 0b1000_0000,       // Often named just Z
    SUBTRACT = 0b0100_0000,   // Often named just N
    HALF_CARRY = 0b0010_0000, // Often named just H
    CARRY = 0b0001_0000,      // Often named just C
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub struct Registers {
    pub(crate) a: u8, // Accumulator
    pub(crate) f: u8, // Flags
    pub(crate) b: u8,
    pub(crate) c: u8,
    pub(crate) d: u8,
    pub(crate) e: u8,
    pub(crate) h: u8,
    pub(crate) l: u8,
    pub(crate) pc: u16, // Program counter
    pub(crate) sp: u16, // Stack pointer
}

impl Registers {
    /// Register state after the DMG boot ROM completes.
    /// The emulator starts here when no boot ROM is provided.
    ///
    /// Values taken from Pan Docs § "Power Up Sequence":
    /// A=01 F=B0 B=00 C=13 D=00 E=D8 H=01 L=4D SP=FFFE PC=0100
    pub(crate) fn new() -> Self {
        Self {
            a: 0x01,
            f: 0xB0, // Z=1 N=0 H=1 C=1
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
        }
    }

    /// Register state when the hardware first powers on, before the boot ROM
    /// runs.  Used when a boot ROM image is provided to `DMG::new`; the boot
    /// ROM will initialise these registers to the post-boot values as it runs.
    pub(crate) fn cold_start() -> Self {
        Self::default() // all zeros; PC = 0x0000
    }

    pub fn get_af(&self) -> u16 {
        (self.a as u16) << 8 | self.f as u16
    }
    pub fn get_bc(&self) -> u16 {
        (self.b as u16) << 8 | self.c as u16
    }
    pub fn get_de(&self) -> u16 {
        (self.d as u16) << 8 | self.e as u16
    }
    pub fn get_hl(&self) -> u16 {
        (self.h as u16) << 8 | self.l as u16
    }

    pub(crate) fn get_hli(&mut self) -> u16 {
        let hl = self.get_hl();
        self.set_hl(hl.wrapping_add(1));
        hl
    }

    pub(crate) fn get_hld(&mut self) -> u16 {
        let hl = self.get_hl();
        self.set_hl(hl.wrapping_sub(1));
        hl
    }

    pub(crate) fn set_af(&mut self, value: u16) {
        self.a = (value >> 8) as u8;
        self.f = (value as u8) & 0xF0;
    }

    pub(crate) fn set_bc(&mut self, value: u16) {
        self.b = (value >> 8) as u8;
        self.c = value as u8;
    }

    pub(crate) fn set_de(&mut self, value: u16) {
        self.d = (value >> 8) as u8;
        self.e = value as u8;
    }

    pub(crate) fn set_hl(&mut self, value: u16) {
        self.h = (value >> 8) as u8;
        self.l = value as u8;
    }

    pub(crate) fn get_flag(&self, flag: Flag) -> bool {
        (self.f & flag as u8) != 0
    }

    pub(crate) fn set_flag(&mut self, flag: Flag, value: bool) {
        if value {
            self.f |= flag as u8;
        } else {
            self.f &= !(flag as u8);
        }
    }
}
