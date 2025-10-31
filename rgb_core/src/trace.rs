use crate::cpu::CPU;
use crate::gameboy::DMG;

#[cfg(feature = "trace")]
use crate::registers::Flag::{CARRY, HALF_CARRY, SUBTRACT, ZERO};
#[cfg(feature = "trace")]
use tracing::{debug, trace, warn};

#[cfg(feature = "trace")]
macro_rules! hex8 {
    ($v:expr) => {
        format_args!("{:#04X}", $v)
    };
}
#[cfg(feature = "trace")]
macro_rules! hex16 {
    ($v:expr) => {
        format_args!("{:#06X}", $v)
    };
}

impl CPU {
    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_inst_start(&self, pc_before: u16, opcode: u8) {
        let regs = self.registers();
        trace!(
            target: "gb::cpu",
            pc = %hex16!(pc_before),
            opcode = %hex8!(opcode),
            a = regs.a, b = regs.b, c = regs.c, d = regs.d, e = regs.e,
            h = regs.h, l = regs.l, sp = %hex16!(regs.sp),
            z = regs.get_flag(ZERO),
            n = regs.get_flag(SUBTRACT),
            hflag = regs.get_flag(HALF_CARRY),
            cflag = regs.get_flag(CARRY),
            ime = self.ime,
            halt = ?self.halt_state,
            "fetch"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_inst_start(&self, _pc_before: u16, _opcode: u8) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_inst_done(&self, opcode: u8, cycles: u8, took_conditional: bool) {
        let regs = self.registers();
        trace!(
            target: "gb::cpu",
            pc_after = %hex16!(regs.pc),
            opcode = %hex8!(opcode),
            cycles = cycles,
            conditional = took_conditional,
            a = regs.a, b = regs.b, c = regs.c, d = regs.d, e = regs.e,
            h = regs.h, l = regs.l, sp = %hex16!(regs.sp),
            z = regs.get_flag(ZERO),
            n = regs.get_flag(SUBTRACT),
            hflag = regs.get_flag(HALF_CARRY),
            cflag = regs.get_flag(CARRY),
            ime = self.ime,
            halt = ?self.halt_state,
            "exec"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_inst_done(&self, _opcode: u8, _cycles: u8, _cond: bool) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_halt_enter(&self) {
        debug!(target: "gb::cpu", pc = %hex16!(self.registers().pc), "HALT entered");
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_halt_enter(&self) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_halt_wake(&self) {
        debug!(target: "gb::cpu", pc = %hex16!(self.registers().pc), "HALT woken by pending interrupt");
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_halt_wake(&self) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_undocumented(&self, opcode: u8) {
        let regs = self.registers();
        warn!(
            target: "gb::cpu",
            pc = %hex16!(regs.pc.wrapping_sub(1)),
            opcode = %hex8!(opcode),
            "undocumented opcode (acts as NOP)"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_undocumented(&self, _opcode: u8) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_interrupt_service(&self, vector: u16, index: u8) {
        let regs = self.registers();
        debug!(
            target: "gb::cpu",
            pc = %hex16!(regs.pc),
            sp = %hex16!(regs.sp),
            vector = %hex16!(vector),
            index = index,
            "servicing interrupt"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_interrupt_service(&self, _vector: u16, _index: u8) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_ime_enabled(&self) {
        trace!(target: "gb::cpu", "IME enabled");
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_ime_enabled(&self) {}
}

impl DMG {
    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_power_on(&self, kind: &'static str) {
        let cpu = self.cpu();
        let regs = cpu.registers();
        debug!(
            target: "gb::dmg",
            boot = kind,
            pc = %hex16!(regs.pc),
            ime = cpu.ime,
            halt = ?cpu.halt_state,
            "boot"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_power_on(&self, _kind: &'static str) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_frame_start(&self, target_cycles: u32) {
        let cpu = self.cpu();
        let regs = cpu.registers();
        trace!(
            target: "gb::dmg",
            pc = %hex16!(regs.pc),
            target_cycles = target_cycles,
            ime = cpu.ime,
            halt = ?cpu.halt_state,
            "frame.start"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_frame_start(&self, _target_cycles: u32) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_frame_done(&self, consumed_cycles: u32) {
        let cpu = self.cpu();
        let regs = cpu.registers();
        trace!(
            target: "gb::dmg",
            pc = %hex16!(regs.pc),
            cycles = consumed_cycles,
            ime = cpu.ime,
            halt = ?cpu.halt_state,
            "frame.complete"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_frame_done(&self, _consumed_cycles: u32) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_run_until_start(&self, max_steps: usize) {
        let cpu = self.cpu();
        let regs = cpu.registers();
        let serial = self.serial();
        trace!(
            target: "gb::dmg",
            pc = %hex16!(regs.pc),
            max_steps = max_steps,
            serial_len = serial.len(),
            ime = cpu.ime,
            halt = ?cpu.halt_state,
            "run_until.start"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_run_until_start(&self, _max_steps: usize) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_run_until_condition_met(&self, step_index: usize, steps_executed: usize) {
        let cpu = self.cpu();
        let regs = cpu.registers();
        let serial = self.serial();
        debug!(
            target: "gb::dmg",
            pc = %hex16!(regs.pc),
            step_index = step_index,
            steps_executed = steps_executed,
            serial_len = serial.len(),
            "run_until.condition_met"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_run_until_condition_met(&self, _step_index: usize, _steps_executed: usize) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_run_until_exhausted(&self, steps_executed: usize, max_steps: usize) {
        let serial = self.serial();
        warn!(
            target: "gb::dmg",
            steps_executed = steps_executed,
            max_steps = max_steps,
            serial_len = serial.len(),
            "run_until.max_steps_exhausted"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_run_until_exhausted(&self, _steps_executed: usize, _max_steps: usize) {}
}
