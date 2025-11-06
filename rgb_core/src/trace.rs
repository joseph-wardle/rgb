use crate::cpu::CPU;
use crate::gameboy::DMG;
use crate::input::{Button, Joypad};
use crate::mmu::MMU;
use crate::serial::Serial;

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
    pub(crate) fn log_stop_enter(&self) {
        debug!(target: "gb::cpu", pc = %hex16!(self.registers().pc), "STOP entered");
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_stop_enter(&self) {}

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
    pub(crate) fn log_halt_bug(&self) {
        let regs = self.registers();
        debug!(
            target: "gb::cpu",
            pc = %hex16!(regs.pc),
            ime = self.ime,
            halt = ?self.halt_state,
            "HALT bug triggered"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_halt_bug(&self) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_stop_speed_switch(&self, parameter: u8) {
        debug!(
            target: "gb::cpu",
            pc = %hex16!(self.registers().pc),
            value = %hex8!(parameter),
            "STOP speed switch parameter ignored"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_stop_speed_switch(&self, _parameter: u8) {}

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

impl MMU {
    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_step(
        &self,
        cycles: u16,
        div: u8,
        tima: u8,
        tma: u8,
        tac: u8,
        interrupt_flag: u8,
        interrupt_enable: u8,
    ) {
        let timer_enabled = (tac & 0x04) != 0;
        trace!(
            target: "gb::mmu",
            cycles = cycles,
            div = %hex8!(div),
            tima = %hex8!(tima),
            tma = %hex8!(tma),
            tac = %hex8!(tac),
            timer_enabled = timer_enabled,
            interrupt_flag = %hex8!(interrupt_flag),
            interrupt_enable = %hex8!(interrupt_enable),
            "step"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn log_step(
        &self,
        _cycles: u16,
        _div: u8,
        _tima: u8,
        _tma: u8,
        _tac: u8,
        _interrupt_flag: u8,
        _interrupt_enable: u8,
    ) {
    }

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_io_read(&self, address: u16, value: u8) {
        if let Some(name) = Self::io_register_label(address) {
            trace!(
                target: "gb::mmu",
                register = name,
                address = %hex16!(address),
                value = %hex8!(value),
                "io.read"
            );
        }
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_io_read(&self, _address: u16, _value: u8) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_io_write(&self, address: u16, value: u8) {
        if let Some(name) = Self::io_register_label(address) {
            debug!(
                target: "gb::mmu",
                register = name,
                address = %hex16!(address),
                value = %hex8!(value),
                "io.write"
            );
        }
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_io_write(&self, _address: u16, _value: u8) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    fn io_register_label(address: u16) -> Option<&'static str> {
        match address {
            0xFF00 => Some("JOYP"),
            0xFF01 => Some("SB"),
            0xFF02 => Some("SC"),
            0xFF04 => Some("DIV"),
            0xFF05 => Some("TIMA"),
            0xFF06 => Some("TMA"),
            0xFF07 => Some("TAC"),
            0xFF0F => Some("IF"),
            0xFFFF => Some("IE"),
            _ => None,
        }
    }
}

impl Serial {
    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_data_write(&self, value: u8, buffer_len: usize) {
        trace!(
            target: "gb::serial",
            value = %hex8!(value),
            buffer_len = buffer_len,
            "sb.write"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_data_write(&self, _value: u8, _buffer_len: usize) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_control_write(
        &self,
        previous_sc: u8,
        raw_sc: u8,
        effective_sc: u8,
        start_transfer: bool,
        transferred: Option<u8>,
        buffer_len: usize,
    ) {
        if let Some(byte) = transferred {
            debug!(
                target: "gb::serial",
                prev = %hex8!(previous_sc),
                control = %hex8!(raw_sc),
                sc = %hex8!(effective_sc),
                byte = %hex8!(byte),
                buffer_len = buffer_len,
                "transfer"
            );
        } else {
            trace!(
                target: "gb::serial",
                prev = %hex8!(previous_sc),
                control = %hex8!(raw_sc),
                sc = %hex8!(effective_sc),
                start_transfer = start_transfer,
                buffer_len = buffer_len,
                "control.write"
            );
        }
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_control_write(
        &self,
        _previous_sc: u8,
        _raw_sc: u8,
        _effective_sc: u8,
        _start_transfer: bool,
        _transferred: Option<u8>,
        _buffer_len: usize,
    ) {
    }
}

impl Joypad {
    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_select_updated(
        &self,
        previous: u8,
        current: u8,
        written: u8,
        buttons_selected: bool,
        dpad_selected: bool,
    ) {
        trace!(
            target: "gb::joypad",
            write = %hex8!(written),
            previous = %hex8!(previous),
            state = %hex8!(current),
            buttons_selected = buttons_selected,
            dpad_selected = dpad_selected,
            "select.write"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_select_updated(
        &self,
        _previous: u8,
        _current: u8,
        _written: u8,
        _buttons_selected: bool,
        _dpad_selected: bool,
    ) {
    }

    #[inline(always)]
    #[cfg(feature = "trace")]
    pub(crate) fn log_button_query(&self, button: Button, pressed: bool) {
        trace!(
            target: "gb::joypad",
            button = Self::button_name(button),
            pressed = pressed,
            state = %hex8!(self.state),
            "query"
        );
    }

    #[inline(always)]
    #[cfg(not(feature = "trace"))]
    pub(crate) fn log_button_query(&self, _button: Button, _pressed: bool) {}

    #[inline(always)]
    #[cfg(feature = "trace")]
    fn button_name(button: Button) -> &'static str {
        match button {
            Button::Start => "Start",
            Button::Select => "Select",
            Button::B => "B",
            Button::A => "A",
            Button::Down => "Down",
            Button::Up => "Up",
            Button::Left => "Left",
            Button::Right => "Right",
        }
    }
}
