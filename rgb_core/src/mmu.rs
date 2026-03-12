use crate::apu::APU;
use crate::cartridge::Cartridge;
use crate::input::{Button, Joypad};
use crate::memory::{Memory, MemoryBus};
use crate::ppu::PPU;
use crate::serial::Serial;
use crate::timer::Timer;

// Bus/Devices: MMU owns Devices + RAM + Interrupts.
struct Devices {
    cartridge: Box<dyn Cartridge>, // Cartridge memory
    apu: APU,                      // Audio Processing Unit
    ppu: PPU,                      // Graphics Processing Unit
    joypad: Joypad,                // Joypad input
    serial: Serial,                // Serial communication
    timer: Timer,                  // Timer
}

impl Devices {
    fn new(cartridge: Box<dyn Cartridge>) -> Self {
        Self {
            cartridge,
            apu: APU::new(),
            ppu: PPU::new(),
            joypad: Joypad::new(),
            serial: Serial::new(),
            timer: Timer::new(),
        }
    }
}

struct Interrupts {
    // | 7  6  5 |   4    |   3    |   2   |  1  |   0    |
    // | ------- | ------ | ------ | ----- | --- | ------ |
    // |         | Joypad | Serial | Timer | LCD | VBlank |
    flag: u8,
    enable: u8,
}

impl Interrupts {
    fn new() -> Self {
        Self {
            flag: 0x00,
            enable: 0x00,
        }
    }
}

#[expect(clippy::upper_case_acronyms)]
pub(crate) struct MMU {
    devices: Devices,
    interrupts: Interrupts,

    hram: [u8; 0x7F],   // High RAM (0xFF80-0xFFFE)
    wram: [u8; 0x2000], // Work RAM: 8 KiB, two fixed 4 KiB banks (DMG has no banking)
    boot_rom: Option<Box<[u8]>>,  // 256-byte boot ROM image; None = skip boot ROM
    boot_rom_mapped: bool,         // true until the game writes 0xFF50 to unmap it
}

#[expect(clippy::upper_case_acronyms)]
enum MemoryRegion {
    Cartridge,
    PPU,
    WRAM,
    HRAM,
    IO,
    Unused,
}

impl MMU {
    pub(crate) fn new(cartridge: Box<dyn Cartridge>, boot_rom: Option<Box<[u8]>>) -> Self {
        MMU {
            devices: Devices::new(cartridge),
            interrupts: Interrupts::new(),
            hram: [0x00; 0x7F],
            wram: [0x00; 0x2000],
            boot_rom_mapped: boot_rom.is_some(),
            boot_rom,
        }
    }

    pub(crate) fn step(&mut self, cycles: u16) {
        self.devices.timer.step(cycles, &mut self.interrupts.flag);
        self.devices.ppu.step(cycles, &mut self.interrupts.flag);
        self.devices.apu.step(cycles);
        self.log_step(
            cycles,
            self.devices.timer.div,
            self.devices.timer.tima,
            self.devices.timer.tma,
            self.devices.timer.tac,
            self.interrupts.flag,
            self.interrupts.enable,
        );
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
        }
    }

    fn read_wram(&self, address: u16) -> u8 {
        // The DMG has 8 KiB of WRAM with two fixed 4 KiB banks. Echo RAM
        // (0xE000-0xFDFF) mirrors the same 8 KiB. Masking the low 13 bits
        // maps all four address ranges directly into the array.
        self.wram[(address & 0x1FFF) as usize]
    }

    fn write_wram(&mut self, address: u16, value: u8) {
        self.wram[(address & 0x1FFF) as usize] = value;
    }

    fn read_io(&self, address: u16) -> u8 {
        let value = match address {
            0xFF00 => self.devices.joypad.read(),
            0xFF01 => self.devices.serial.sb,
            0xFF02 => self.devices.serial.sc,
            0xFF04 => self.devices.timer.div,
            0xFF05 => self.devices.timer.tima,
            0xFF06 => self.devices.timer.tma,
            0xFF07 => self.devices.timer.tac,
            0xFF0F => self.interrupts.flag,
            0xFFFF => self.interrupts.enable,
            0xFF10..=0xFF3F => self.devices.apu.read_byte(address),
            0xFF40..=0xFF4B => self.devices.ppu.read_byte(address),
            _ => 0,
        };
        self.log_io_read(address, value);
        value
    }

    fn write_io(&mut self, address: u16, value: u8) {
        match address {
            0xFF00 => self.devices.joypad.write_select(value),
            0xFF01 => self.devices.serial.write_data(value),
            0xFF02 => {
                if self.devices.serial.write_control(value) {
                    self.interrupts.flag |= 0x08;
                }
            }
            0xFF04 => self.devices.timer.reset_divider(),
            0xFF05 => self.devices.timer.tima = value,
            0xFF06 => self.devices.timer.tma = value,
            0xFF07 => self.devices.timer.tac = value,
            0xFF0F => self.interrupts.flag = value,
            0xFFFF => self.interrupts.enable = value,
            0xFF10..=0xFF3F => self.devices.apu.write_byte(address, value),
            0xFF46 => {
                // OAM DMA: copy 160 bytes from (value × 0x100) into OAM.
                // On hardware this locks the CPU bus for 160 µs; here we model
                // it as an instantaneous copy, which is sufficient for games
                // that wait for the transfer to complete before accessing OAM.
                // The DMA uses the PPU's internal bus, bypassing the mode-based
                // access restriction that applies to the CPU bus.
                let source = (value as u16) << 8;
                for i in 0..0xA0u16 {
                    let byte = self.read_byte(source + i);
                    self.devices.ppu.write_oam_direct(i as usize, byte);
                }
                self.devices.ppu.write_byte(0xFF46, value); // record for reads
            }
            0xFF40..=0xFF45 | 0xFF47..=0xFF4B => self.devices.ppu.write_byte(address, value),
            0xFF50 => {
                // Writing any value to 0xFF50 unmaps the boot ROM.  On real
                // hardware the boot ROM writes 0x01 here as its final act,
                // handing control to the cartridge at 0x0100.
                self.boot_rom_mapped = false;
            }
            _ => (),
        }
        self.log_io_write(address, value);
    }
}

impl Memory for MMU {
    fn read_byte(&self, address: u16) -> u8 {
        // When the boot ROM is mapped it shadows cartridge addresses 0x0000–0x00FF.
        // The boot ROM unmaps itself by writing to 0xFF50.
        if self.boot_rom_mapped {
            if let Some(ref rom) = self.boot_rom {
                if (address as usize) < rom.len() {
                    return rom[address as usize];
                }
            }
        }

        match self.get_memory_region(address) {
            MemoryRegion::Cartridge => self.devices.cartridge.read_byte(address),
            MemoryRegion::PPU => self.devices.ppu.read_byte(address),
            MemoryRegion::WRAM => self.read_wram(address),
            MemoryRegion::HRAM => self.hram[address as usize - 0xFF80],
            MemoryRegion::IO => self.read_io(address),
            MemoryRegion::Unused => 0,
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        match self.get_memory_region(address) {
            MemoryRegion::Cartridge => self.devices.cartridge.write_byte(address, value),
            MemoryRegion::PPU => self.devices.ppu.write_byte(address, value),
            MemoryRegion::WRAM => self.write_wram(address, value),
            MemoryRegion::HRAM => self.hram[address as usize - 0xFF80] = value,
            MemoryRegion::IO => self.write_io(address, value),
            MemoryRegion::Unused => (),
        }
    }
}

impl MemoryBus for MMU {}

impl MMU {
    pub(crate) fn serial(&self) -> &Serial {
        &self.devices.serial
    }

    /// Drain and return all audio samples generated since the last call.
    /// Returns interleaved stereo f32 pairs (left, right, …) in −1.0..=+1.0.
    pub(crate) fn drain_samples(&mut self) -> Vec<f32> {
        self.devices.apu.drain_samples()
    }

    /// Battery-backed RAM contents for writing to a `.sav` file, or `None`
    /// if the cartridge has no battery.
    pub(crate) fn save_data(&self) -> Option<&[u8]> {
        self.devices.cartridge.save_data()
    }

    /// Restore RAM from a `.sav` file loaded at startup.
    pub(crate) fn load_save_data(&mut self, data: &[u8]) {
        self.devices.cartridge.load_save_data(data);
    }

    pub(crate) fn framebuffer(&self) -> &[u8] {
        self.devices.ppu.framebuffer()
    }

    /// Forward a button press from the host to the joypad. Sets the
    /// joypad interrupt flag (IF bit 4) if the button's row is currently
    /// selected and the button was previously released.
    pub(crate) fn press_button(&mut self, button: Button) {
        if self.devices.joypad.press(button) {
            self.interrupts.flag |= 0x10; // IF bit 4: joypad interrupt
        }
    }

    /// Forward a button release from the host to the joypad.
    /// No interrupt fires on release.
    pub(crate) fn release_button(&mut self, button: Button) {
        self.devices.joypad.release(button);
    }
}
