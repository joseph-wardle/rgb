use crate::cartridge::Cartridge;
use crate::cpu::CPU;
use crate::input::Button;
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
    ///
    /// Call once per frame for every button that is currently pressed (not
    /// just on the keydown edge). Triggers a joypad interrupt if the button's
    /// row is selected and the button was previously released.
    pub fn press(&mut self, button: Button) {
        self.bus.press_button(button);
    }

    /// Notify the emulator that a host key mapped to `button` has been
    /// released. No interrupt is generated on release.
    pub fn release(&mut self, button: Button) {
        self.bus.release_button(button);
    }
}

#[cfg(test)]
impl DMG {
    pub(crate) fn peek_byte(&self, address: u16) -> u8 {
        self.bus.read_byte(address)
    }
}
