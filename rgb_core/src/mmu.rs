use crate::apu::APU;
use crate::cartridge::Cartridge;
use crate::input::Joypad;
use crate::memory::{Memory, MemoryBus};
use crate::ppu::PPU;
use crate::serial::Serial;
use crate::timer::Timer;

pub struct MMU {
    cartridge: Box<dyn Cartridge>, // Cartridge memory
    apu: APU,                      // Audio Processing Unit
    ppu: PPU,                      // Graphics Processing Unit
    joypad: Joypad,                // Joypad input
    pub serial: Serial,            // Serial communication
    timer: Timer,                  // Timer
    

    // | 7  6  5 |   4    |   3    |   2   |  1  |   0    |
    // | ------- | ------ | ------ | ----- | --- | ------ |
    // |         | Joypad | Serial | Timer | LCD | VBlank |
    interrupt_flag: u8, // Interrupt flag
    interrupt_enable: u8,

    hram: [u8; 0x7F],   // High RAM
    wram: [u8; 0x8000], // Work RAM
    wram_bank: u8,      // Work RAM bank selector
}

enum MemoryRegion {
    Cartridge,
    PPU,
    WRAM,
    HRAM,
    IO,
    Unused,
    Invalid,
}

impl MMU {
    pub fn new(cartridge: Box<dyn Cartridge>) -> Self {
        MMU {
            cartridge,
            apu: APU::new(),
            ppu: PPU::new(),
            joypad: Joypad::new(),
            serial: Serial::new(),
            timer: Timer::new(),

            interrupt_flag: 0x00,
            interrupt_enable: 0x00,
            hram: [0x00; 0x7F],
            wram: [0x00; 0x8000],
            wram_bank: 1,
        }
    }

    pub fn step(&mut self, cycles: u16) {
        self.timer.step(cycles, &mut self.interrupt_flag);
    }
    
    fn get_memory_region(&self, address: u16) -> MemoryRegion {
        match address {
            0x0000..=0x7FFF => MemoryRegion::Cartridge,
            0x8000..=0x9FFF => MemoryRegion::PPU,
            0xA000..=0xBFFF => MemoryRegion::Cartridge,
            0xC000..=0xDFFF => MemoryRegion::WRAM,
            0xE000..=0xFDFF => MemoryRegion::WRAM,
            0xFE00..=0xFE9F => MemoryRegion::PPU,
            0xFEA0..=0xFEFF => MemoryRegion::Unused,
            0xFF00..=0xFF7F => MemoryRegion::IO,
            0xFF80..=0xFFFE => MemoryRegion::HRAM,
            0xFFFF => MemoryRegion::IO,
            _ => MemoryRegion::Invalid,
        }
    }

    fn read_wram(&self, address: u16) -> u8 {
        match address {
            0xC000..=0xcFFF => self.wram[address as usize - 0xC000],
            0xD000..=0xDFFF => {
                self.wram[address as usize - 0xD000 + 0x1000 * self.wram_bank as usize]
            }
            0xE000..=0xEFFF => self.wram[address as usize - 0xE000],
            0xF000..=0xFDFF => {
                self.wram[address as usize - 0xF000 + 0x1000 * self.wram_bank as usize]
            }
            _ => unreachable!("Invalid WRAM address: 0x{:04X}", address),
        }
    }

    fn write_wram(&mut self, address: u16, value: u8) {
        match address {
            0xC000..=0xcFFF => self.wram[address as usize - 0xC000] = value,
            0xD000..=0xDFFF => {
                self.wram[address as usize - 0xD000 + 0x1000 * self.wram_bank as usize] = value
            }
            0xE000..=0xEFFF => self.wram[address as usize - 0xE000] = value,
            0xF000..=0xFDFF => {
                self.wram[address as usize - 0xF000 + 0x1000 * self.wram_bank as usize] = value
            }
            _ => unreachable!("Invalid WRAM address: 0x{:04X}", address),
        }
    }

    fn read_io(&self, address: u16) -> u8 {
        match address {
            0xFF00 => self.joypad.state,
            0xFF01 => self.serial.sb,
            0xFF02 => self.serial.sc,
            0xFF04 => self.timer.div,
            0xFF05 => self.timer.tima,
            0xFF06 => self.timer.tma,
            0xFF07 => self.timer.tac,
            0xFF0F => self.interrupt_flag,
            0xFFFF => self.interrupt_enable,
            0xFF10..=0xFF3F => self.apu.read_byte(address),
            0xFF40..=0xFF4B => self.ppu.read_byte(address),
            _ => 0,
        }
    }

    fn write_io(&mut self, address: u16, value: u8) {
        match address {
            0xFF00 => self.joypad.state = (self.joypad.state & 0x0F) | (value & 0xF0),
            0xFF01 => self.serial.sb = value,
            0xFF02 => self.serial.write_control(value),
            0xFF04 => self.timer.div = 0,
            0xFF05 => self.timer.tima = value,
            0xFF06 => self.timer.tma = value,
            0xFF07 => self.timer.tac = value,
            0xFF0F => self.interrupt_flag = value,
            0xFFFF => self.interrupt_enable = value,
            0xFF10..=0xFF3F => self.apu.write_byte(address, value),
            0xFF40..=0xFF4B => self.ppu.write_byte(address, value),
            _ => (),
        }
    }
}

impl Memory for MMU {
    fn read_byte(&self, address: u16) -> u8 {
        match self.get_memory_region(address) {
            MemoryRegion::Cartridge => self.cartridge.read_byte(address),
            MemoryRegion::PPU => self.ppu.read_byte(address),
            MemoryRegion::WRAM => self.read_wram(address),
            MemoryRegion::HRAM => self.hram[address as usize - 0xFF80],
            MemoryRegion::IO => self.read_io(address),
            MemoryRegion::Unused => 0,
            MemoryRegion::Invalid => unreachable!("Invalid memory address: 0x{:04X}", address),
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        match self.get_memory_region(address) {
            MemoryRegion::Cartridge => self.cartridge.write_byte(address, value),
            MemoryRegion::PPU => self.ppu.write_byte(address, value),
            MemoryRegion::WRAM => self.write_wram(address, value),
            MemoryRegion::HRAM => self.hram[address as usize - 0xFF80] = value,
            MemoryRegion::IO => self.write_io(address, value),
            MemoryRegion::Unused => (),
            MemoryRegion::Invalid => unreachable!("Invalid memory address: 0x{:04X}", address),
        }
    }
}

impl MemoryBus for MMU {}