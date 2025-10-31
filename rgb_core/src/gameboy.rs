use crate::apu::APU;
use crate::cartridge::Cartridge;
use crate::cpu::CPU;
use crate::mmu::MMU;
use crate::ppu::PPU;
use crate::serial::Serial;

#[expect(
    dead_code,
    reason = "PPU is not fully implemented yet, but will be used in the future when rendering is added"
)]
pub struct DMG {
    cpu: CPU,
    ppu: PPU,
    apu: APU,
    pub bus: MMU,
}

impl DMG {
    pub fn new(cartridge: Box<dyn Cartridge>) -> Self {
        Self {
            cpu: CPU::new(),
            ppu: PPU::new(),
            apu: APU::new(),
            bus: MMU::new(cartridge),
        }
    }

    pub fn new_post_bios(cartridge: Box<dyn Cartridge>) -> Self {
        Self {
            cpu: CPU::new_post_bios(),
            ppu: PPU::new(),
            apu: APU::new(),
            bus: MMU::new(cartridge),
        }
    }

    pub fn step_frame(&mut self) {
        const CYCLES_PER_FRAME: u32 = 70_224;
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
    }

    pub fn run_until<F>(&mut self, mut condition: F, max_steps: usize)
    where
        F: FnMut(&Serial) -> bool,
    {
        for _ in 0..max_steps {
            self.step_frame();
            if condition(&self.bus.serial) {
                break;
            }
        }
    }

    pub fn cpu(&self) -> &CPU {
        &self.cpu
    }

    pub fn cpu_mut(&mut self) -> &mut CPU {
        &mut self.cpu
    }
}
