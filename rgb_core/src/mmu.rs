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

    hram: [u8; 0x7F],            // High RAM (0xFF80-0xFFFE)
    wram: [u8; 0x2000],          // Work RAM: 8 KiB, two fixed 4 KiB banks (DMG has no banking)
    boot_rom: Option<Box<[u8]>>, // 256-byte boot ROM image; None = skip boot ROM
    boot_rom_mapped: bool,       // true until the game writes 0xFF50 to unmap it

    // OAM DMA state machine.
    //
    // Writing to 0xFF46 initiates a transfer: the hardware copies 160 bytes
    // from (source × 0x100) into OAM, one byte per M-cycle, preceded by a
    // 1-cycle startup delay.  On real hardware the CPU bus is restricted to
    // HRAM during the transfer; games run their DMA handler ("trampoline")
    // from HRAM to work around this.  We model the cycle-accurate DMA copy
    // but do not currently enforce the CPU bus lockout.
    //
    // oam_dma_remaining counts the cycles left in the current transfer:
    //   161      = just started, startup delay (no bytes copied yet)
    //   160..=1  = transferring byte (160 − remaining)
    //   0        = idle, no transfer in progress
    oam_dma_remaining: u8,
    oam_dma_source: u16, // base address latched from the value written to 0xFF46
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
            oam_dma_remaining: 0,
            oam_dma_source: 0,
        }
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
            0xFF04 => self.devices.timer.div(),
            0xFF05 => self.devices.timer.tima,
            0xFF06 => self.devices.timer.tma,
            0xFF07 => self.devices.timer.tac,
            0xFF0F => self.interrupts.flag,
            0xFFFF => self.interrupts.enable,
            0xFF10..=0xFF3F => self.devices.apu.read_byte(address),
            0xFF40..=0xFF4B => self.devices.ppu.read_byte(address),
            // Unimplemented IO registers read as 0xFF on real DMG hardware.
            // Returning 0x00 here causes CGB+DMG games to misidentify themselves
            // as running on a CGB (e.g. reading KEY1 at 0xFF4D expects 0xFF on DMG).
            _ => 0xFF,
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
            0xFF04 => {
                // Resetting the system counter can create a falling edge on
                // both the TIMA bit tap and the APU frame-sequencer bit (12).
                let old = self.devices.timer.reset_divider();
                self.devices.apu.notify_div_reset(old);
            }
            0xFF05 => self.devices.timer.write_tima(value),
            0xFF06 => self.devices.timer.tma = value,
            0xFF07 => self.devices.timer.write_tac(value),
            0xFF0F => self.interrupts.flag = value,
            0xFFFF => self.interrupts.enable = value,
            0xFF10..=0xFF3F => self.devices.apu.write_byte(address, value),
            0xFF41 => {
                // Writing to STAT on DMG hardware fires a spurious STAT interrupt
                // as a side-effect of the write bus glitch (see PPU::write_stat).
                if self.devices.ppu.write_stat(value) {
                    self.interrupts.flag |= 0x02; // IF bit 1: LCD STAT
                }
            }
            0xFF46 => {
                // Writing to 0xFF46 starts an OAM DMA transfer (see the
                // oam_dma_remaining field and tick_m_cycle for the transfer
                // state machine).  The DMA register value is stored in the PPU
                // so it can be read back via 0xFF46.
                self.devices.ppu.write_byte(0xFF46, value);
                self.oam_dma_source = (value as u16) << 8;
                self.oam_dma_remaining = 161; // 1 startup cycle + 160 transfer cycles
            }
            0xFF40 | 0xFF42..=0xFF45 | 0xFF47..=0xFF4B => self.devices.ppu.write_byte(address, value),
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

impl MMU {
    /// Route a read to the appropriate device or memory region.
    ///
    /// Used directly by the OAM DMA hardware (which has its own bus) and by
    /// `Memory::read_byte`.  On real hardware the CPU path would enforce a
    /// bus restriction during DMA; that lockout is not yet implemented here.
    fn read_byte_dispatch(&self, address: u16) -> u8 {
        // The boot ROM shadows cartridge addresses 0x0000–0x00FF while mapped.
        if self.boot_rom_mapped
            && let Some(ref rom) = self.boot_rom
            && (address as usize) < rom.len()
        {
            return rom[address as usize];
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
}

impl Memory for MMU {
    fn read_byte(&self, address: u16) -> u8 {
        self.read_byte_dispatch(address)
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

impl MemoryBus for MMU {
    /// Advance all hardware devices by exactly one machine cycle (4 T-cycles).
    ///
    /// Called by the CPU after each M-cycle of instruction execution — opcode
    /// fetch, operand fetch, memory read, memory write, or internal delay.
    /// Stepping devices here, interleaved with the CPU's bus accesses, gives
    /// the timer, PPU, and APU the correct device state at every bus access.
    fn tick_m_cycle(&mut self) {
        // Capture the counter before the timer advances so the APU can detect
        // falling edges (frame sequencer bit 12) over this 4-T-cycle window.
        let counter_before = self.devices.timer.system_counter();
        self.devices.timer.step(4, &mut self.interrupts.flag);
        self.devices.ppu.step(4, &mut self.interrupts.flag);
        self.devices.apu.step(4, counter_before);

        // Advance an active OAM DMA transfer by one M-cycle.
        //
        // The first cycle (161 → 160) is a startup delay — the DMG hardware
        // needs one cycle to set up the transfer before bytes begin flowing.
        // Each subsequent cycle (160 → 1) copies one byte from the source
        // address into OAM.  The DMA reads via read_byte_dispatch, which uses
        // the DMA's own bus and is not subject to the CPU bus restriction.
        if self.oam_dma_remaining > 0 {
            self.oam_dma_remaining -= 1;
            if self.oam_dma_remaining < 160 {
                let byte_index = (159 - self.oam_dma_remaining) as usize;
                let src = self.oam_dma_source + byte_index as u16;
                let byte = self.read_byte_dispatch(src);
                self.devices.ppu.write_oam_direct(byte_index, byte);
            }
        }

        self.log_step(
            4,
            self.devices.timer.div(),
            self.devices.timer.tima,
            self.devices.timer.tma,
            self.devices.timer.tac,
            self.interrupts.flag,
            self.interrupts.enable,
        );
    }
}

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
