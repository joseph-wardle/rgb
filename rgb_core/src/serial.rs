//! DMG serial port (SB / SC).
//!
//! ## Hardware overview
//!
//! The serial port is an SPI shift register.  One Game Boy drives the clock
//! (master, SC bit 0 = 1); the other waits for it (slave, SC bit 0 = 0).
//! Each transfer shifts 8 bits over the wire, one per clock edge.  While the
//! transfer is in progress SB holds a blend of outgoing and incoming bits:
//!
//! | Step     |   7   |   6   |   5   |   4   |   3   |   2   |   1   |   0   |
//! | -------- | ----- | ----- | ----- | ----- | ----- | ----- | ----- | ----- |
//! | Initial  | out.7 | out.6 | out.5 | out.4 | out.3 | out.2 | out.1 | out.0 |
//! | 1 shift  | out.6 | out.5 | out.4 | out.3 | out.2 | out.1 | out.0 |  in.7 |
//! | …        |       |       |       |       |       |       |       |       |
//! | 8 shifts |  in.7 |  in.6 |  in.5 |  in.4 |  in.3 |  in.2 |  in.1 |  in.0 |
//!
//! SC register layout:
//!
//! | Bit 7           | Bits 6–2 | Bit 1       | Bit 0        |
//! | --------------- | -------- | ----------- | ------------ |
//! | Transfer enable | (unused) | Clock speed | Clock select |
//!
//! On real hardware a master-mode transfer takes 8,192 T-cycles (normal speed)
//! to shift all 8 bits.  When SC bit 7 is cleared the serial interrupt fires.
//! A slave-mode transfer (SC bit 0 = 0) requires a second Game Boy to drive
//! the clock; without one the transfer never completes and no interrupt fires.
//!
//! ## Approximation
//!
//! We model transfers as **instantaneous**: writing SC with bits 7 and 0 both
//! set immediately captures SB into the output buffer, clears SC bit 7, and
//! signals completion.  This is sufficient for all single-player games and
//! test ROMs that use the serial port for text output (Blargg suite), because
//! those ROMs write SB, then SC, then poll SC bit 7 or wait for the interrupt
//! — all of which complete correctly with an instantaneous model.
//!
//! The one known test that requires cycle-accurate serial timing is the mooneye
//! `serial/boot_sclk_align-dmgABCmgb` test, which verifies the alignment of
//! the first clock edge after the boot ROM finishes.  That test is marked
//! `#[ignore]` in `mooneye_acceptance.rs`.

#[derive(Debug, Default)]
pub struct Serial {
    pub(crate) sb: u8, // Serial transfer data
    pub(crate) sc: u8, // Serial transfer control
    buffer: Vec<u8>,
}

impl Serial {
    pub fn output_string(&self) -> String {
        String::from_utf8_lossy(&self.buffer).to_string()
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

impl Serial {
    pub(crate) fn new() -> Self {
        Serial {
            sb: 0,
            sc: 0,
            buffer: Vec::new(),
        }
    }

    pub(crate) fn write_data(&mut self, value: u8) {
        self.sb = value;
        self.log_data_write(value, self.buffer.len());
    }

    pub(crate) fn write_control(&mut self, value: u8) -> bool {
        let previous = self.sc;
        let raw = value;
        self.sc = value;
        let start_transfer = (raw & 0x80) != 0;
        // Bit 0 = 1 means internal clock (this GB drives the transfer).
        // Bit 0 = 0 means external clock (slave mode: waits for another GB).
        // Without a second Game Boy, an external-clock transfer never completes,
        // so we must NOT fire the serial interrupt for it.
        let internal_clock = (raw & 0x01) != 0;
        let mut transferred = None;
        let mut completed = false;
        if start_transfer && internal_clock {
            let byte = self.sb;
            self.buffer.push(byte);
            self.sc &= 0x7F; // transfer complete
            transferred = Some(byte);
            completed = true;
        }
        self.log_control_write(
            previous,
            raw,
            self.sc,
            start_transfer,
            transferred,
            self.buffer.len(),
        );
        completed
    }
}
