use crate::apu::APU;
use crate::cartridge::Cartridge;
use crate::cpu::CPU;
use crate::mmu::MMU;
use crate::ppu::PPU;
use crate::serial::Serial;

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
        self.cpu.step(&mut self.bus);
        self.bus.step(16);
        self.cpu.service_interrupts(&mut self.bus);
        // self.ppu.step(c, &mut self.bus);
        // self.timer.step(c, &mut self.bus);
        // self.apu.step(c);
        // cycles_this_frame += c;
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