//! CPU execution engine for the DMG Game Boy (see Pan Docs for reference).
//!
//! The design here deliberately favours readability and small, well-named helpers
//! so the control flow can be studied without mental gymnastics.

mod opcode;

use self::opcode::{CycleCost, cb_cycle_cost, cycle_cost};

use crate::memory::MemoryBus;
use crate::registers::Flag::{CARRY, HALF_CARRY, SUBTRACT, ZERO};
use crate::registers::Registers;

#[derive(Default)]
struct Clock {
    last_instruction: u8,
}

impl Clock {
    fn record(&mut self, cycles: u8) {
        self.last_instruction = cycles;
    }

    fn last(&self) -> u8 {
        self.last_instruction
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum HaltState {
    #[default]
    Running,
    Halted,
}

#[derive(Copy, Clone, Debug)]
struct CycleResult {
    cost: CycleCost,
    took_conditional: bool,
}

impl CycleResult {
    fn new(cost: CycleCost) -> Self {
        Self {
            cost,
            took_conditional: false,
        }
    }

    fn set_cost(&mut self, cost: CycleCost) {
        self.cost = cost;
        self.took_conditional = false;
    }

    fn take_conditional(&mut self) {
        self.took_conditional = true;
    }

    fn total(self) -> u8 {
        self.cost.total(self.took_conditional)
    }
}

pub struct CPU {
    reg: Registers,
    clock: Clock,
    pub(crate) halt_state: HaltState,
    pub(crate) ime: bool,
    pub(crate) ime_scheduled: Option<u8>,
    halt_bug: bool,
}

impl Default for CPU {
    fn default() -> Self {
        Self::new()
    }
}


/// Read one byte from the bus and tick all hardware by one M-cycle (4 T-cycles).
///
/// This is a free function (not a method) so it only borrows `mmu`, leaving
/// `self` free to be borrowed separately for register access in the same expression.
fn bus_read(mmu: &mut impl MemoryBus, address: u16) -> u8 {
    let value = mmu.read_byte(address);
    mmu.tick_m_cycle();
    value
}

/// Write one byte to the bus and tick all hardware by one M-cycle (4 T-cycles).
///
/// Free function for the same reason as `bus_read`.
fn bus_write(mmu: &mut impl MemoryBus, address: u16, value: u8) {
    mmu.write_byte(address, value);
    mmu.tick_m_cycle();
}

impl CPU {
    fn fetch_byte(&mut self, mmu: &mut impl MemoryBus) -> u8 {
        // Each opcode or operand fetch is one M-cycle: read the byte, advance
        // PC, then tick all hardware devices for those 4 T-cycles.
        let byte = bus_read(mmu, self.reg.pc);
        self.reg.pc = self.reg.pc.wrapping_add(1);
        if self.halt_bug {
            self.halt_bug = false;
            self.reg.pc = self.reg.pc.wrapping_sub(1);
        }
        byte
    }

    fn fetch_word(&mut self, mmu: &mut impl MemoryBus) -> u16 {
        let lo = self.fetch_byte(mmu) as u16;
        let hi = self.fetch_byte(mmu) as u16;
        (hi << 8) | lo
    }

    /// Push a 16-bit value onto the stack.
    ///
    /// Hardware writes the high byte to [SP−1] first, then the low byte to
    /// [SP−2].  Each write is one M-cycle (via `bus_write`).
    /// The caller is responsible for the one internal M-cycle that the hardware
    /// spends decrementing SP before the writes begin.
    fn push(&mut self, mmu: &mut impl MemoryBus, value: u16) {
        let hi = (value >> 8) as u8;
        let lo = value as u8;
        self.reg.sp = self.reg.sp.wrapping_sub(1);
        bus_write(mmu, self.reg.sp, hi);
        self.reg.sp = self.reg.sp.wrapping_sub(1);
        bus_write(mmu, self.reg.sp, lo);
    }

    /// Pop a 16-bit value from the stack.
    ///
    /// Reads low byte from [SP], high byte from [SP+1], then advances SP by 2.
    /// Each read is one M-cycle (via `bus_read`).
    fn pop(&mut self, mmu: &mut impl MemoryBus) -> u16 {
        let lo = bus_read(mmu, self.reg.sp) as u16;
        self.reg.sp = self.reg.sp.wrapping_add(1);
        let hi = bus_read(mmu, self.reg.sp) as u16;
        self.reg.sp = self.reg.sp.wrapping_add(1);
        (hi << 8) | lo
    }

    fn call(&mut self, mmu: &mut impl MemoryBus, address: u16) {
        self.push(mmu, self.reg.pc);
        self.reg.pc = address;
    }

    fn rst(&mut self, mmu: &mut impl MemoryBus, address: u16) {
        self.call(mmu, address);
    }
}

impl CPU {
    // Add n to A
    // n = A,B,C,D,E,H,L,(HL),imm8
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Reset
    // HALF_CARRY - Set if carry from bit 3
    // CARRY      - Set if carry from bit 7
    fn add(&mut self, n: u8) {
        let a = self.reg.a;
        let result = a.wrapping_add(n);

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, false);
        self.reg
            .set_flag(HALF_CARRY, (a & 0x0F) + (n & 0x0F) > 0x0F);
        self.reg.set_flag(CARRY, (a as u16) + (n as u16) > 0xFF);

        self.reg.a = result;
    }

    // Add n + carry flag to A
    // n = A,B,C,D,E,H,L,(HL),imm8
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Reset
    // HALF_CARRY - Set if carry from bit 3
    // CARRY      - Set if carry from bit 7
    fn adc(&mut self, n: u8) {
        let a = self.reg.a;
        let carry = self.reg.get_flag(CARRY) as u8;
        let result = a.wrapping_add(n).wrapping_add(carry);

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, false);
        self.reg
            .set_flag(HALF_CARRY, (a & 0x0F) + (n & 0x0F) + carry > 0x0F);
        self.reg
            .set_flag(CARRY, (a as u16) + (n as u16) + carry as u16 > 0xFF);

        self.reg.a = result;
    }

    // Subtract n from A
    // n = A,B,C,D,E,H,L,(HL),imm8
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Set
    // HALF_CARRY - Set if no borrow from bit 4
    // CARRY      - Set if no borrow
    fn sub(&mut self, n: u8) {
        let a = self.reg.a;
        let result = a.wrapping_sub(n);

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, true);
        self.reg.set_flag(HALF_CARRY, (a & 0x0F) < (n & 0x0F));
        self.reg.set_flag(CARRY, (a as u16) < (n as u16));

        self.reg.a = result;
    }

    // Subtract n + carry flag from A
    // n = A,B,C,D,E,H,L,(HL),imm8
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Set
    // HALF_CARRY - Set if no borrow from bit 4
    // CARRY      - Set if no borrow
    fn sbc(&mut self, n: u8) {
        let a = self.reg.a;
        let carry = self.reg.get_flag(CARRY) as u8;
        let result = a.wrapping_sub(n).wrapping_sub(carry);

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, true);
        self.reg
            .set_flag(HALF_CARRY, (a & 0x0F) < (n & 0x0F) + carry);
        self.reg
            .set_flag(CARRY, (a as u16) < (n as u16) + carry as u16);

        self.reg.a = result;
    }

    // Logical AND n with A, result in A
    // n = A,B,C,D,E,H,L,(HL),imm8
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Reset
    // HALF_CARRY - Set
    // CARRY      - Reset
    fn and(&mut self, n: u8) {
        let result = self.reg.a & n;

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, false);
        self.reg.set_flag(HALF_CARRY, true);
        self.reg.set_flag(CARRY, false);

        self.reg.a = result;
    }

    // Logical OR n with register A, result in A
    // n = A,B,C,D,E,H,L,(HL),imm8
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Reset
    // HALF_CARRY - Reset
    // CARRY      - Reset
    fn or(&mut self, n: u8) {
        let result = self.reg.a | n;

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, false);
        self.reg.set_flag(HALF_CARRY, false);
        self.reg.set_flag(CARRY, false);

        self.reg.a = result;
    }

    // Logical exclusive OR n with register A, result in A
    // n = A,B,C,D,E,H,L,(HL),imm8
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Reset
    // HALF_CARRY - Reset
    // CARRY      - Reset
    fn xor(&mut self, n: u8) {
        let result = self.reg.a ^ n;

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, false);
        self.reg.set_flag(HALF_CARRY, false);
        self.reg.set_flag(CARRY, false);

        self.reg.a = result;
    }

    // Compare A with n
    // n = A,B,C,D,E,H,L,(HL),imm8
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Set
    // HALF_CARRY - Set if no borrow from bit 4
    // CARRY      - Set for no borrow
    fn cp(&mut self, n: u8) {
        let a = self.reg.a;
        let result = a.wrapping_sub(n);

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, true);
        self.reg.set_flag(HALF_CARRY, (a & 0x0F) < (n & 0x0F));
        self.reg.set_flag(CARRY, (a as u16) < (n as u16));
    }

    // Increment register n
    // n = A,B,C,D,E,H,L,(HL)
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Reset
    // HALF_CARRY - Set if carry from bit 3
    // CARRY      - Not affected
    fn inc(&mut self, n: u8) -> u8 {
        let result = n.wrapping_add(1);

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, false);
        self.reg.set_flag(HALF_CARRY, n & 0x0F == 0x0F);

        result
    }

    // Decrement register n
    // n = A,B,C,D,E,H,L,(HL)
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Set
    // HALF_CARRY - Set if no borrow from bit 4
    // CARRY      - Not affected
    fn dec(&mut self, n: u8) -> u8 {
        let result = n.wrapping_sub(1);

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, true);
        self.reg.set_flag(HALF_CARRY, n & 0x0F == 0x00);

        result
    }

    // Add n to HL
    // n = BC,DE,HL,SP
    //
    // ZERO       - Not affected
    // SUBTRACT   - Reset
    // HALF_CARRY - Set if carry from bit 11
    // CARRY      - Set if carry from bit 15
    fn add_hl(&mut self, n: u16) {
        let hl = self.reg.get_hl();
        let result = hl.wrapping_add(n);

        self.reg.set_flag(SUBTRACT, false);
        self.reg
            .set_flag(HALF_CARRY, (hl & 0x0FFF) + (n & 0x0FFF) > 0x0FFF);
        self.reg.set_flag(CARRY, (hl as u32) + (n as u32) > 0xFFFF);

        self.reg.set_hl(result);
    }

    // Add signed imm8 to SP.
    //
    // ZERO       - Reset
    // SUBTRACT   - Reset
    // HALF_CARRY - Set if carry from bit 11
    // CARRY      - Set if carry from bit 15
    fn add_sp(&mut self, n: u8) {
        let sp = self.reg.sp;
        let n = n as i8 as i16 as u16;

        let result = sp.wrapping_add(n);

        self.reg.set_flag(ZERO, false);
        self.reg.set_flag(SUBTRACT, false);

        self.reg
            .set_flag(HALF_CARRY, (sp & 0x000F) + (n & 0x000F) > 0x000F);
        self.reg
            .set_flag(CARRY, (sp & 0x00FF) + (n & 0x00FF) > 0x00FF);

        self.reg.sp = result;
    }

    // Swap upper & lower nibles of n
    // n = A,B,C,D,E,H,L,(HL)
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Reset
    // HALF_CARRY - Reset
    // CARRY      - Reset
    fn swap(&mut self, n: u8) -> u8 {
        let upper = n >> 4;
        let lower = n & 0x0F;
        let result = (lower << 4) | upper;

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, false);
        self.reg.set_flag(HALF_CARRY, false);
        self.reg.set_flag(CARRY, false);

        result
    }

    // Decimal adjust register A
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Not affected
    // HALF_CARRY - Reset
    // CARRY      - Set or reset according to operation
    fn daa(&mut self) {
        const LOW_ADJUST: u8 = 0x06;
        const HIGH_ADJUST: u8 = 0x60;

        let mut a = self.reg.a;
        let mut adjust: u8 = 0;
        let subtract = self.reg.get_flag(SUBTRACT);
        let mut carry_out = self.reg.get_flag(CARRY);

        if subtract {
            if self.reg.get_flag(HALF_CARRY) {
                adjust |= LOW_ADJUST;
            }
            if self.reg.get_flag(CARRY) {
                adjust |= HIGH_ADJUST;
            }
            a = a.wrapping_sub(adjust);
        } else {
            let lower_nibble_overflow = (a & 0x0F) > 0x09;
            if self.reg.get_flag(HALF_CARRY) || lower_nibble_overflow {
                adjust |= LOW_ADJUST;
            }
            let upper_overflow = a > 0x99;
            if self.reg.get_flag(CARRY) || upper_overflow {
                adjust |= HIGH_ADJUST;
                carry_out = true;
            }
            a = a.wrapping_add(adjust);
        }

        self.reg.set_flag(ZERO, a == 0);
        self.reg.set_flag(SUBTRACT, subtract);
        self.reg.set_flag(HALF_CARRY, false);
        self.reg.set_flag(CARRY, carry_out);

        self.reg.a = a;
    }

    // Complement A
    //
    // ZERO       - Not affected
    // SUBTRACT   - Set
    // HALF_CARRY - Set
    // CARRY      - Not affected
    fn cpl(&mut self) {
        self.reg.a = !self.reg.a;

        self.reg.set_flag(SUBTRACT, true);
        self.reg.set_flag(HALF_CARRY, true);
    }

    // Complement carry flag
    //
    // ZERO       - Not affected
    // SUBTRACT   - Reset
    // HALF_CARRY - Reset
    // CARRY      - Complemented
    fn ccf(&mut self) {
        self.reg.set_flag(SUBTRACT, false);
        self.reg.set_flag(HALF_CARRY, false);
        self.reg.set_flag(CARRY, !self.reg.get_flag(CARRY));
    }

    // Set carry flag
    //
    // ZERO       - Not affected
    // SUBTRACT   - Reset
    // HALF_CARRY - Reset
    // CARRY      - Set
    fn scf(&mut self) {
        self.reg.set_flag(SUBTRACT, false);
        self.reg.set_flag(HALF_CARRY, false);
        self.reg.set_flag(CARRY, true);
    }

    // Rotate n left
    // n = A,B,C,D,E,H,L,(HL)
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Reset
    // HALF_CARRY - Reset
    // CARRY      - Set to bit 7 of A
    fn rlc(&mut self, n: u8) -> u8 {
        let result = n.rotate_left(1);

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, false);
        self.reg.set_flag(HALF_CARRY, false);
        self.reg.set_flag(CARRY, n & 0x80 != 0);

        result
    }

    // Rotate n left through carry
    // n = A,B,C,D,E,H,L,(HL)
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Reset
    // HALF_CARRY - Reset
    // CARRY      - Set to bit 7 of A
    fn rl(&mut self, n: u8) -> u8 {
        let carry = self.reg.get_flag(CARRY) as u8;
        let result = (n << 1) | carry;

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, false);
        self.reg.set_flag(HALF_CARRY, false);
        self.reg.set_flag(CARRY, n & 0x80 != 0);

        result
    }

    // Rotate n right
    // n = A,B,C,D,E,H,L,(HL)
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Reset
    // HALF_CARRY - Reset
    // CARRY      - Set to bit 0 of A
    fn rrc(&mut self, n: u8) -> u8 {
        let result = n.rotate_right(1);

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, false);
        self.reg.set_flag(HALF_CARRY, false);
        self.reg.set_flag(CARRY, n & 0x01 != 0);

        result
    }

    // Rotate n right through carry
    // n = A,B,C,D,E,H,L,(HL)
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Reset
    // HALF_CARRY - Reset
    // CARRY      - Set to bit 0 of A
    fn rr(&mut self, n: u8) -> u8 {
        let carry = self.reg.get_flag(CARRY) as u8;
        let result = (n >> 1) | (carry << 7);

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, false);
        self.reg.set_flag(HALF_CARRY, false);
        self.reg.set_flag(CARRY, n & 0x01 != 0);

        result
    }

    // Shift n left into carry
    // n = A,B,C,D,E,H,L,(HL)
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Reset
    // HALF_CARRY - Reset
    // CARRY      - Set to bit 7 of A
    fn sla(&mut self, n: u8) -> u8 {
        let result = n << 1;

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, false);
        self.reg.set_flag(HALF_CARRY, false);
        self.reg.set_flag(CARRY, n & 0x80 != 0);

        result
    }

    // Shift n right into carry
    // n = A,B,C,D,E,H,L,(HL)
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Reset
    // HALF_CARRY - Reset
    // CARRY      - Set to bit 0 of A
    fn sra(&mut self, n: u8) -> u8 {
        let result = (n >> 1) | (n & 0x80);

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, false);
        self.reg.set_flag(HALF_CARRY, false);
        self.reg.set_flag(CARRY, n & 0x01 != 0);

        result
    }

    // Shift n right into carry. Most significant byte set to 0
    // n = A,B,C,D,E,H,L,(HL)
    //
    // ZERO       - Set if result is zero
    // SUBTRACT   - Reset
    // HALF_CARRY - Reset
    // CARRY      - Set to bit 0 of A
    fn srl(&mut self, n: u8) -> u8 {
        let result = n >> 1;

        self.reg.set_flag(ZERO, result == 0);
        self.reg.set_flag(SUBTRACT, false);
        self.reg.set_flag(HALF_CARRY, false);
        self.reg.set_flag(CARRY, n & 0x01 != 0);

        result
    }

    // Test bit b in register n
    // b = 0-7, n = A,B,C,D,E,H,L,(HL)
    //
    // ZERO       - Set if bit b of register n is 0
    // SUBTRACT   - Reset
    // HALF_CARRY - Set
    // CARRY      - Not affected
    fn bit(&mut self, b: u8, n: u8) {
        self.reg.set_flag(ZERO, n & (1 << b) == 0);
        self.reg.set_flag(SUBTRACT, false);
        self.reg.set_flag(HALF_CARRY, true);
    }

    // Reset bit b in register n
    // b = 0-7, n = A,B,C,D,E,H,L,(HL)
    //
    // ZERO       - Not affected
    // SUBTRACT   - Reset
    // HALF_CARRY - Not affected
    // CARRY      - Not affected
    fn res(&mut self, b: u8, n: u8) -> u8 {
        n & !(1 << b)
    }

    // Set bit b in register n
    // b = 0-7, n = A,B,C,D,E,H,L,(HL)
    //
    // ZERO       - Not affected
    // SUBTRACT   - Reset
    // HALF_CARRY - Not affected
    // CARRY      - Not affected
    fn set(&mut self, b: u8, n: u8) -> u8 {
        n | (1 << b)
    }

    // Add n to current address and jump to it
    // n = signed imm8
    fn jr(&mut self, n: u8) {
        let n = n as i8;
        if n >= 0 {
            self.reg.pc = self.reg.pc.wrapping_add(n as u16);
        } else {
            self.reg.pc = self.reg.pc.wrapping_sub((-n) as u16);
        }
    }
}

impl CPU {
    pub(crate) fn new() -> Self {
        Self {
            reg: Registers::new(),
            clock: Clock::default(),
            halt_state: HaltState::Running,
            ime: false,
            ime_scheduled: None,
            halt_bug: false,
        }
    }

    pub(crate) fn new_cold() -> Self {
        Self {
            reg: Registers::cold_start(),
            clock: Clock::default(),
            halt_state: HaltState::Running,
            ime: false,
            ime_scheduled: None,
            halt_bug: false,
        }
    }

    #[inline]
    fn enter_halt_mode(&mut self) {
        self.halt_state = HaltState::Halted;
        self.halt_bug = false;
        self.log_halt_enter();
    }

    #[inline]
    fn enter_stop_mode(&mut self) {
        // Without host input wiring we temporarily treat STOP as a 4-cycle NOP.
        self.log_stop_enter();
    }

    #[inline]
    fn trigger_halt_bug(&mut self) {
        self.halt_bug = true;
        self.log_halt_bug();
    }

    pub(crate) fn step(&mut self, mmu: &mut impl MemoryBus) -> u8 {
        if let Some(cycles) = self.step_halt_state(mmu) {
            self.clock.record(cycles);
            self.advance_ime_schedule();
            return cycles;
        }

        let pc_before = self.reg.pc;
        let opcode = self.fetch_byte(mmu);

        self.log_inst_start(pc_before, opcode);

        let cycles = self.execute_opcode(opcode, mmu);
        let total = cycles.total();

        self.log_inst_done(opcode, total, cycles.took_conditional);

        self.clock.record(total);
        self.advance_ime_schedule();
        total
    }

    pub fn last_cycles(&self) -> u8 {
        self.clock.last()
    }

    pub fn registers(&self) -> &Registers {
        &self.reg
    }

    fn execute_opcode(&mut self, opcode: u8, mmu: &mut impl MemoryBus) -> CycleResult {
        let mut cycles = CycleResult::new(cycle_cost(opcode));
        self.execute_main_opcode(opcode, mmu, &mut cycles);
        cycles
    }

    fn execute_main_opcode(
        &mut self,
        opcode: u8,
        mmu: &mut impl MemoryBus,
        cycles: &mut CycleResult,
    ) {
        match opcode {
            // NOP
            0x00 => {} // NOP

            // LD r16, imm16
            0x01 => {
                let word = self.fetch_word(mmu);
                self.reg.set_bc(word)
            } // LD BC,imm16
            0x11 => {
                let word = self.fetch_word(mmu);
                self.reg.set_de(word)
            } // LD DE,imm16
            0x21 => {
                let word = self.fetch_word(mmu);
                self.reg.set_hl(word)
            } // LD HL,imm16
            0x31 => self.reg.sp = self.fetch_word(mmu), // LD SP,imm16

            // LD (r16), A
            0x02 => bus_write(mmu, self.reg.get_bc(), self.reg.a), // LD (BC),A
            0x12 => bus_write(mmu, self.reg.get_de(), self.reg.a), // LD (DE),A
            0x22 => bus_write(mmu, self.reg.get_hli(), self.reg.a), // LD (HL+),A
            0x32 => bus_write(mmu, self.reg.get_hld(), self.reg.a), // LD (HL-),A

            // LD A, (r16)
            0x0A => self.reg.a = bus_read(mmu, self.reg.get_bc()), // LD A,(BC)
            0x1A => self.reg.a = bus_read(mmu, self.reg.get_de()), // LD A,(DE)
            0x2A => self.reg.a = bus_read(mmu, self.reg.get_hli()), // LD A,(HL+)
            0x3A => self.reg.a = bus_read(mmu, self.reg.get_hld()), // LD A,(HL-)

            // LD (imm16), SP
            0x08 => {
                let addr = self.fetch_word(mmu);
                bus_write(mmu, addr, self.reg.sp as u8);
                bus_write(mmu, addr.wrapping_add(1), (self.reg.sp >> 8) as u8)
            } // LD (imm16),SP

            // INC r16 — 8 cycles: opcode fetch + 1 internal (16-bit increment).
            0x03 => { self.reg.set_bc(self.reg.get_bc().wrapping_add(1)); mmu.tick_m_cycle() } // INC BC
            0x13 => { self.reg.set_de(self.reg.get_de().wrapping_add(1)); mmu.tick_m_cycle() } // INC DE
            0x23 => { self.reg.set_hl(self.reg.get_hl().wrapping_add(1)); mmu.tick_m_cycle() } // INC HL
            0x33 => { self.reg.sp = self.reg.sp.wrapping_add(1); mmu.tick_m_cycle() }          // INC SP

            // DEC r16 — 8 cycles: opcode fetch + 1 internal (16-bit decrement).
            0x0B => { self.reg.set_bc(self.reg.get_bc().wrapping_sub(1)); mmu.tick_m_cycle() } // DEC BC
            0x1B => { self.reg.set_de(self.reg.get_de().wrapping_sub(1)); mmu.tick_m_cycle() } // DEC DE
            0x2B => { self.reg.set_hl(self.reg.get_hl().wrapping_sub(1)); mmu.tick_m_cycle() } // DEC HL
            0x3B => { self.reg.sp = self.reg.sp.wrapping_sub(1); mmu.tick_m_cycle() }          // DEC SP

            // ADD HL, r16 — 8 cycles: opcode fetch + 1 internal (16-bit addition).
            0x09 => { self.add_hl(self.reg.get_bc()); mmu.tick_m_cycle() } // ADD HL,BC
            0x19 => { self.add_hl(self.reg.get_de()); mmu.tick_m_cycle() } // ADD HL,DE
            0x29 => { self.add_hl(self.reg.get_hl()); mmu.tick_m_cycle() } // ADD HL,HL
            0x39 => { self.add_hl(self.reg.sp); mmu.tick_m_cycle() }       // ADD HL,SP

            // INC r8
            0x04 => self.reg.b = self.inc(self.reg.b), // INC B
            0x0C => self.reg.c = self.inc(self.reg.c), // INC C
            0x14 => self.reg.d = self.inc(self.reg.d), // INC D
            0x1C => self.reg.e = self.inc(self.reg.e), // INC E
            0x24 => self.reg.h = self.inc(self.reg.h), // INC H
            0x2C => self.reg.l = self.inc(self.reg.l), // INC L
            0x34 => {
                let hl = self.reg.get_hl();
                let value = self.inc(bus_read(mmu, hl));
                bus_write(mmu, hl, value);
            } // INC (HL)
            0x3C => self.reg.a = self.inc(self.reg.a), // INC A

            // DEC r8
            0x05 => self.reg.b = self.dec(self.reg.b), // DEC B
            0x0D => self.reg.c = self.dec(self.reg.c), // DEC C
            0x15 => self.reg.d = self.dec(self.reg.d), // DEC D
            0x1D => self.reg.e = self.dec(self.reg.e), // DEC E
            0x25 => self.reg.h = self.dec(self.reg.h), // DEC H
            0x2D => self.reg.l = self.dec(self.reg.l), // DEC L
            0x35 => {
                let hl = self.reg.get_hl();
                let value = self.dec(bus_read(mmu, hl));
                bus_write(mmu, hl, value);
            } // DEC (HL)
            0x3D => self.reg.a = self.dec(self.reg.a), // DEC A

            // LD r8, imm8
            0x06 => self.reg.b = self.fetch_byte(mmu), // LD B,imm8
            0x0E => self.reg.c = self.fetch_byte(mmu), // LD C,imm8
            0x16 => self.reg.d = self.fetch_byte(mmu), // LD D,imm8
            0x1E => self.reg.e = self.fetch_byte(mmu), // LD E,imm8
            0x26 => self.reg.h = self.fetch_byte(mmu), // LD H,imm8
            0x2E => self.reg.l = self.fetch_byte(mmu), // LD L,imm8
            0x36 => {
                // LD (HL),imm8
                let hl = self.reg.get_hl();
                let imm8 = self.fetch_byte(mmu);
                bus_write(mmu, hl, imm8)
            }
            0x3E => self.reg.a = self.fetch_byte(mmu), // LD A,imm8

            // RLCA
            0x07 => {
                self.reg.a = self.rlc(self.reg.a);
                self.reg.set_flag(ZERO, false);
            } // RLCA

            // RLA
            0x17 => {
                self.reg.a = self.rl(self.reg.a);
                self.reg.set_flag(ZERO, false);
            } // RLA

            // RRCA
            0x0F => {
                self.reg.a = self.rrc(self.reg.a);
                self.reg.set_flag(ZERO, false);
            } // RRCA

            // RRA
            0x1F => {
                self.reg.a = self.rr(self.reg.a);
                self.reg.set_flag(ZERO, false);
            } // RRA

            // DAA
            0x27 => self.daa(), // DAA

            // CPL
            0x2F => self.cpl(), // CPL

            // SCF
            0x37 => self.scf(), // SCF

            // CCF
            0x3F => self.ccf(), // CCF

            // JR cond, imm8
            0x18 => {
                // Unconditional JR: fetch + internal PC-adjust = 3 M-cycles.
                let imm8 = self.fetch_byte(mmu);
                self.jr(imm8);
                mmu.tick_m_cycle(); // internal: apply PC offset
            } // JR imm8
            0x20 => {
                let imm8 = self.fetch_byte(mmu);
                if !self.reg.get_flag(ZERO) {
                    self.jr(imm8);
                    mmu.tick_m_cycle(); // internal: apply PC offset (taken only)
                    cycles.take_conditional();
                }
            } // JR NZ,imm8
            0x28 => {
                let imm8 = self.fetch_byte(mmu);
                if self.reg.get_flag(ZERO) {
                    self.jr(imm8);
                    mmu.tick_m_cycle(); // internal: apply PC offset (taken only)
                    cycles.take_conditional();
                }
            } // JR Z,imm8
            0x30 => {
                let imm8 = self.fetch_byte(mmu);
                if !self.reg.get_flag(CARRY) {
                    self.jr(imm8);
                    mmu.tick_m_cycle(); // internal: apply PC offset (taken only)
                    cycles.take_conditional();
                }
            } // JR NC,imm8
            0x38 => {
                let imm8 = self.fetch_byte(mmu);
                if self.reg.get_flag(CARRY) {
                    self.jr(imm8);
                    mmu.tick_m_cycle(); // internal: apply PC offset (taken only)
                    cycles.take_conditional();
                }
            } // JR C,imm8

            // STOP
            0x10 => {
                let parameter = self.fetch_byte(mmu);
                if parameter != 0 {
                    self.log_stop_speed_switch(parameter);
                }
                self.enter_stop_mode();
            } // STOP

            // LD r8, r8
            0x40 => {}                                             // LD B,B (no op)
            0x41 => self.reg.b = self.reg.c,                       // LD B,C
            0x42 => self.reg.b = self.reg.d,                       // LD B,D
            0x43 => self.reg.b = self.reg.e,                       // LD B,E
            0x44 => self.reg.b = self.reg.h,                       // LD B,H
            0x45 => self.reg.b = self.reg.l,                       // LD B,L
            0x46 => self.reg.b = bus_read(mmu, self.reg.get_hl()), // LD B,(HL)
            0x47 => self.reg.b = self.reg.a,                       // LD B,A

            0x48 => self.reg.c = self.reg.b, // LD C,B
            0x49 => {}                       // LD C,C (no op)
            0x4A => self.reg.c = self.reg.d, // LD C,D
            0x4B => self.reg.c = self.reg.e, // LD C,E
            0x4C => self.reg.c = self.reg.h, // LD C,H
            0x4D => self.reg.c = self.reg.l, // LD C,L
            0x4E => self.reg.c = bus_read(mmu, self.reg.get_hl()), // LD C,(HL)
            0x4F => self.reg.c = self.reg.a, // LD C,A

            0x50 => self.reg.d = self.reg.b, // LD D,B
            0x51 => self.reg.d = self.reg.c, // LD D,C
            0x52 => {}                       // LD D,D (no op)
            0x53 => self.reg.d = self.reg.e, // LD D,E
            0x54 => self.reg.d = self.reg.h, // LD D,H
            0x55 => self.reg.d = self.reg.l, // LD D,L
            0x56 => self.reg.d = bus_read(mmu, self.reg.get_hl()), // LD D,(HL)
            0x57 => self.reg.d = self.reg.a, // LD D,A

            0x58 => self.reg.e = self.reg.b, // LD E,B
            0x59 => self.reg.e = self.reg.c, // LD E,C
            0x5A => self.reg.e = self.reg.d, // LD E,D
            0x5B => {}                       // LD E,E (no op)
            0x5C => self.reg.e = self.reg.h, // LD E,H
            0x5D => self.reg.e = self.reg.l, // LD E,L
            0x5E => self.reg.e = bus_read(mmu, self.reg.get_hl()), // LD E,(HL)
            0x5F => self.reg.e = self.reg.a, // LD E,A

            0x60 => self.reg.h = self.reg.b, // LD H,B
            0x61 => self.reg.h = self.reg.c, // LD H,C
            0x62 => self.reg.h = self.reg.d, // LD H,D
            0x63 => self.reg.h = self.reg.e, // LD H,E
            0x64 => {}                       // LD H,H (no op)
            0x65 => self.reg.h = self.reg.l, // LD H,L
            0x66 => self.reg.h = bus_read(mmu, self.reg.get_hl()), // LD H,(HL)
            0x67 => self.reg.h = self.reg.a, // LD H,A

            0x68 => self.reg.l = self.reg.b, // LD L,B
            0x69 => self.reg.l = self.reg.c, // LD L,C
            0x6A => self.reg.l = self.reg.d, // LD L,D
            0x6B => self.reg.l = self.reg.e, // LD L,E
            0x6C => self.reg.l = self.reg.h, // LD L,H
            0x6D => {}                       // LD L,L (no op)
            0x6E => self.reg.l = bus_read(mmu, self.reg.get_hl()), // LD L,(HL)
            0x6F => self.reg.l = self.reg.a, // LD L,A

            0x70 => bus_write(mmu, self.reg.get_hl(), self.reg.b), // LD (HL),B
            0x71 => bus_write(mmu, self.reg.get_hl(), self.reg.c), // LD (HL),C
            0x72 => bus_write(mmu, self.reg.get_hl(), self.reg.d), // LD (HL),D
            0x73 => bus_write(mmu, self.reg.get_hl(), self.reg.e), // LD (HL),E
            0x74 => bus_write(mmu, self.reg.get_hl(), self.reg.h), // LD (HL),H
            0x75 => bus_write(mmu, self.reg.get_hl(), self.reg.l), // LD (HL),L
            0x77 => bus_write(mmu, self.reg.get_hl(), self.reg.a), // LD (HL),A

            0x78 => self.reg.a = self.reg.b, // LD A,B
            0x79 => self.reg.a = self.reg.c, // LD A,C
            0x7A => self.reg.a = self.reg.d, // LD A,D
            0x7B => self.reg.a = self.reg.e, // LD A,E
            0x7C => self.reg.a = self.reg.h, // LD A,H
            0x7D => self.reg.a = self.reg.l, // LD A,L
            0x7E => self.reg.a = bus_read(mmu, self.reg.get_hl()), // LD A,(HL)
            0x7F => { /* LD A,A – no effect */ } // LD A,A

            // HALT
            0x76 => {
                if !self.ime && self.pending_interrupt_mask(mmu) != 0 {
                    self.trigger_halt_bug();
                } else {
                    self.enter_halt_mode();
                }
            } // HALT

            // ADD A, r8
            0x80 => self.add(self.reg.b), // ADD A,B
            0x81 => self.add(self.reg.c), // ADD A,C
            0x82 => self.add(self.reg.d), // ADD A,D
            0x83 => self.add(self.reg.e), // ADD A,E
            0x84 => self.add(self.reg.h), // ADD A,H
            0x85 => self.add(self.reg.l), // ADD A,L
            0x86 => self.add(bus_read(mmu, self.reg.get_hl())), // ADD A,(HL)
            0x87 => self.add(self.reg.a), // ADD A,A

            // ADD A, imm8
            0xC6 => {
                let imm8 = self.fetch_byte(mmu);
                self.add(imm8)
            } // ADD A,imm8

            // ADC A, r8
            0x88 => self.adc(self.reg.b), // ADC A,B
            0x89 => self.adc(self.reg.c), // ADC A,C
            0x8A => self.adc(self.reg.d), // ADC A,D
            0x8B => self.adc(self.reg.e), // ADC A,E
            0x8C => self.adc(self.reg.h), // ADC A,H
            0x8D => self.adc(self.reg.l), // ADC A,L
            0x8E => self.adc(bus_read(mmu, self.reg.get_hl())), // ADC A,(HL)
            0x8F => self.adc(self.reg.a), // ADC A,A

            // ADC A, imm8
            0xCE => {
                let imm8 = self.fetch_byte(mmu);
                self.adc(imm8)
            } // ADC A,imm8

            // SUB A, r8
            0x90 => self.sub(self.reg.b),                       // SUB B
            0x91 => self.sub(self.reg.c),                       // SUB C
            0x92 => self.sub(self.reg.d),                       // SUB D
            0x93 => self.sub(self.reg.e),                       // SUB E
            0x94 => self.sub(self.reg.h),                       // SUB H
            0x95 => self.sub(self.reg.l),                       // SUB L
            0x96 => self.sub(bus_read(mmu, self.reg.get_hl())), // SUB (HL)
            0x97 => self.sub(self.reg.a),                       // SUB A

            // SUB A, imm8
            0xD6 => {
                let imm8 = self.fetch_byte(mmu);
                self.sub(imm8)
            } // SUB A,imm8

            // SBC A, r8
            0x98 => self.sbc(self.reg.b), // SBC A,B
            0x99 => self.sbc(self.reg.c), // SBC A,C
            0x9A => self.sbc(self.reg.d), // SBC A,D
            0x9B => self.sbc(self.reg.e), // SBC A,E
            0x9C => self.sbc(self.reg.h), // SBC A,H
            0x9D => self.sbc(self.reg.l), // SBC A,L
            0x9E => self.sbc(bus_read(mmu, self.reg.get_hl())), // SBC A,(HL)
            0x9F => self.sbc(self.reg.a), // SBC A,A

            // SBC A, imm8
            0xDE => {
                let imm8 = self.fetch_byte(mmu);
                self.sbc(imm8)
            } // SBC A,imm8

            // AND A, r8
            0xA0 => self.and(self.reg.b),                       // AND B
            0xA1 => self.and(self.reg.c),                       // AND C
            0xA2 => self.and(self.reg.d),                       // AND D
            0xA3 => self.and(self.reg.e),                       // AND E
            0xA4 => self.and(self.reg.h),                       // AND H
            0xA5 => self.and(self.reg.l),                       // AND L
            0xA6 => self.and(bus_read(mmu, self.reg.get_hl())), // AND (HL)
            0xA7 => self.and(self.reg.a),                       // AND A

            // AND A, imm8
            0xE6 => {
                let imm8 = self.fetch_byte(mmu);
                self.and(imm8)
            } // AND A,imm8

            // XOR A, r8
            0xA8 => self.xor(self.reg.b),                       // XOR B
            0xA9 => self.xor(self.reg.c),                       // XOR C
            0xAA => self.xor(self.reg.d),                       // XOR D
            0xAB => self.xor(self.reg.e),                       // XOR E
            0xAC => self.xor(self.reg.h),                       // XOR H
            0xAD => self.xor(self.reg.l),                       // XOR L
            0xAE => self.xor(bus_read(mmu, self.reg.get_hl())), // XOR (HL)
            0xAF => self.xor(self.reg.a),                       // XOR A

            // XOR A, imm8
            0xEE => {
                let imm8 = self.fetch_byte(mmu);
                self.xor(imm8)
            } // XOR A,imm8

            // OR A, r8
            0xB0 => self.or(self.reg.b),                       // OR B
            0xB1 => self.or(self.reg.c),                       // OR C
            0xB2 => self.or(self.reg.d),                       // OR D
            0xB3 => self.or(self.reg.e),                       // OR E
            0xB4 => self.or(self.reg.h),                       // OR H
            0xB5 => self.or(self.reg.l),                       // OR L
            0xB6 => self.or(bus_read(mmu, self.reg.get_hl())), // OR (HL)
            0xB7 => self.or(self.reg.a),                       // OR A

            // OR A, imm8
            0xF6 => {
                let imm8 = self.fetch_byte(mmu);
                self.or(imm8)
            } // OR A,imm8

            // CP A, r8
            0xB8 => self.cp(self.reg.b),                       // CP B
            0xB9 => self.cp(self.reg.c),                       // CP C
            0xBA => self.cp(self.reg.d),                       // CP D
            0xBB => self.cp(self.reg.e),                       // CP E
            0xBC => self.cp(self.reg.h),                       // CP H
            0xBD => self.cp(self.reg.l),                       // CP L
            0xBE => self.cp(bus_read(mmu, self.reg.get_hl())), // CP (HL)
            0xBF => self.cp(self.reg.a),                       // CP A

            // CP A, imm8
            0xFE => {
                let imm8 = self.fetch_byte(mmu);
                self.cp(imm8)
            } // CP A,imm8

            // RET cond
            //
            // Hardware timing: M1 = fetch, M2 = check condition (internal).
            // If taken: M3 = pop lo, M4 = pop hi, M5 = set PC (internal).
            // Not taken: 2 M-cycles total (fetch + condition check).
            0xC0 => {
                mmu.tick_m_cycle(); // M2: condition check (always)
                if !self.reg.get_flag(ZERO) {
                    let addr = self.pop(mmu); // M3, M4
                    mmu.tick_m_cycle(); // M5: load PC
                    self.reg.pc = addr;
                    cycles.take_conditional();
                }
            } // RET NZ
            0xC8 => {
                mmu.tick_m_cycle(); // M2: condition check (always)
                if self.reg.get_flag(ZERO) {
                    let addr = self.pop(mmu); // M3, M4
                    mmu.tick_m_cycle(); // M5: load PC
                    self.reg.pc = addr;
                    cycles.take_conditional();
                }
            } // RET Z
            0xD0 => {
                mmu.tick_m_cycle(); // M2: condition check (always)
                if !self.reg.get_flag(CARRY) {
                    let addr = self.pop(mmu); // M3, M4
                    mmu.tick_m_cycle(); // M5: load PC
                    self.reg.pc = addr;
                    cycles.take_conditional();
                }
            } // RET NC
            0xD8 => {
                mmu.tick_m_cycle(); // M2: condition check (always)
                if self.reg.get_flag(CARRY) {
                    let addr = self.pop(mmu); // M3, M4
                    mmu.tick_m_cycle(); // M5: load PC
                    self.reg.pc = addr;
                    cycles.take_conditional();
                }
            } // RET C

            // RET — 16 cycles: fetch + pop lo + pop hi + internal (load PC).
            0xC9 => {
                let addr = self.pop(mmu);
                mmu.tick_m_cycle(); // internal: load PC
                self.reg.pc = addr;
            } // RET

            // RETI — same timing as RET, plus re-enable IME.
            0xD9 => {
                let addr = self.pop(mmu);
                mmu.tick_m_cycle(); // internal: load PC
                self.reg.pc = addr;
                self.ime = true;
                self.ime_scheduled = None;
            } // RETI

            // JP cond, imm16
            //
            // Not taken: fetch + fetch lo + fetch hi = 3 M-cycles (12 cycles).
            // Taken: + 1 internal for PC load = 4 M-cycles (16 cycles).
            0xC2 => {
                let imm16 = self.fetch_word(mmu);
                if !self.reg.get_flag(ZERO) {
                    mmu.tick_m_cycle(); // internal: load PC
                    self.reg.pc = imm16;
                    cycles.take_conditional();
                }
            } // JP NZ,imm16
            0xCA => {
                let imm16 = self.fetch_word(mmu);
                if self.reg.get_flag(ZERO) {
                    mmu.tick_m_cycle(); // internal: load PC
                    self.reg.pc = imm16;
                    cycles.take_conditional();
                }
            } // JP Z,imm16
            0xD2 => {
                let imm16 = self.fetch_word(mmu);
                if !self.reg.get_flag(CARRY) {
                    mmu.tick_m_cycle(); // internal: load PC
                    self.reg.pc = imm16;
                    cycles.take_conditional();
                }
            } // JP NC,imm16
            0xDA => {
                let imm16 = self.fetch_word(mmu);
                if self.reg.get_flag(CARRY) {
                    mmu.tick_m_cycle(); // internal: load PC
                    self.reg.pc = imm16;
                    cycles.take_conditional();
                }
            } // JP C,imm16

            // JP imm16 — 16 cycles: fetch + fetch lo + fetch hi + internal (load PC).
            0xC3 => {
                let imm16 = self.fetch_word(mmu);
                mmu.tick_m_cycle(); // internal: load PC
                self.reg.pc = imm16;
            } // JP imm16

            // JP HL — 4 cycles: just the opcode fetch (PC already set to HL).
            0xE9 => self.reg.pc = self.reg.get_hl(), // JP HL

            // CALL cond, imm16
            //
            // Not taken: fetch + fetch lo + fetch hi = 3 M-cycles (12 cycles).
            // Taken: + 1 internal (SP adjust) + push hi + push lo = 6 M-cycles (24 cycles).
            0xC4 => {
                let imm16 = self.fetch_word(mmu);
                if !self.reg.get_flag(ZERO) {
                    mmu.tick_m_cycle(); // internal: SP adjust before push
                    self.call(mmu, imm16);
                    cycles.take_conditional();
                }
            } // CALL NZ,imm16
            0xCC => {
                let imm16 = self.fetch_word(mmu);
                if self.reg.get_flag(ZERO) {
                    mmu.tick_m_cycle(); // internal: SP adjust before push
                    self.call(mmu, imm16);
                    cycles.take_conditional();
                }
            } // CALL Z,imm16
            0xD4 => {
                let imm16 = self.fetch_word(mmu);
                if !self.reg.get_flag(CARRY) {
                    mmu.tick_m_cycle(); // internal: SP adjust before push
                    self.call(mmu, imm16);
                    cycles.take_conditional();
                }
            } // CALL NC,imm16
            0xDC => {
                let imm16 = self.fetch_word(mmu);
                if self.reg.get_flag(CARRY) {
                    mmu.tick_m_cycle(); // internal: SP adjust before push
                    self.call(mmu, imm16);
                    cycles.take_conditional();
                }
            } // CALL C,imm16

            // CALL imm16 — 24 cycles: fetch + fetch lo + fetch hi + internal + push hi + push lo.
            0xCD => {
                let imm16 = self.fetch_word(mmu);
                mmu.tick_m_cycle(); // internal: SP adjust before push
                self.call(mmu, imm16)
            } // CALL imm16

            // RST target — 16 cycles: fetch + internal (SP adjust) + push hi + push lo.
            0xC7 => { mmu.tick_m_cycle(); self.rst(mmu, 0x00) } // RST 00H
            0xCF => { mmu.tick_m_cycle(); self.rst(mmu, 0x08) } // RST 08H
            0xD7 => { mmu.tick_m_cycle(); self.rst(mmu, 0x10) } // RST 10H
            0xDF => { mmu.tick_m_cycle(); self.rst(mmu, 0x18) } // RST 18H
            0xE7 => { mmu.tick_m_cycle(); self.rst(mmu, 0x20) } // RST 20H
            0xEF => { mmu.tick_m_cycle(); self.rst(mmu, 0x28) } // RST 28H
            0xF7 => { mmu.tick_m_cycle(); self.rst(mmu, 0x30) } // RST 30H
            0xFF => { mmu.tick_m_cycle(); self.rst(mmu, 0x38) } // RST 38H

            // POP r16 — 12 cycles: fetch + pop lo + pop hi (no internal tick needed).
            0xC1 => {
                let bc = self.pop(mmu);
                self.reg.set_bc(bc)
            } // POP BC
            0xD1 => {
                let de = self.pop(mmu);
                self.reg.set_de(de)
            } // POP DE
            0xE1 => {
                let hl = self.pop(mmu);
                self.reg.set_hl(hl)
            } // POP HL
            0xF1 => {
                let af = self.pop(mmu);
                self.reg.set_af(af)
            } // POP AF

            // PUSH r16 — 16 cycles: fetch + internal (SP adjust) + push hi + push lo.
            0xC5 => { mmu.tick_m_cycle(); self.push(mmu, self.reg.get_bc()) } // PUSH BC
            0xD5 => { mmu.tick_m_cycle(); self.push(mmu, self.reg.get_de()) } // PUSH DE
            0xE5 => { mmu.tick_m_cycle(); self.push(mmu, self.reg.get_hl()) } // PUSH HL
            0xF5 => { mmu.tick_m_cycle(); self.push(mmu, self.reg.get_af()) } // PUSH AF

            // LDH (C), A
            0xE2 => bus_write(mmu, 0xFF00 + self.reg.c as u16, self.reg.a), // LDH (C),A

            // LDH (imm8), A
            0xE0 => {
                let imm8 = self.fetch_byte(mmu);
                bus_write(mmu, 0xFF00 + imm8 as u16, self.reg.a)
            } // LDH (imm8),A

            // LD (imm16), A
            0xEA => {
                let imm16 = self.fetch_word(mmu);
                bus_write(mmu, imm16, self.reg.a)
            } // LD (imm16),A

            // LDH A, (C)
            0xF2 => self.reg.a = bus_read(mmu, 0xFF00 + self.reg.c as u16), // LDH A,(C)

            // LDH A, (imm8)
            0xF0 => {
                let imm8 = self.fetch_byte(mmu);
                self.reg.a = bus_read(mmu, 0xFF00 + imm8 as u16)
            } // LDH A,(imm8)

            // LD A, (imm16)
            0xFA => {
                let imm16 = self.fetch_word(mmu);
                self.reg.a = bus_read(mmu, imm16);
            } // LD A,(imm16)

            // ADD SP, imm8 — 16 cycles: fetch opcode + fetch imm8 + 2 internal (ALU).
            0xE8 => {
                let imm8 = self.fetch_byte(mmu);
                mmu.tick_m_cycle(); // internal: ALU cycle 1
                mmu.tick_m_cycle(); // internal: ALU cycle 2
                self.add_sp(imm8)
            } // ADD SP,imm8

            // LD HL, SP + imm8 — 12 cycles: fetch opcode + fetch imm8 + 1 internal (ALU).
            0xF8 => {
                let n = self.fetch_byte(mmu);
                mmu.tick_m_cycle(); // internal: ALU
                let sp = self.reg.sp;
                let n = n as i8 as i16 as u16;

                let result = sp.wrapping_add(n);

                self.reg.set_flag(ZERO, false);
                self.reg.set_flag(SUBTRACT, false);

                self.reg
                    .set_flag(HALF_CARRY, (sp & 0x000F) + (n & 0x000F) > 0x000F);
                self.reg
                    .set_flag(CARRY, (sp & 0x00FF) + (n & 0x00FF) > 0x00FF);

                self.reg.set_hl(result);
            } // LD HL,SP+imm8

            // LD SP, HL — 8 cycles: opcode fetch + 1 internal (16-bit register copy).
            0xF9 => { self.reg.sp = self.reg.get_hl(); mmu.tick_m_cycle() } // LD SP,HL

            // DI
            0xF3 => {
                self.ime = false;
                self.ime_scheduled = None;
            } // DI

            // EI
            0xFB => {
                self.ime_scheduled = Some(1);
            } // EI

            // CB PREFIX
            0xCB => {
                let opcode = self.fetch_byte(mmu);
                cycles.set_cost(cb_cycle_cost(opcode));
                match opcode {
                    // RLC r8
                    0x00 => self.reg.b = self.rlc(self.reg.b), // RLC B
                    0x01 => self.reg.c = self.rlc(self.reg.c), // RLC C
                    0x02 => self.reg.d = self.rlc(self.reg.d), // RLC D
                    0x03 => self.reg.e = self.rlc(self.reg.e), // RLC E
                    0x04 => self.reg.h = self.rlc(self.reg.h), // RLC H
                    0x05 => self.reg.l = self.rlc(self.reg.l), // RLC L
                    0x06 => {
                        let hl = self.reg.get_hl();
                        let value = self.rlc(bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // RLC (HL)
                    0x07 => self.reg.a = self.rlc(self.reg.a), // RLC A

                    // RRC r8
                    0x08 => self.reg.b = self.rrc(self.reg.b), // RRC B
                    0x09 => self.reg.c = self.rrc(self.reg.c), // RRC C
                    0x0A => self.reg.d = self.rrc(self.reg.d), // RRC D
                    0x0B => self.reg.e = self.rrc(self.reg.e), // RRC E
                    0x0C => self.reg.h = self.rrc(self.reg.h), // RRC H
                    0x0D => self.reg.l = self.rrc(self.reg.l), // RRC L
                    0x0E => {
                        let hl = self.reg.get_hl();
                        let value = self.rrc(bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // RRC (HL)
                    0x0F => self.reg.a = self.rrc(self.reg.a), // RRC A

                    // RL r8
                    0x10 => self.reg.b = self.rl(self.reg.b), // RL B
                    0x11 => self.reg.c = self.rl(self.reg.c), // RL C
                    0x12 => self.reg.d = self.rl(self.reg.d), // RL D
                    0x13 => self.reg.e = self.rl(self.reg.e), // RL E
                    0x14 => self.reg.h = self.rl(self.reg.h), // RL H
                    0x15 => self.reg.l = self.rl(self.reg.l), // RL L
                    0x16 => {
                        let hl = self.reg.get_hl();
                        let value = self.rl(bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // RL (HL)
                    0x17 => self.reg.a = self.rl(self.reg.a), // RL A

                    // RR r8
                    0x18 => self.reg.b = self.rr(self.reg.b), // RR B
                    0x19 => self.reg.c = self.rr(self.reg.c), // RR C
                    0x1A => self.reg.d = self.rr(self.reg.d), // RR D
                    0x1B => self.reg.e = self.rr(self.reg.e), // RR E
                    0x1C => self.reg.h = self.rr(self.reg.h), // RR H
                    0x1D => self.reg.l = self.rr(self.reg.l), // RR L
                    0x1E => {
                        let hl = self.reg.get_hl();
                        let value = self.rr(bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // RR (HL)
                    0x1F => self.reg.a = self.rr(self.reg.a), // RR A

                    // SLA r8
                    0x20 => self.reg.b = self.sla(self.reg.b), // SLA B
                    0x21 => self.reg.c = self.sla(self.reg.c), // SLA C
                    0x22 => self.reg.d = self.sla(self.reg.d), // SLA D
                    0x23 => self.reg.e = self.sla(self.reg.e), // SLA E
                    0x24 => self.reg.h = self.sla(self.reg.h), // SLA H
                    0x25 => self.reg.l = self.sla(self.reg.l), // SLA L
                    0x26 => {
                        let hl = self.reg.get_hl();
                        let value = self.sla(bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // SLA (HL)
                    0x27 => self.reg.a = self.sla(self.reg.a), // SLA A

                    // SRA r8
                    0x28 => self.reg.b = self.sra(self.reg.b), // SRA B
                    0x29 => self.reg.c = self.sra(self.reg.c), // SRA C
                    0x2A => self.reg.d = self.sra(self.reg.d), // SRA D
                    0x2B => self.reg.e = self.sra(self.reg.e), // SRA E
                    0x2C => self.reg.h = self.sra(self.reg.h), // SRA H
                    0x2D => self.reg.l = self.sra(self.reg.l), // SRA L
                    0x2E => {
                        let hl = self.reg.get_hl();
                        let value = self.sra(bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // SRA (HL)
                    0x2F => self.reg.a = self.sra(self.reg.a), // SRA A

                    // SWAP r8
                    0x30 => self.reg.b = self.swap(self.reg.b), // SWAP B
                    0x31 => self.reg.c = self.swap(self.reg.c), // SWAP C
                    0x32 => self.reg.d = self.swap(self.reg.d), // SWAP D
                    0x33 => self.reg.e = self.swap(self.reg.e), // SWAP E
                    0x34 => self.reg.h = self.swap(self.reg.h), // SWAP H
                    0x35 => self.reg.l = self.swap(self.reg.l), // SWAP L
                    0x36 => {
                        let hl = self.reg.get_hl();
                        let value = self.swap(bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // SWAP (HL)
                    0x37 => self.reg.a = self.swap(self.reg.a), // SWAP A

                    // SRL r8
                    0x38 => self.reg.b = self.srl(self.reg.b), // SRL B
                    0x39 => self.reg.c = self.srl(self.reg.c), // SRL C
                    0x3A => self.reg.d = self.srl(self.reg.d), // SRL D
                    0x3B => self.reg.e = self.srl(self.reg.e), // SRL E
                    0x3C => self.reg.h = self.srl(self.reg.h), // SRL H
                    0x3D => self.reg.l = self.srl(self.reg.l), // SRL L
                    0x3E => {
                        let hl = self.reg.get_hl();
                        let value = self.srl(bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // SRL (HL)
                    0x3F => self.reg.a = self.srl(self.reg.a), // SRL A

                    // BIT b, r8
                    0x40 => self.bit(0, self.reg.b), // BIT 0,B
                    0x41 => self.bit(0, self.reg.c), // BIT 0,C
                    0x42 => self.bit(0, self.reg.d), // BIT 0,D
                    0x43 => self.bit(0, self.reg.e), // BIT 0,E
                    0x44 => self.bit(0, self.reg.h), // BIT 0,H
                    0x45 => self.bit(0, self.reg.l), // BIT 0,L
                    0x46 => self.bit(0, bus_read(mmu, self.reg.get_hl())), // BIT 0,(HL)
                    0x47 => self.bit(0, self.reg.a), // BIT 0,A

                    0x48 => self.bit(1, self.reg.b), // BIT 1,B
                    0x49 => self.bit(1, self.reg.c), // BIT 1,C
                    0x4A => self.bit(1, self.reg.d), // BIT 1,D
                    0x4B => self.bit(1, self.reg.e), // BIT 1,E
                    0x4C => self.bit(1, self.reg.h), // BIT 1,H
                    0x4D => self.bit(1, self.reg.l), // BIT 1,L
                    0x4E => self.bit(1, bus_read(mmu, self.reg.get_hl())), // BIT 1,(HL)
                    0x4F => self.bit(1, self.reg.a), // BIT 1,A

                    0x50 => self.bit(2, self.reg.b), // BIT 2,B
                    0x51 => self.bit(2, self.reg.c), // BIT 2,C
                    0x52 => self.bit(2, self.reg.d), // BIT 2,D
                    0x53 => self.bit(2, self.reg.e), // BIT 2,E
                    0x54 => self.bit(2, self.reg.h), // BIT 2,H
                    0x55 => self.bit(2, self.reg.l), // BIT 2,L
                    0x56 => self.bit(2, bus_read(mmu, self.reg.get_hl())), // BIT 2,(HL)
                    0x57 => self.bit(2, self.reg.a), // BIT 2,A

                    0x58 => self.bit(3, self.reg.b), // BIT 3,B
                    0x59 => self.bit(3, self.reg.c), // BIT 3,C
                    0x5A => self.bit(3, self.reg.d), // BIT 3,D
                    0x5B => self.bit(3, self.reg.e), // BIT 3,E
                    0x5C => self.bit(3, self.reg.h), // BIT 3,H
                    0x5D => self.bit(3, self.reg.l), // BIT 3,L
                    0x5E => self.bit(3, bus_read(mmu, self.reg.get_hl())), // BIT 3,(HL)
                    0x5F => self.bit(3, self.reg.a), // BIT 3,A

                    0x60 => self.bit(4, self.reg.b), // BIT 4,B
                    0x61 => self.bit(4, self.reg.c), // BIT 4,C
                    0x62 => self.bit(4, self.reg.d), // BIT 4,D
                    0x63 => self.bit(4, self.reg.e), // BIT 4,E
                    0x64 => self.bit(4, self.reg.h), // BIT 4,H
                    0x65 => self.bit(4, self.reg.l), // BIT 4,L
                    0x66 => self.bit(4, bus_read(mmu, self.reg.get_hl())), // BIT 4,(HL)
                    0x67 => self.bit(4, self.reg.a), // BIT 4,A

                    0x68 => self.bit(5, self.reg.b), // BIT 5,B
                    0x69 => self.bit(5, self.reg.c), // BIT 5,C
                    0x6A => self.bit(5, self.reg.d), // BIT 5,D
                    0x6B => self.bit(5, self.reg.e), // BIT 5,E
                    0x6C => self.bit(5, self.reg.h), // BIT 5,H
                    0x6D => self.bit(5, self.reg.l), // BIT 5,L
                    0x6E => self.bit(5, bus_read(mmu, self.reg.get_hl())), // BIT 5,(HL)
                    0x6F => self.bit(5, self.reg.a), // BIT 5,A

                    0x70 => self.bit(6, self.reg.b), // BIT 6,B
                    0x71 => self.bit(6, self.reg.c), // BIT 6,C
                    0x72 => self.bit(6, self.reg.d), // BIT 6,D
                    0x73 => self.bit(6, self.reg.e), // BIT 6,E
                    0x74 => self.bit(6, self.reg.h), // BIT 6,H
                    0x75 => self.bit(6, self.reg.l), // BIT 6,L
                    0x76 => self.bit(6, bus_read(mmu, self.reg.get_hl())), // BIT 6,(HL)
                    0x77 => self.bit(6, self.reg.a), // BIT 6,A

                    0x78 => self.bit(7, self.reg.b), // BIT 7,B
                    0x79 => self.bit(7, self.reg.c), // BIT 7,C
                    0x7A => self.bit(7, self.reg.d), // BIT 7,D
                    0x7B => self.bit(7, self.reg.e), // BIT 7,E
                    0x7C => self.bit(7, self.reg.h), // BIT 7,H
                    0x7D => self.bit(7, self.reg.l), // BIT 7,L
                    0x7E => self.bit(7, bus_read(mmu, self.reg.get_hl())), // BIT 7,(HL)
                    0x7F => self.bit(7, self.reg.a), // BIT 7,A

                    // RES b, r8
                    0x80 => self.reg.b = self.res(0, self.reg.b), // RES 0,B
                    0x81 => self.reg.c = self.res(0, self.reg.c), // RES 0,C
                    0x82 => self.reg.d = self.res(0, self.reg.d), // RES 0,D
                    0x83 => self.reg.e = self.res(0, self.reg.e), // RES 0,E
                    0x84 => self.reg.h = self.res(0, self.reg.h), // RES 0,H
                    0x85 => self.reg.l = self.res(0, self.reg.l), // RES 0,L
                    0x86 => {
                        let hl = self.reg.get_hl();
                        let value = self.res(0, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // RES 0,(HL)
                    0x87 => self.reg.a = self.res(0, self.reg.a), // RES 0,A

                    0x88 => self.reg.b = self.res(1, self.reg.b), // RES 1,B
                    0x89 => self.reg.c = self.res(1, self.reg.c), // RES 1,C
                    0x8A => self.reg.d = self.res(1, self.reg.d), // RES 1,D
                    0x8B => self.reg.e = self.res(1, self.reg.e), // RES 1,E
                    0x8C => self.reg.h = self.res(1, self.reg.h), // RES 1,H
                    0x8D => self.reg.l = self.res(1, self.reg.l), // RES 1,L
                    0x8E => {
                        let hl = self.reg.get_hl();
                        let value = self.res(1, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // RES 1,(HL)
                    0x8F => self.reg.a = self.res(1, self.reg.a), // RES 1,A

                    0x90 => self.reg.b = self.res(2, self.reg.b), // RES 2,B
                    0x91 => self.reg.c = self.res(2, self.reg.c), // RES 2,C
                    0x92 => self.reg.d = self.res(2, self.reg.d), // RES 2,D
                    0x93 => self.reg.e = self.res(2, self.reg.e), // RES 2,E
                    0x94 => self.reg.h = self.res(2, self.reg.h), // RES 2,H
                    0x95 => self.reg.l = self.res(2, self.reg.l), // RES 2,L
                    0x96 => {
                        let hl = self.reg.get_hl();
                        let value = self.res(2, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // RES 2,(HL)
                    0x97 => self.reg.a = self.res(2, self.reg.a), // RES 2,A

                    0x98 => self.reg.b = self.res(3, self.reg.b), // RES 3,B
                    0x99 => self.reg.c = self.res(3, self.reg.c), // RES 3,C
                    0x9A => self.reg.d = self.res(3, self.reg.d), // RES 3,D
                    0x9B => self.reg.e = self.res(3, self.reg.e), // RES 3,E
                    0x9C => self.reg.h = self.res(3, self.reg.h), // RES 3,H
                    0x9D => self.reg.l = self.res(3, self.reg.l), // RES 3,L
                    0x9E => {
                        let hl = self.reg.get_hl();
                        let value = self.res(3, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // RES 3,(HL)
                    0x9F => self.reg.a = self.res(3, self.reg.a), // RES 3,A

                    0xA0 => self.reg.b = self.res(4, self.reg.b), // RES 4,B
                    0xA1 => self.reg.c = self.res(4, self.reg.c), // RES 4,C
                    0xA2 => self.reg.d = self.res(4, self.reg.d), // RES 4,D
                    0xA3 => self.reg.e = self.res(4, self.reg.e), // RES 4,E
                    0xA4 => self.reg.h = self.res(4, self.reg.h), // RES 4,H
                    0xA5 => self.reg.l = self.res(4, self.reg.l), // RES 4,L
                    0xA6 => {
                        let hl = self.reg.get_hl();
                        let value = self.res(4, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // RES 4,(HL)
                    0xA7 => self.reg.a = self.res(4, self.reg.a), // RES 4,A

                    0xA8 => self.reg.b = self.res(5, self.reg.b), // RES 5,B
                    0xA9 => self.reg.c = self.res(5, self.reg.c), // RES 5,C
                    0xAA => self.reg.d = self.res(5, self.reg.d), // RES 5,D
                    0xAB => self.reg.e = self.res(5, self.reg.e), // RES 5,E
                    0xAC => self.reg.h = self.res(5, self.reg.h), // RES 5,H
                    0xAD => self.reg.l = self.res(5, self.reg.l), // RES 5,L
                    0xAE => {
                        let hl = self.reg.get_hl();
                        let value = self.res(5, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // RES 5,(HL)
                    0xAF => self.reg.a = self.res(5, self.reg.a), // RES 5,A

                    0xB0 => self.reg.b = self.res(6, self.reg.b), // RES 6,B
                    0xB1 => self.reg.c = self.res(6, self.reg.c), // RES 6,C
                    0xB2 => self.reg.d = self.res(6, self.reg.d), // RES 6,D
                    0xB3 => self.reg.e = self.res(6, self.reg.e), // RES 6,E
                    0xB4 => self.reg.h = self.res(6, self.reg.h), // RES 6,H
                    0xB5 => self.reg.l = self.res(6, self.reg.l), // RES 6,L
                    0xB6 => {
                        let hl = self.reg.get_hl();
                        let value = self.res(6, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // RES 6,(HL)
                    0xB7 => self.reg.a = self.res(6, self.reg.a), // RES 6,A

                    0xB8 => self.reg.b = self.res(7, self.reg.b), // RES 7,B
                    0xB9 => self.reg.c = self.res(7, self.reg.c), // RES 7,C
                    0xBA => self.reg.d = self.res(7, self.reg.d), // RES 7,D
                    0xBB => self.reg.e = self.res(7, self.reg.e), // RES 7,E
                    0xBC => self.reg.h = self.res(7, self.reg.h), // RES 7,H
                    0xBD => self.reg.l = self.res(7, self.reg.l), // RES 7,L
                    0xBE => {
                        let hl = self.reg.get_hl();
                        let value = self.res(7, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // RES 7,(HL)
                    0xBF => self.reg.a = self.res(7, self.reg.a), // RES 7,A

                    // SET b, r8
                    0xC0 => self.reg.b = self.set(0, self.reg.b), // SET 0,B
                    0xC1 => self.reg.c = self.set(0, self.reg.c), // SET 0,C
                    0xC2 => self.reg.d = self.set(0, self.reg.d), // SET 0,D
                    0xC3 => self.reg.e = self.set(0, self.reg.e), // SET 0,E
                    0xC4 => self.reg.h = self.set(0, self.reg.h), // SET 0,H
                    0xC5 => self.reg.l = self.set(0, self.reg.l), // SET 0,L
                    0xC6 => {
                        let hl = self.reg.get_hl();
                        let value = self.set(0, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // SET 0,(HL)
                    0xC7 => self.reg.a = self.set(0, self.reg.a), // SET 0,A

                    0xC8 => self.reg.b = self.set(1, self.reg.b), // SET 1,B
                    0xC9 => self.reg.c = self.set(1, self.reg.c), // SET 1,C
                    0xCA => self.reg.d = self.set(1, self.reg.d), // SET 1,D
                    0xCB => self.reg.e = self.set(1, self.reg.e), // SET 1,E
                    0xCC => self.reg.h = self.set(1, self.reg.h), // SET 1,H
                    0xCD => self.reg.l = self.set(1, self.reg.l), // SET 1,L
                    0xCE => {
                        let hl = self.reg.get_hl();
                        let value = self.set(1, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // SET 1,(HL)
                    0xCF => self.reg.a = self.set(1, self.reg.a), // SET 1,A

                    0xD0 => self.reg.b = self.set(2, self.reg.b), // SET 2,B
                    0xD1 => self.reg.c = self.set(2, self.reg.c), // SET 2,C
                    0xD2 => self.reg.d = self.set(2, self.reg.d), // SET 2,D
                    0xD3 => self.reg.e = self.set(2, self.reg.e), // SET 2,E
                    0xD4 => self.reg.h = self.set(2, self.reg.h), // SET 2,H
                    0xD5 => self.reg.l = self.set(2, self.reg.l), // SET 2,L
                    0xD6 => {
                        let hl = self.reg.get_hl();
                        let value = self.set(2, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // SET 2,(HL)
                    0xD7 => self.reg.a = self.set(2, self.reg.a), // SET 2,A

                    0xD8 => self.reg.b = self.set(3, self.reg.b), // SET 3,B
                    0xD9 => self.reg.c = self.set(3, self.reg.c), // SET 3,C
                    0xDA => self.reg.d = self.set(3, self.reg.d), // SET 3,D
                    0xDB => self.reg.e = self.set(3, self.reg.e), // SET 3,E
                    0xDC => self.reg.h = self.set(3, self.reg.h), // SET 3,H
                    0xDD => self.reg.l = self.set(3, self.reg.l), // SET 3,L
                    0xDE => {
                        let hl = self.reg.get_hl();
                        let value = self.set(3, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // SET 3,(HL)
                    0xDF => self.reg.a = self.set(3, self.reg.a), // SET 3,A

                    0xE0 => self.reg.b = self.set(4, self.reg.b), // SET 4,B
                    0xE1 => self.reg.c = self.set(4, self.reg.c), // SET 4,C
                    0xE2 => self.reg.d = self.set(4, self.reg.d), // SET 4,D
                    0xE3 => self.reg.e = self.set(4, self.reg.e), // SET 4,E
                    0xE4 => self.reg.h = self.set(4, self.reg.h), // SET 4,H
                    0xE5 => self.reg.l = self.set(4, self.reg.l), // SET 4,L
                    0xE6 => {
                        let hl = self.reg.get_hl();
                        let value = self.set(4, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // SET 4,(HL)
                    0xE7 => self.reg.a = self.set(4, self.reg.a), // SET 4,A

                    0xE8 => self.reg.b = self.set(5, self.reg.b), // SET 5,B
                    0xE9 => self.reg.c = self.set(5, self.reg.c), // SET 5,C
                    0xEA => self.reg.d = self.set(5, self.reg.d), // SET 5,D
                    0xEB => self.reg.e = self.set(5, self.reg.e), // SET 5,E
                    0xEC => self.reg.h = self.set(5, self.reg.h), // SET 5,H
                    0xED => self.reg.l = self.set(5, self.reg.l), // SET 5,L
                    0xEE => {
                        let hl = self.reg.get_hl();
                        let value = self.set(5, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // SET 5,(HL)
                    0xEF => self.reg.a = self.set(5, self.reg.a), // SET 5,A

                    0xF0 => self.reg.b = self.set(6, self.reg.b), // SET 6,B
                    0xF1 => self.reg.c = self.set(6, self.reg.c), // SET 6,C
                    0xF2 => self.reg.d = self.set(6, self.reg.d), // SET 6,D
                    0xF3 => self.reg.e = self.set(6, self.reg.e), // SET 6,E
                    0xF4 => self.reg.h = self.set(6, self.reg.h), // SET 6,H
                    0xF5 => self.reg.l = self.set(6, self.reg.l), // SET 6,L
                    0xF6 => {
                        let hl = self.reg.get_hl();
                        let value = self.set(6, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // SET 6,(HL)
                    0xF7 => self.reg.a = self.set(6, self.reg.a), // SET 6,A

                    0xF8 => self.reg.b = self.set(7, self.reg.b), // SET 7,B
                    0xF9 => self.reg.c = self.set(7, self.reg.c), // SET 7,C
                    0xFA => self.reg.d = self.set(7, self.reg.d), // SET 7,D
                    0xFB => self.reg.e = self.set(7, self.reg.e), // SET 7,E
                    0xFC => self.reg.h = self.set(7, self.reg.h), // SET 7,H
                    0xFD => self.reg.l = self.set(7, self.reg.l), // SET 7,L
                    0xFE => {
                        let hl = self.reg.get_hl();
                        let value = self.set(7, bus_read(mmu, hl));
                        bus_write(mmu, hl, value);
                    } // SET 7,(HL)
                    0xFF => self.reg.a = self.set(7, self.reg.a), // SET 7,A
                }
            }

            _ => {
                // Undocumented opcodes act as NOPs on the DMG.
                self.log_undocumented(opcode);
            }
        }
    }

    fn step_halt_state(&mut self, mmu: &mut impl MemoryBus) -> Option<u8> {
        match self.halt_state {
            HaltState::Running => None,
            HaltState::Halted => {
                if self.interrupts_pending(mmu) {
                    // A pending interrupt wakes HALT regardless of IME. If IME is
                    // disabled the CPU resumes execution without servicing the
                    // interrupt; if IME is enabled, service_interrupts() will handle
                    // it on the same step. Either way, no halt bug applies here —
                    // the bug only fires when HALT is *executed* with a pending
                    // interrupt (see opcode 0x76).
                    self.halt_state = HaltState::Running;
                    self.log_halt_wake();
                    None
                } else {
                    // Still halted: execute one idle M-cycle (NOP equivalent).
                    mmu.tick_m_cycle();
                    Some(4)
                }
            }
        }
    }

    fn interrupts_pending(&self, mmu: &mut impl MemoryBus) -> bool {
        self.pending_interrupt_mask(mmu) != 0
    }

    fn pending_interrupt_mask(&self, mmu: &mut impl MemoryBus) -> u8 {
        let ie = mmu.read_byte(0xFFFF);
        let iflag = mmu.read_byte(0xFF0F);
        ie & iflag
    }

    fn advance_ime_schedule(&mut self) {
        if let Some(remaining) = self.ime_scheduled {
            if remaining == 0 {
                self.ime = true;
                self.ime_scheduled = None;
                self.log_ime_enabled();
            } else {
                self.ime_scheduled = Some(remaining - 1);
            }
        }
    }

    pub(crate) fn service_interrupts(&mut self, mmu: &mut impl MemoryBus) -> Option<u8> {
        let ie = mmu.read_byte(0xFFFF);
        let mut iflag = mmu.read_byte(0xFF0F);
        let pending = ie & iflag;

        if pending == 0 {
            return None;
        }

        if !self.ime {
            // A pending interrupt wakes HALT even if IME is disabled.
            self.halt_state = HaltState::Running;
            return None;
        }

        self.halt_state = HaltState::Running;
        self.ime = false;
        self.ime_scheduled = None;

        for i in 0..5 {
            let mask = 1 << i;
            if pending & mask != 0 {
                // Interrupt dispatch: 20 cycles = 5 M-cycles.
                //   M1: disable IME, acknowledge interrupt (internal)
                //   M2: NOP / stall (internal)
                //   M3: push PCH to [SP-1]
                //   M4: push PCL to [SP-2]
                //   M5: load PC with interrupt vector (internal)
                //
                // The IF write is an internal bus operation, not a CPU M-cycle.
                // The push (M3+M4) is handled by self.push which ticks twice.
                mmu.tick_m_cycle(); // M1: internal
                mmu.tick_m_cycle(); // M2: internal
                iflag &= !mask;
                mmu.write_byte(0xFF0F, iflag); // acknowledge interrupt (no tick: internal)
                self.push(mmu, self.reg.pc); // M3, M4
                let vector = match i {
                    0 => 0x40,
                    1 => 0x48,
                    2 => 0x50,
                    3 => 0x58,
                    4 => 0x60,
                    _ => 0x00,
                };
                mmu.tick_m_cycle(); // M5: internal (load PC / vector fetch)
                self.reg.pc = vector;
                self.log_interrupt_service(vector, i);
                return Some(20);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Memory;

    struct DummyBus {
        mem: [u8; 0x10000],
    }

    impl DummyBus {
        fn new() -> Self {
            Self { mem: [0; 0x10000] }
        }
    }

    impl Memory for DummyBus {
        fn read_byte(&self, address: u16) -> u8 {
            self.mem[address as usize]
        }

        fn write_byte(&mut self, address: u16, value: u8) {
            self.mem[address as usize] = value;
        }
    }

    impl MemoryBus for DummyBus {}

    #[test]
    fn halt_bug_repeats_instruction_with_glitch() {
        let mut bus = DummyBus::new();
        // Program layout:
        // 0x100: HALT
        // 0x101: LD A,0x42
        // 0x102: 0x42 (immediate)
        bus.mem[0x100] = 0x76;
        bus.mem[0x101] = 0x3E;
        bus.mem[0x102] = 0x42;
        bus.mem[0x103] = 0x00;
        bus.mem[0xFFFF] = 0x01; // IE: enable VBlank
        bus.mem[0xFF0F] = 0x01; // IF: request VBlank

        let mut cpu = CPU::new();
        cpu.reg.pc = 0x0100;
        cpu.ime = false;
        cpu.reg.d = 0x77;

        cpu.step(&mut bus); // Execute HALT, triggers bug and leaves CPU running
        assert_eq!(cpu.halt_state, HaltState::Running);

        cpu.step(&mut bus); // First execution of LD A,0x42 (glitched)
        assert_eq!(
            cpu.reg.a, 0x3E,
            "First instruction after HALT bug should consume the opcode as its operand"
        );
        assert_eq!(
            cpu.reg.pc, 0x0102,
            "PC should advance only once, landing on the byte that was meant to be the immediate"
        );

        cpu.step(&mut bus); // Execute opcode at 0x0102 (0x42 == LD B,D)
        assert_eq!(
            cpu.reg.b, 0x77,
            "Next opcode should execute using the old immediate byte"
        );
        assert_eq!(cpu.reg.pc, 0x0103);
    }

    #[test]
    #[ignore = "debug helper"]
    fn debug_halt_bug_rom_once() {
        use crate::cartridge::CartridgeKind;
        use crate::gameboy::DMG;
        use std::path::PathBuf;

        let rom_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join("target/blargg-test-roms/halt_bug.gb");
        let data =
            std::fs::read(&rom_path).expect("failed to read halt_bug.gb (run blargg download?)");
        let cart = CartridgeKind::from_bytes(data).expect("failed to parse cartridge");
        let mut gb = DMG::new(Box::new(cart));

        for frame in 0..200 {
            gb.step_frame();
            if frame % 10 == 0 {
                eprintln!(
                    "frame {} pc={:04X} serial='{}'",
                    frame,
                    gb.cpu().registers().pc,
                    gb.serial_output()
                );
            }
        }

        eprintln!("code @C080:");
        let mut line = String::new();
        for (i, addr) in (0xC080u16..0xC0A0).enumerate() {
            if i % 8 == 0 {
                if !line.is_empty() {
                    eprintln!("{line}");
                }
                line = format!("{addr:04X}:");
            }
            line.push_str(&format!(" {:02X}", gb.peek_byte(addr)));
        }
        if !line.is_empty() {
            eprintln!("{line}");
        }

        eprintln!("data @C210:");
        line.clear();
        for (i, addr) in (0xC210u16..0xC230).enumerate() {
            if i % 8 == 0 {
                if !line.is_empty() {
                    eprintln!("{line}");
                }
                line = format!("{addr:04X}:");
            }
            line.push_str(&format!(" {:02X}", gb.peek_byte(addr)));
        }
        if !line.is_empty() {
            eprintln!("{line}");
        }

        eprintln!("serial output: '{}'", gb.serial_output());
    }
}
