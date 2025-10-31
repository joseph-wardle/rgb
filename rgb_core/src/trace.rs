use crate::cpu::CPU;

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
        trace!(
            target: "gb::cpu",
            pc = %hex16!(pc_before),
            opcode = %hex8!(opcode),
            a = self.reg.a, b = self.reg.b, c = self.reg.c, d = self.reg.d, e = self.reg.e,
            h = self.reg.h, l = self.reg.l, sp = %hex16!(self.reg.sp),
            z = self.reg.get_flag(ZERO),
            n = self.reg.get_flag(SUBTRACT),
            hflag = self.reg.get_flag(HALF_CARRY),
            cflag = self.reg.get_flag(CARRY),
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
        trace!(
            target: "gb::cpu",
            pc_after = %hex16!(self.reg.pc),
            opcode = %hex8!(opcode),
            cycles = cycles,
            conditional = took_conditional,
            a = self.reg.a, b = self.reg.b, c = self.reg.c, d = self.reg.d, e = self.reg.e,
            h = self.reg.h, l = self.reg.l, sp = %hex16!(self.reg.sp),
            z = self.reg.get_flag(ZERO),
            n = self.reg.get_flag(SUBTRACT),
            hflag = self.reg.get_flag(HALF_CARRY),
            cflag = self.reg.get_flag(CARRY),
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
        debug!(target: "gb::cpu", pc = %hex16!(self.reg.pc), "HALT entered");
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_halt_enter(&self) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_halt_wake(&self) {
        debug!(target: "gb::cpu", pc = %hex16!(self.reg.pc), "HALT woken by pending interrupt");
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_halt_wake(&self) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_undocumented(&self, opcode: u8) {
        warn!(
            target: "gb::cpu",
            pc = %hex16!(self.reg.pc.wrapping_sub(1)),
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
        debug!(
            target: "gb::cpu",
            pc = %hex16!(self.reg.pc),
            sp = %hex16!(self.reg.sp),
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
