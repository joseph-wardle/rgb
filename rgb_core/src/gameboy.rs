use crate::cartridge::Cartridge;
use crate::cpu::CPU;
use crate::input::Button;
use crate::memory::Memory;
use crate::mmu::MMU;
use crate::serial::Serial;

// DMG owns CPU + Bus. Bus (MMU) owns Devices + RAM + Interrupts.
pub struct DMG {
    cpu: CPU,
    bus: MMU,
}

impl DMG {
    /// Creates a DMG starting from the post-boot-ROM register state (PC=0x0100).
    /// To run an actual boot ROM, use [`DMG::new_with_boot_rom`] instead.
    pub fn new(cartridge: Box<dyn Cartridge>) -> Self {
        let dmg = Self {
            cpu: CPU::new(),
            bus: MMU::new(cartridge, None),
        };
        dmg.log_power_on();
        dmg
    }

    /// Creates a DMG with a boot ROM image mapped at 0x0000–0x00FF.
    ///
    /// The CPU starts in the cold-start state (PC=0x0000, all registers zero)
    /// and will execute the boot ROM, which initialises hardware registers,
    /// displays the Nintendo logo, and jumps to 0x0100 when done.
    ///
    /// The boot ROM unmaps itself by writing to 0xFF50.  At that point the
    /// emulator continues from the cartridge entry point.
    ///
    /// The DMG boot ROM image is copyrighted by Nintendo and is not included
    /// here.  Many open-source alternatives (e.g. SameBoot, dmg_boot.bin) are
    /// compatible and produce the correct post-boot register state.
    pub fn new_with_boot_rom(cartridge: Box<dyn Cartridge>, boot_rom: Box<[u8]>) -> Self {
        let dmg = Self {
            cpu: CPU::new_cold(),
            bus: MMU::new(cartridge, Some(boot_rom)),
        };
        dmg.log_power_on();
        dmg
    }

    pub fn step_frame(&mut self) {
        const CYCLES_PER_FRAME: u32 = 70_224;
        self.log_frame_start(CYCLES_PER_FRAME);
        let mut consumed: u32 = 0;

        while consumed < CYCLES_PER_FRAME {
            // The CPU ticks all hardware devices (timer, PPU, APU) M-cycle-by-
            // M-cycle as it executes each instruction via MemoryBus::tick_m_cycle.
            let cycles = self.cpu.step(&mut self.bus);
            consumed += cycles as u32;

            if let Some(extra) = self.cpu.service_interrupts(&mut self.bus) {
                consumed += extra as u32;
            }
        }
        self.log_frame_done(consumed);
    }

    pub fn run_until<F>(&mut self, mut condition: F, max_steps: usize)
    where
        F: FnMut(&Serial) -> bool,
    {
        self.log_run_until_start(max_steps);
        let mut steps_executed: usize = 0;
        let mut condition_met = false;

        for step in 0..max_steps {
            self.step_frame();
            steps_executed = step + 1;
            if condition(self.serial()) {
                condition_met = true;
                self.log_run_until_condition_met(step, steps_executed);
                break;
            }
        }

        if !condition_met {
            self.log_run_until_exhausted(steps_executed, max_steps);
        }
    }

    pub fn cpu(&self) -> &CPU {
        &self.cpu
    }

    pub fn serial(&self) -> &Serial {
        self.bus.serial()
    }

    pub fn serial_output(&self) -> String {
        self.bus.serial().output_string()
    }

    /// The most recently completed frame as a flat array of shade indices (0–3).
    /// Laid out row-major: index = y * 160 + x.
    pub fn framebuffer(&self) -> &[u8] {
        self.bus.framebuffer()
    }

    /// Drain and return all audio samples generated during the last frame.
    ///
    /// Returns interleaved stereo f32 pairs (left, right, left, right, …)
    /// in the range −1.0 to +1.0.  Call once per frame and push the result
    /// into the audio output ring buffer.
    pub fn drain_samples(&mut self) -> Vec<f32> {
        self.bus.drain_samples()
    }

    /// Battery-backed RAM contents for writing to a `.sav` file on exit.
    /// Returns `None` if the cartridge has no battery (nothing to save).
    pub fn save_data(&self) -> Option<&[u8]> {
        self.bus.save_data()
    }

    /// Restore battery-backed RAM from a `.sav` file loaded at startup.
    /// No-op if the cartridge has no battery or the data size mismatches.
    pub fn load_save_data(&mut self, data: &[u8]) {
        self.bus.load_save_data(data);
    }

    /// Notify the emulator that a host key mapped to `button` is now held.
    /// Triggers a joypad interrupt if the button's row is selected and
    /// the button was previously released.
    pub fn press(&mut self, button: Button) {
        self.bus.press_button(button);
    }

    /// Notify the emulator that a host key mapped to `button` has been
    /// released. No interrupt is generated on release.
    pub fn release(&mut self, button: Button) {
        self.bus.release_button(button);
    }
}

impl DMG {
    /// Read a single byte from the bus without advancing any hardware state.
    /// Intended for test harnesses and debuggers — does not tick the clock.
    pub fn peek_byte(&self, address: u16) -> u8 {
        self.bus.read_byte(address)
    }

    /// Current program counter of the CPU.
    ///
    /// Intended for test harnesses that need to detect when execution has
    /// reached a specific address — for example the `LD B,B` done-signal
    /// loop used by mooneye-test-suite ROMs.
    pub fn cpu_pc(&self) -> u16 {
        self.cpu.registers().pc
    }

    /// Returns `true` when the CPU registers contain the mooneye-test-suite
    /// PASS signature: B=3, C=5, D=8, E=13, H=21, L=34 (Fibonacci sequence).
    ///
    /// Mooneye ROMs signal test completion by loading those values and then
    /// executing `LD B,B` (opcode 0x40) in a tight loop.  Call this after
    /// detecting a stable PC at a `LD B,B` instruction to distinguish pass
    /// from fail.
    pub fn mooneye_pass(&self) -> bool {
        let r = self.cpu.registers();
        r.b == 3 && r.c == 5 && r.d == 8 && r.e == 13 && r.h == 21 && r.l == 34
    }
}
