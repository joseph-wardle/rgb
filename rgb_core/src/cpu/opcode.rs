//! Opcode cycle cost tables for the Game Boy's SM83 CPU.
//!
//! Timings are sourced from <https://gbdev.io/gb-opcodes/optables> (retrieved on 2024-10-31).
//! Keeping the data in a dedicated module makes it easy to audit and reference
//! against documentation while letting the execution engine focus purely on behaviour.

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) struct CycleCost {
    base: u8,
    taken: Option<u8>,
}

impl CycleCost {
    /// Returns a cost for instructions that always consume the same number of cycles.
    pub(super) const fn fixed(cycles: u8) -> Self {
        Self {
            base: cycles,
            taken: None,
        }
    }

    /// Returns a cost for instructions whose duration depends on a branching condition.
    pub(super) const fn branch(not_taken: u8, taken: u8) -> Self {
        Self {
            base: not_taken,
            taken: Some(taken),
        }
    }

    /// Marks an opcode that is undocumented / illegal on real hardware.
    pub(super) const fn illegal() -> Self {
        Self {
            base: 0,
            taken: None,
        }
    }

    /// Computes the total cycles consumed, accounting for conditional branches.
    pub(super) const fn total(self, took_conditional: bool) -> u8 {
        match (took_conditional, self.taken) {
            (true, Some(taken)) => taken,
            _ => self.base,
        }
    }
}

/// Returns the timing information for an unprefixed opcode (0x00-0xFF).
pub(super) fn cycle_cost(opcode: u8) -> CycleCost {
    MAIN_CYCLE_COSTS[opcode as usize]
}

/// Returns the timing information for a CB-prefixed opcode (0x00-0xFF).
pub(super) fn cb_cycle_cost(opcode: u8) -> CycleCost {
    CB_CYCLE_COSTS[opcode as usize]
}

/// Lookup table for the 256 unprefixed opcodes.
pub(super) const MAIN_CYCLE_COSTS: [CycleCost; 256] = [
    CycleCost::fixed(4),       // 0x00: NOP
    CycleCost::fixed(12),      // 0x01: LD BC, n16
    CycleCost::fixed(8),       // 0x02: LD [BC], A
    CycleCost::fixed(8),       // 0x03: INC BC
    CycleCost::fixed(4),       // 0x04: INC B
    CycleCost::fixed(4),       // 0x05: DEC B
    CycleCost::fixed(8),       // 0x06: LD B, n8
    CycleCost::fixed(4),       // 0x07: RLCA
    CycleCost::fixed(20),      // 0x08: LD [a16], SP
    CycleCost::fixed(8),       // 0x09: ADD HL, BC
    CycleCost::fixed(8),       // 0x0A: LD A, [BC]
    CycleCost::fixed(8),       // 0x0B: DEC BC
    CycleCost::fixed(4),       // 0x0C: INC C
    CycleCost::fixed(4),       // 0x0D: DEC C
    CycleCost::fixed(8),       // 0x0E: LD C, n8
    CycleCost::fixed(4),       // 0x0F: RRCA
    CycleCost::fixed(4),       // 0x10: STOP n8
    CycleCost::fixed(12),      // 0x11: LD DE, n16
    CycleCost::fixed(8),       // 0x12: LD [DE], A
    CycleCost::fixed(8),       // 0x13: INC DE
    CycleCost::fixed(4),       // 0x14: INC D
    CycleCost::fixed(4),       // 0x15: DEC D
    CycleCost::fixed(8),       // 0x16: LD D, n8
    CycleCost::fixed(4),       // 0x17: RLA
    CycleCost::fixed(12),      // 0x18: JR e8
    CycleCost::fixed(8),       // 0x19: ADD HL, DE
    CycleCost::fixed(8),       // 0x1A: LD A, [DE]
    CycleCost::fixed(8),       // 0x1B: DEC DE
    CycleCost::fixed(4),       // 0x1C: INC E
    CycleCost::fixed(4),       // 0x1D: DEC E
    CycleCost::fixed(8),       // 0x1E: LD E, n8
    CycleCost::fixed(4),       // 0x1F: RRA
    CycleCost::branch(8, 12),  // 0x20: JR NZ, e8
    CycleCost::fixed(12),      // 0x21: LD HL, n16
    CycleCost::fixed(8),       // 0x22: LD [HL+], A
    CycleCost::fixed(8),       // 0x23: INC HL
    CycleCost::fixed(4),       // 0x24: INC H
    CycleCost::fixed(4),       // 0x25: DEC H
    CycleCost::fixed(8),       // 0x26: LD H, n8
    CycleCost::fixed(4),       // 0x27: DAA
    CycleCost::branch(8, 12),  // 0x28: JR Z, e8
    CycleCost::fixed(8),       // 0x29: ADD HL, HL
    CycleCost::fixed(8),       // 0x2A: LD A, [HL+]
    CycleCost::fixed(8),       // 0x2B: DEC HL
    CycleCost::fixed(4),       // 0x2C: INC L
    CycleCost::fixed(4),       // 0x2D: DEC L
    CycleCost::fixed(8),       // 0x2E: LD L, n8
    CycleCost::fixed(4),       // 0x2F: CPL
    CycleCost::branch(8, 12),  // 0x30: JR NC, e8
    CycleCost::fixed(12),      // 0x31: LD SP, n16
    CycleCost::fixed(8),       // 0x32: LD [HL-], A
    CycleCost::fixed(8),       // 0x33: INC SP
    CycleCost::fixed(12),      // 0x34: INC [HL]
    CycleCost::fixed(12),      // 0x35: DEC [HL]
    CycleCost::fixed(12),      // 0x36: LD [HL], n8
    CycleCost::fixed(4),       // 0x37: SCF
    CycleCost::branch(8, 12),  // 0x38: JR C, e8
    CycleCost::fixed(8),       // 0x39: ADD HL, SP
    CycleCost::fixed(8),       // 0x3A: LD A, [HL-]
    CycleCost::fixed(8),       // 0x3B: DEC SP
    CycleCost::fixed(4),       // 0x3C: INC A
    CycleCost::fixed(4),       // 0x3D: DEC A
    CycleCost::fixed(8),       // 0x3E: LD A, n8
    CycleCost::fixed(4),       // 0x3F: CCF
    CycleCost::fixed(4),       // 0x40: LD B, B
    CycleCost::fixed(4),       // 0x41: LD B, C
    CycleCost::fixed(4),       // 0x42: LD B, D
    CycleCost::fixed(4),       // 0x43: LD B, E
    CycleCost::fixed(4),       // 0x44: LD B, H
    CycleCost::fixed(4),       // 0x45: LD B, L
    CycleCost::fixed(8),       // 0x46: LD B, [HL]
    CycleCost::fixed(4),       // 0x47: LD B, A
    CycleCost::fixed(4),       // 0x48: LD C, B
    CycleCost::fixed(4),       // 0x49: LD C, C
    CycleCost::fixed(4),       // 0x4A: LD C, D
    CycleCost::fixed(4),       // 0x4B: LD C, E
    CycleCost::fixed(4),       // 0x4C: LD C, H
    CycleCost::fixed(4),       // 0x4D: LD C, L
    CycleCost::fixed(8),       // 0x4E: LD C, [HL]
    CycleCost::fixed(4),       // 0x4F: LD C, A
    CycleCost::fixed(4),       // 0x50: LD D, B
    CycleCost::fixed(4),       // 0x51: LD D, C
    CycleCost::fixed(4),       // 0x52: LD D, D
    CycleCost::fixed(4),       // 0x53: LD D, E
    CycleCost::fixed(4),       // 0x54: LD D, H
    CycleCost::fixed(4),       // 0x55: LD D, L
    CycleCost::fixed(8),       // 0x56: LD D, [HL]
    CycleCost::fixed(4),       // 0x57: LD D, A
    CycleCost::fixed(4),       // 0x58: LD E, B
    CycleCost::fixed(4),       // 0x59: LD E, C
    CycleCost::fixed(4),       // 0x5A: LD E, D
    CycleCost::fixed(4),       // 0x5B: LD E, E
    CycleCost::fixed(4),       // 0x5C: LD E, H
    CycleCost::fixed(4),       // 0x5D: LD E, L
    CycleCost::fixed(8),       // 0x5E: LD E, [HL]
    CycleCost::fixed(4),       // 0x5F: LD E, A
    CycleCost::fixed(4),       // 0x60: LD H, B
    CycleCost::fixed(4),       // 0x61: LD H, C
    CycleCost::fixed(4),       // 0x62: LD H, D
    CycleCost::fixed(4),       // 0x63: LD H, E
    CycleCost::fixed(4),       // 0x64: LD H, H
    CycleCost::fixed(4),       // 0x65: LD H, L
    CycleCost::fixed(8),       // 0x66: LD H, [HL]
    CycleCost::fixed(4),       // 0x67: LD H, A
    CycleCost::fixed(4),       // 0x68: LD L, B
    CycleCost::fixed(4),       // 0x69: LD L, C
    CycleCost::fixed(4),       // 0x6A: LD L, D
    CycleCost::fixed(4),       // 0x6B: LD L, E
    CycleCost::fixed(4),       // 0x6C: LD L, H
    CycleCost::fixed(4),       // 0x6D: LD L, L
    CycleCost::fixed(8),       // 0x6E: LD L, [HL]
    CycleCost::fixed(4),       // 0x6F: LD L, A
    CycleCost::fixed(8),       // 0x70: LD [HL], B
    CycleCost::fixed(8),       // 0x71: LD [HL], C
    CycleCost::fixed(8),       // 0x72: LD [HL], D
    CycleCost::fixed(8),       // 0x73: LD [HL], E
    CycleCost::fixed(8),       // 0x74: LD [HL], H
    CycleCost::fixed(8),       // 0x75: LD [HL], L
    CycleCost::fixed(4),       // 0x76: HALT
    CycleCost::fixed(8),       // 0x77: LD [HL], A
    CycleCost::fixed(4),       // 0x78: LD A, B
    CycleCost::fixed(4),       // 0x79: LD A, C
    CycleCost::fixed(4),       // 0x7A: LD A, D
    CycleCost::fixed(4),       // 0x7B: LD A, E
    CycleCost::fixed(4),       // 0x7C: LD A, H
    CycleCost::fixed(4),       // 0x7D: LD A, L
    CycleCost::fixed(8),       // 0x7E: LD A, [HL]
    CycleCost::fixed(4),       // 0x7F: LD A, A
    CycleCost::fixed(4),       // 0x80: ADD A, B
    CycleCost::fixed(4),       // 0x81: ADD A, C
    CycleCost::fixed(4),       // 0x82: ADD A, D
    CycleCost::fixed(4),       // 0x83: ADD A, E
    CycleCost::fixed(4),       // 0x84: ADD A, H
    CycleCost::fixed(4),       // 0x85: ADD A, L
    CycleCost::fixed(8),       // 0x86: ADD A, [HL]
    CycleCost::fixed(4),       // 0x87: ADD A, A
    CycleCost::fixed(4),       // 0x88: ADC A, B
    CycleCost::fixed(4),       // 0x89: ADC A, C
    CycleCost::fixed(4),       // 0x8A: ADC A, D
    CycleCost::fixed(4),       // 0x8B: ADC A, E
    CycleCost::fixed(4),       // 0x8C: ADC A, H
    CycleCost::fixed(4),       // 0x8D: ADC A, L
    CycleCost::fixed(8),       // 0x8E: ADC A, [HL]
    CycleCost::fixed(4),       // 0x8F: ADC A, A
    CycleCost::fixed(4),       // 0x90: SUB A, B
    CycleCost::fixed(4),       // 0x91: SUB A, C
    CycleCost::fixed(4),       // 0x92: SUB A, D
    CycleCost::fixed(4),       // 0x93: SUB A, E
    CycleCost::fixed(4),       // 0x94: SUB A, H
    CycleCost::fixed(4),       // 0x95: SUB A, L
    CycleCost::fixed(8),       // 0x96: SUB A, [HL]
    CycleCost::fixed(4),       // 0x97: SUB A, A
    CycleCost::fixed(4),       // 0x98: SBC A, B
    CycleCost::fixed(4),       // 0x99: SBC A, C
    CycleCost::fixed(4),       // 0x9A: SBC A, D
    CycleCost::fixed(4),       // 0x9B: SBC A, E
    CycleCost::fixed(4),       // 0x9C: SBC A, H
    CycleCost::fixed(4),       // 0x9D: SBC A, L
    CycleCost::fixed(8),       // 0x9E: SBC A, [HL]
    CycleCost::fixed(4),       // 0x9F: SBC A, A
    CycleCost::fixed(4),       // 0xA0: AND A, B
    CycleCost::fixed(4),       // 0xA1: AND A, C
    CycleCost::fixed(4),       // 0xA2: AND A, D
    CycleCost::fixed(4),       // 0xA3: AND A, E
    CycleCost::fixed(4),       // 0xA4: AND A, H
    CycleCost::fixed(4),       // 0xA5: AND A, L
    CycleCost::fixed(8),       // 0xA6: AND A, [HL]
    CycleCost::fixed(4),       // 0xA7: AND A, A
    CycleCost::fixed(4),       // 0xA8: XOR A, B
    CycleCost::fixed(4),       // 0xA9: XOR A, C
    CycleCost::fixed(4),       // 0xAA: XOR A, D
    CycleCost::fixed(4),       // 0xAB: XOR A, E
    CycleCost::fixed(4),       // 0xAC: XOR A, H
    CycleCost::fixed(4),       // 0xAD: XOR A, L
    CycleCost::fixed(8),       // 0xAE: XOR A, [HL]
    CycleCost::fixed(4),       // 0xAF: XOR A, A
    CycleCost::fixed(4),       // 0xB0: OR A, B
    CycleCost::fixed(4),       // 0xB1: OR A, C
    CycleCost::fixed(4),       // 0xB2: OR A, D
    CycleCost::fixed(4),       // 0xB3: OR A, E
    CycleCost::fixed(4),       // 0xB4: OR A, H
    CycleCost::fixed(4),       // 0xB5: OR A, L
    CycleCost::fixed(8),       // 0xB6: OR A, [HL]
    CycleCost::fixed(4),       // 0xB7: OR A, A
    CycleCost::fixed(4),       // 0xB8: CP A, B
    CycleCost::fixed(4),       // 0xB9: CP A, C
    CycleCost::fixed(4),       // 0xBA: CP A, D
    CycleCost::fixed(4),       // 0xBB: CP A, E
    CycleCost::fixed(4),       // 0xBC: CP A, H
    CycleCost::fixed(4),       // 0xBD: CP A, L
    CycleCost::fixed(8),       // 0xBE: CP A, [HL]
    CycleCost::fixed(4),       // 0xBF: CP A, A
    CycleCost::branch(8, 20),  // 0xC0: RET NZ
    CycleCost::fixed(12),      // 0xC1: POP BC
    CycleCost::branch(12, 16), // 0xC2: JP NZ, a16
    CycleCost::fixed(16),      // 0xC3: JP a16
    CycleCost::branch(12, 24), // 0xC4: CALL NZ, a16
    CycleCost::fixed(16),      // 0xC5: PUSH BC
    CycleCost::fixed(8),       // 0xC6: ADD A, n8
    CycleCost::fixed(16),      // 0xC7: RST $00
    CycleCost::branch(8, 20),  // 0xC8: RET Z
    CycleCost::fixed(16),      // 0xC9: RET
    CycleCost::branch(12, 16), // 0xCA: JP Z, a16
    CycleCost::fixed(4),       // 0xCB: PREFIX
    CycleCost::branch(12, 24), // 0xCC: CALL Z, a16
    CycleCost::fixed(24),      // 0xCD: CALL a16
    CycleCost::fixed(8),       // 0xCE: ADC A, n8
    CycleCost::fixed(16),      // 0xCF: RST $08
    CycleCost::branch(8, 20),  // 0xD0: RET NC
    CycleCost::fixed(12),      // 0xD1: POP DE
    CycleCost::branch(12, 16), // 0xD2: JP NC, a16
    CycleCost::illegal(),      // 0xD3: —
    CycleCost::branch(12, 24), // 0xD4: CALL NC, a16
    CycleCost::fixed(16),      // 0xD5: PUSH DE
    CycleCost::fixed(8),       // 0xD6: SUB A, n8
    CycleCost::fixed(16),      // 0xD7: RST $10
    CycleCost::branch(8, 20),  // 0xD8: RET C
    CycleCost::fixed(16),      // 0xD9: RETI
    CycleCost::branch(12, 16), // 0xDA: JP C, a16
    CycleCost::illegal(),      // 0xDB: —
    CycleCost::branch(12, 24), // 0xDC: CALL C, a16
    CycleCost::illegal(),      // 0xDD: —
    CycleCost::fixed(8),       // 0xDE: SBC A, n8
    CycleCost::fixed(16),      // 0xDF: RST $18
    CycleCost::fixed(12),      // 0xE0: LDH [a8], A
    CycleCost::fixed(12),      // 0xE1: POP HL
    CycleCost::fixed(8),       // 0xE2: LDH [C], A
    CycleCost::illegal(),      // 0xE3: —
    CycleCost::illegal(),      // 0xE4: —
    CycleCost::fixed(16),      // 0xE5: PUSH HL
    CycleCost::fixed(8),       // 0xE6: AND A, n8
    CycleCost::fixed(16),      // 0xE7: RST $20
    CycleCost::fixed(16),      // 0xE8: ADD SP, e8
    CycleCost::fixed(4),       // 0xE9: JP HL
    CycleCost::fixed(16),      // 0xEA: LD [a16], A
    CycleCost::illegal(),      // 0xEB: —
    CycleCost::illegal(),      // 0xEC: —
    CycleCost::illegal(),      // 0xED: —
    CycleCost::fixed(8),       // 0xEE: XOR A, n8
    CycleCost::fixed(16),      // 0xEF: RST $28
    CycleCost::fixed(12),      // 0xF0: LDH A, [a8]
    CycleCost::fixed(12),      // 0xF1: POP AF
    CycleCost::fixed(8),       // 0xF2: LDH A, [C]
    CycleCost::fixed(4),       // 0xF3: DI
    CycleCost::illegal(),      // 0xF4: —
    CycleCost::fixed(16),      // 0xF5: PUSH AF
    CycleCost::fixed(8),       // 0xF6: OR A, n8
    CycleCost::fixed(16),      // 0xF7: RST $30
    CycleCost::fixed(12),      // 0xF8: LD HL, SP + e8
    CycleCost::fixed(8),       // 0xF9: LD SP, HL
    CycleCost::fixed(16),      // 0xFA: LD A, [a16]
    CycleCost::fixed(4),       // 0xFB: EI
    CycleCost::illegal(),      // 0xFC: —
    CycleCost::illegal(),      // 0xFD: —
    CycleCost::fixed(8),       // 0xFE: CP A, n8
    CycleCost::fixed(16),      // 0xFF: RST $38
];

/// Lookup table for the 256 CB-prefixed opcodes.
pub(super) const CB_CYCLE_COSTS: [CycleCost; 256] = [
    CycleCost::fixed(8),  // 0xCB00: RLC B
    CycleCost::fixed(8),  // 0xCB01: RLC C
    CycleCost::fixed(8),  // 0xCB02: RLC D
    CycleCost::fixed(8),  // 0xCB03: RLC E
    CycleCost::fixed(8),  // 0xCB04: RLC H
    CycleCost::fixed(8),  // 0xCB05: RLC L
    CycleCost::fixed(16), // 0xCB06: RLC [HL]
    CycleCost::fixed(8),  // 0xCB07: RLC A
    CycleCost::fixed(8),  // 0xCB08: RRC B
    CycleCost::fixed(8),  // 0xCB09: RRC C
    CycleCost::fixed(8),  // 0xCB0A: RRC D
    CycleCost::fixed(8),  // 0xCB0B: RRC E
    CycleCost::fixed(8),  // 0xCB0C: RRC H
    CycleCost::fixed(8),  // 0xCB0D: RRC L
    CycleCost::fixed(16), // 0xCB0E: RRC [HL]
    CycleCost::fixed(8),  // 0xCB0F: RRC A
    CycleCost::fixed(8),  // 0xCB10: RL B
    CycleCost::fixed(8),  // 0xCB11: RL C
    CycleCost::fixed(8),  // 0xCB12: RL D
    CycleCost::fixed(8),  // 0xCB13: RL E
    CycleCost::fixed(8),  // 0xCB14: RL H
    CycleCost::fixed(8),  // 0xCB15: RL L
    CycleCost::fixed(16), // 0xCB16: RL [HL]
    CycleCost::fixed(8),  // 0xCB17: RL A
    CycleCost::fixed(8),  // 0xCB18: RR B
    CycleCost::fixed(8),  // 0xCB19: RR C
    CycleCost::fixed(8),  // 0xCB1A: RR D
    CycleCost::fixed(8),  // 0xCB1B: RR E
    CycleCost::fixed(8),  // 0xCB1C: RR H
    CycleCost::fixed(8),  // 0xCB1D: RR L
    CycleCost::fixed(16), // 0xCB1E: RR [HL]
    CycleCost::fixed(8),  // 0xCB1F: RR A
    CycleCost::fixed(8),  // 0xCB20: SLA B
    CycleCost::fixed(8),  // 0xCB21: SLA C
    CycleCost::fixed(8),  // 0xCB22: SLA D
    CycleCost::fixed(8),  // 0xCB23: SLA E
    CycleCost::fixed(8),  // 0xCB24: SLA H
    CycleCost::fixed(8),  // 0xCB25: SLA L
    CycleCost::fixed(16), // 0xCB26: SLA [HL]
    CycleCost::fixed(8),  // 0xCB27: SLA A
    CycleCost::fixed(8),  // 0xCB28: SRA B
    CycleCost::fixed(8),  // 0xCB29: SRA C
    CycleCost::fixed(8),  // 0xCB2A: SRA D
    CycleCost::fixed(8),  // 0xCB2B: SRA E
    CycleCost::fixed(8),  // 0xCB2C: SRA H
    CycleCost::fixed(8),  // 0xCB2D: SRA L
    CycleCost::fixed(16), // 0xCB2E: SRA [HL]
    CycleCost::fixed(8),  // 0xCB2F: SRA A
    CycleCost::fixed(8),  // 0xCB30: SWAP B
    CycleCost::fixed(8),  // 0xCB31: SWAP C
    CycleCost::fixed(8),  // 0xCB32: SWAP D
    CycleCost::fixed(8),  // 0xCB33: SWAP E
    CycleCost::fixed(8),  // 0xCB34: SWAP H
    CycleCost::fixed(8),  // 0xCB35: SWAP L
    CycleCost::fixed(16), // 0xCB36: SWAP [HL]
    CycleCost::fixed(8),  // 0xCB37: SWAP A
    CycleCost::fixed(8),  // 0xCB38: SRL B
    CycleCost::fixed(8),  // 0xCB39: SRL C
    CycleCost::fixed(8),  // 0xCB3A: SRL D
    CycleCost::fixed(8),  // 0xCB3B: SRL E
    CycleCost::fixed(8),  // 0xCB3C: SRL H
    CycleCost::fixed(8),  // 0xCB3D: SRL L
    CycleCost::fixed(16), // 0xCB3E: SRL [HL]
    CycleCost::fixed(8),  // 0xCB3F: SRL A
    CycleCost::fixed(8),  // 0xCB40: BIT 0, B
    CycleCost::fixed(8),  // 0xCB41: BIT 0, C
    CycleCost::fixed(8),  // 0xCB42: BIT 0, D
    CycleCost::fixed(8),  // 0xCB43: BIT 0, E
    CycleCost::fixed(8),  // 0xCB44: BIT 0, H
    CycleCost::fixed(8),  // 0xCB45: BIT 0, L
    CycleCost::fixed(12), // 0xCB46: BIT 0, [HL]
    CycleCost::fixed(8),  // 0xCB47: BIT 0, A
    CycleCost::fixed(8),  // 0xCB48: BIT 1, B
    CycleCost::fixed(8),  // 0xCB49: BIT 1, C
    CycleCost::fixed(8),  // 0xCB4A: BIT 1, D
    CycleCost::fixed(8),  // 0xCB4B: BIT 1, E
    CycleCost::fixed(8),  // 0xCB4C: BIT 1, H
    CycleCost::fixed(8),  // 0xCB4D: BIT 1, L
    CycleCost::fixed(12), // 0xCB4E: BIT 1, [HL]
    CycleCost::fixed(8),  // 0xCB4F: BIT 1, A
    CycleCost::fixed(8),  // 0xCB50: BIT 2, B
    CycleCost::fixed(8),  // 0xCB51: BIT 2, C
    CycleCost::fixed(8),  // 0xCB52: BIT 2, D
    CycleCost::fixed(8),  // 0xCB53: BIT 2, E
    CycleCost::fixed(8),  // 0xCB54: BIT 2, H
    CycleCost::fixed(8),  // 0xCB55: BIT 2, L
    CycleCost::fixed(12), // 0xCB56: BIT 2, [HL]
    CycleCost::fixed(8),  // 0xCB57: BIT 2, A
    CycleCost::fixed(8),  // 0xCB58: BIT 3, B
    CycleCost::fixed(8),  // 0xCB59: BIT 3, C
    CycleCost::fixed(8),  // 0xCB5A: BIT 3, D
    CycleCost::fixed(8),  // 0xCB5B: BIT 3, E
    CycleCost::fixed(8),  // 0xCB5C: BIT 3, H
    CycleCost::fixed(8),  // 0xCB5D: BIT 3, L
    CycleCost::fixed(12), // 0xCB5E: BIT 3, [HL]
    CycleCost::fixed(8),  // 0xCB5F: BIT 3, A
    CycleCost::fixed(8),  // 0xCB60: BIT 4, B
    CycleCost::fixed(8),  // 0xCB61: BIT 4, C
    CycleCost::fixed(8),  // 0xCB62: BIT 4, D
    CycleCost::fixed(8),  // 0xCB63: BIT 4, E
    CycleCost::fixed(8),  // 0xCB64: BIT 4, H
    CycleCost::fixed(8),  // 0xCB65: BIT 4, L
    CycleCost::fixed(12), // 0xCB66: BIT 4, [HL]
    CycleCost::fixed(8),  // 0xCB67: BIT 4, A
    CycleCost::fixed(8),  // 0xCB68: BIT 5, B
    CycleCost::fixed(8),  // 0xCB69: BIT 5, C
    CycleCost::fixed(8),  // 0xCB6A: BIT 5, D
    CycleCost::fixed(8),  // 0xCB6B: BIT 5, E
    CycleCost::fixed(8),  // 0xCB6C: BIT 5, H
    CycleCost::fixed(8),  // 0xCB6D: BIT 5, L
    CycleCost::fixed(12), // 0xCB6E: BIT 5, [HL]
    CycleCost::fixed(8),  // 0xCB6F: BIT 5, A
    CycleCost::fixed(8),  // 0xCB70: BIT 6, B
    CycleCost::fixed(8),  // 0xCB71: BIT 6, C
    CycleCost::fixed(8),  // 0xCB72: BIT 6, D
    CycleCost::fixed(8),  // 0xCB73: BIT 6, E
    CycleCost::fixed(8),  // 0xCB74: BIT 6, H
    CycleCost::fixed(8),  // 0xCB75: BIT 6, L
    CycleCost::fixed(12), // 0xCB76: BIT 6, [HL]
    CycleCost::fixed(8),  // 0xCB77: BIT 6, A
    CycleCost::fixed(8),  // 0xCB78: BIT 7, B
    CycleCost::fixed(8),  // 0xCB79: BIT 7, C
    CycleCost::fixed(8),  // 0xCB7A: BIT 7, D
    CycleCost::fixed(8),  // 0xCB7B: BIT 7, E
    CycleCost::fixed(8),  // 0xCB7C: BIT 7, H
    CycleCost::fixed(8),  // 0xCB7D: BIT 7, L
    CycleCost::fixed(12), // 0xCB7E: BIT 7, [HL]
    CycleCost::fixed(8),  // 0xCB7F: BIT 7, A
    CycleCost::fixed(8),  // 0xCB80: RES 0, B
    CycleCost::fixed(8),  // 0xCB81: RES 0, C
    CycleCost::fixed(8),  // 0xCB82: RES 0, D
    CycleCost::fixed(8),  // 0xCB83: RES 0, E
    CycleCost::fixed(8),  // 0xCB84: RES 0, H
    CycleCost::fixed(8),  // 0xCB85: RES 0, L
    CycleCost::fixed(16), // 0xCB86: RES 0, [HL]
    CycleCost::fixed(8),  // 0xCB87: RES 0, A
    CycleCost::fixed(8),  // 0xCB88: RES 1, B
    CycleCost::fixed(8),  // 0xCB89: RES 1, C
    CycleCost::fixed(8),  // 0xCB8A: RES 1, D
    CycleCost::fixed(8),  // 0xCB8B: RES 1, E
    CycleCost::fixed(8),  // 0xCB8C: RES 1, H
    CycleCost::fixed(8),  // 0xCB8D: RES 1, L
    CycleCost::fixed(16), // 0xCB8E: RES 1, [HL]
    CycleCost::fixed(8),  // 0xCB8F: RES 1, A
    CycleCost::fixed(8),  // 0xCB90: RES 2, B
    CycleCost::fixed(8),  // 0xCB91: RES 2, C
    CycleCost::fixed(8),  // 0xCB92: RES 2, D
    CycleCost::fixed(8),  // 0xCB93: RES 2, E
    CycleCost::fixed(8),  // 0xCB94: RES 2, H
    CycleCost::fixed(8),  // 0xCB95: RES 2, L
    CycleCost::fixed(16), // 0xCB96: RES 2, [HL]
    CycleCost::fixed(8),  // 0xCB97: RES 2, A
    CycleCost::fixed(8),  // 0xCB98: RES 3, B
    CycleCost::fixed(8),  // 0xCB99: RES 3, C
    CycleCost::fixed(8),  // 0xCB9A: RES 3, D
    CycleCost::fixed(8),  // 0xCB9B: RES 3, E
    CycleCost::fixed(8),  // 0xCB9C: RES 3, H
    CycleCost::fixed(8),  // 0xCB9D: RES 3, L
    CycleCost::fixed(16), // 0xCB9E: RES 3, [HL]
    CycleCost::fixed(8),  // 0xCB9F: RES 3, A
    CycleCost::fixed(8),  // 0xCBA0: RES 4, B
    CycleCost::fixed(8),  // 0xCBA1: RES 4, C
    CycleCost::fixed(8),  // 0xCBA2: RES 4, D
    CycleCost::fixed(8),  // 0xCBA3: RES 4, E
    CycleCost::fixed(8),  // 0xCBA4: RES 4, H
    CycleCost::fixed(8),  // 0xCBA5: RES 4, L
    CycleCost::fixed(16), // 0xCBA6: RES 4, [HL]
    CycleCost::fixed(8),  // 0xCBA7: RES 4, A
    CycleCost::fixed(8),  // 0xCBA8: RES 5, B
    CycleCost::fixed(8),  // 0xCBA9: RES 5, C
    CycleCost::fixed(8),  // 0xCBAA: RES 5, D
    CycleCost::fixed(8),  // 0xCBAB: RES 5, E
    CycleCost::fixed(8),  // 0xCBAC: RES 5, H
    CycleCost::fixed(8),  // 0xCBAD: RES 5, L
    CycleCost::fixed(16), // 0xCBAE: RES 5, [HL]
    CycleCost::fixed(8),  // 0xCBAF: RES 5, A
    CycleCost::fixed(8),  // 0xCBB0: RES 6, B
    CycleCost::fixed(8),  // 0xCBB1: RES 6, C
    CycleCost::fixed(8),  // 0xCBB2: RES 6, D
    CycleCost::fixed(8),  // 0xCBB3: RES 6, E
    CycleCost::fixed(8),  // 0xCBB4: RES 6, H
    CycleCost::fixed(8),  // 0xCBB5: RES 6, L
    CycleCost::fixed(16), // 0xCBB6: RES 6, [HL]
    CycleCost::fixed(8),  // 0xCBB7: RES 6, A
    CycleCost::fixed(8),  // 0xCBB8: RES 7, B
    CycleCost::fixed(8),  // 0xCBB9: RES 7, C
    CycleCost::fixed(8),  // 0xCBBA: RES 7, D
    CycleCost::fixed(8),  // 0xCBBB: RES 7, E
    CycleCost::fixed(8),  // 0xCBBC: RES 7, H
    CycleCost::fixed(8),  // 0xCBBD: RES 7, L
    CycleCost::fixed(16), // 0xCBBE: RES 7, [HL]
    CycleCost::fixed(8),  // 0xCBBF: RES 7, A
    CycleCost::fixed(8),  // 0xCBC0: SET 0, B
    CycleCost::fixed(8),  // 0xCBC1: SET 0, C
    CycleCost::fixed(8),  // 0xCBC2: SET 0, D
    CycleCost::fixed(8),  // 0xCBC3: SET 0, E
    CycleCost::fixed(8),  // 0xCBC4: SET 0, H
    CycleCost::fixed(8),  // 0xCBC5: SET 0, L
    CycleCost::fixed(16), // 0xCBC6: SET 0, [HL]
    CycleCost::fixed(8),  // 0xCBC7: SET 0, A
    CycleCost::fixed(8),  // 0xCBC8: SET 1, B
    CycleCost::fixed(8),  // 0xCBC9: SET 1, C
    CycleCost::fixed(8),  // 0xCBCA: SET 1, D
    CycleCost::fixed(8),  // 0xCBCB: SET 1, E
    CycleCost::fixed(8),  // 0xCBCC: SET 1, H
    CycleCost::fixed(8),  // 0xCBCD: SET 1, L
    CycleCost::fixed(16), // 0xCBCE: SET 1, [HL]
    CycleCost::fixed(8),  // 0xCBCF: SET 1, A
    CycleCost::fixed(8),  // 0xCBD0: SET 2, B
    CycleCost::fixed(8),  // 0xCBD1: SET 2, C
    CycleCost::fixed(8),  // 0xCBD2: SET 2, D
    CycleCost::fixed(8),  // 0xCBD3: SET 2, E
    CycleCost::fixed(8),  // 0xCBD4: SET 2, H
    CycleCost::fixed(8),  // 0xCBD5: SET 2, L
    CycleCost::fixed(16), // 0xCBD6: SET 2, [HL]
    CycleCost::fixed(8),  // 0xCBD7: SET 2, A
    CycleCost::fixed(8),  // 0xCBD8: SET 3, B
    CycleCost::fixed(8),  // 0xCBD9: SET 3, C
    CycleCost::fixed(8),  // 0xCBDA: SET 3, D
    CycleCost::fixed(8),  // 0xCBDB: SET 3, E
    CycleCost::fixed(8),  // 0xCBDC: SET 3, H
    CycleCost::fixed(8),  // 0xCBDD: SET 3, L
    CycleCost::fixed(16), // 0xCBDE: SET 3, [HL]
    CycleCost::fixed(8),  // 0xCBDF: SET 3, A
    CycleCost::fixed(8),  // 0xCBE0: SET 4, B
    CycleCost::fixed(8),  // 0xCBE1: SET 4, C
    CycleCost::fixed(8),  // 0xCBE2: SET 4, D
    CycleCost::fixed(8),  // 0xCBE3: SET 4, E
    CycleCost::fixed(8),  // 0xCBE4: SET 4, H
    CycleCost::fixed(8),  // 0xCBE5: SET 4, L
    CycleCost::fixed(16), // 0xCBE6: SET 4, [HL]
    CycleCost::fixed(8),  // 0xCBE7: SET 4, A
    CycleCost::fixed(8),  // 0xCBE8: SET 5, B
    CycleCost::fixed(8),  // 0xCBE9: SET 5, C
    CycleCost::fixed(8),  // 0xCBEA: SET 5, D
    CycleCost::fixed(8),  // 0xCBEB: SET 5, E
    CycleCost::fixed(8),  // 0xCBEC: SET 5, H
    CycleCost::fixed(8),  // 0xCBED: SET 5, L
    CycleCost::fixed(16), // 0xCBEE: SET 5, [HL]
    CycleCost::fixed(8),  // 0xCBEF: SET 5, A
    CycleCost::fixed(8),  // 0xCBF0: SET 6, B
    CycleCost::fixed(8),  // 0xCBF1: SET 6, C
    CycleCost::fixed(8),  // 0xCBF2: SET 6, D
    CycleCost::fixed(8),  // 0xCBF3: SET 6, E
    CycleCost::fixed(8),  // 0xCBF4: SET 6, H
    CycleCost::fixed(8),  // 0xCBF5: SET 6, L
    CycleCost::fixed(16), // 0xCBF6: SET 6, [HL]
    CycleCost::fixed(8),  // 0xCBF7: SET 6, A
    CycleCost::fixed(8),  // 0xCBF8: SET 7, B
    CycleCost::fixed(8),  // 0xCBF9: SET 7, C
    CycleCost::fixed(8),  // 0xCBFA: SET 7, D
    CycleCost::fixed(8),  // 0xCBFB: SET 7, E
    CycleCost::fixed(8),  // 0xCBFC: SET 7, H
    CycleCost::fixed(8),  // 0xCBFD: SET 7, L
    CycleCost::fixed(16), // 0xCBFE: SET 7, [HL]
    CycleCost::fixed(8),  // 0xCBFF: SET 7, A
];
