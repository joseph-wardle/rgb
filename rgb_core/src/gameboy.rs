use crate::cartridge::Cartridge;
use crate::cpu::CPU;
#[cfg(test)]
use crate::memory::Memory;
use crate::mmu::MMU;
use crate::serial::Serial;

// Architecture (grep: "Bus/Devices"):
// DMG owns CPU + Bus. Bus (MMU) owns Devices + RAM + Interrupts.
pub struct DMG {
    cpu: CPU,
    bus: MMU,
}

impl DMG {
    /// Creates a DMG starting from the post-boot-ROM register state (PC=0x0100).
    /// Boot ROM emulation is not yet implemented, so this is the only way to
    /// start the emulator.
    pub fn new(cartridge: Box<dyn Cartridge>) -> Self {
        let dmg = Self {
            cpu: CPU::new(),
            bus: MMU::new(cartridge),
        };
        dmg.log_power_on();
        dmg
    }

    pub fn step_frame(&mut self) {
        const CYCLES_PER_FRAME: u32 = 70_224;
        self.log_frame_start(CYCLES_PER_FRAME);
        let mut consumed: u32 = 0;

        while consumed < CYCLES_PER_FRAME {
            // Each iteration advances the CPU by one instruction and then tells
            // other clocked subsystems how many machine cycles it took.
            let cycles = self.cpu.step(&mut self.bus);
            consumed += cycles as u32;
            self.bus.step(cycles.into());

            if let Some(extra) = self.cpu.service_interrupts(&mut self.bus) {
                consumed += extra as u32;
                self.bus.step(extra.into());
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
    /// Laid out row-major: index = y * 160 + x. All zeroes until the pixel
    /// pipeline is implemented.
    pub fn framebuffer(&self) -> &[u8] {
        self.bus.framebuffer()
    }
}

#[cfg(test)]
impl DMG {
    pub(crate) fn peek_byte(&self, address: u16) -> u8 {
        self.bus.read_byte(address)
    }
}
