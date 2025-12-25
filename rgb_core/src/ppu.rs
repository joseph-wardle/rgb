use crate::memory::Memory;

#[expect(clippy::upper_case_acronyms)]
pub(crate) struct PPU {
    vram: [u8; 0x2000], // Video RAM
    oam: [u8; 0xA0],    // Object Attribute Memory
    lcd_control: u8,    // LCD Control Register
    lcd_status: u8,     // LCD Status Register
    scroll_y: u8,       // Scroll Y Register
    scroll_x: u8,       // Scroll X Register
    ly: u8,             // LY Register (current scanline)
    lyc: u8,            // LYC Register (LY Compare)
    dma: u8,            // DMA Transfer Register
    bg_palette: u8,     // Background Palette Data
    obj_palette0: u8,   // Object Palette 0 Data
    obj_palette1: u8,   // Object Palette 1 Data
    window_y: u8,       // Window Y Position
    window_x: u8,       // Window X Position
    cycle_counter: u32, // Accumulated PPU cycles since last VBlank
}

impl Default for PPU {
    fn default() -> Self {
        Self::new()
    }
}

impl PPU {
    pub(crate) fn new() -> Self {
        PPU {
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            lcd_control: 0,
            lcd_status: 0,
            scroll_y: 0,
            scroll_x: 0,
            ly: 0x90, //0,
            lyc: 0,
            dma: 0,
            bg_palette: 0,
            obj_palette0: 0,
            obj_palette1: 0,
            window_y: 0,
            window_x: 0,
            cycle_counter: 0,
        }
    }

    pub(crate) fn step(&mut self, cycles: u16, interrupt_flag: &mut u8) {
        const CYCLES_PER_FRAME: u32 = 70_224;
        const LCD_ENABLE: u8 = 0x80;

        if (self.lcd_control & LCD_ENABLE) == 0 {
            // When the LCD is disabled, the PPU stops ticking and no VBlank interrupts fire.
            self.cycle_counter = 0;
            return;
        }

        self.cycle_counter = self.cycle_counter.wrapping_add(cycles as u32);
        if self.cycle_counter >= CYCLES_PER_FRAME {
            self.cycle_counter -= CYCLES_PER_FRAME;
            #[cfg(debug_assertions)]
            eprintln!(
                "PPU VBlank: setting IF bit. Previous IF: {:02X}",
                *interrupt_flag
            );
            *interrupt_flag |= 0x01; // VBlank interrupt
        }
    }
}

impl Memory for PPU {
    fn read_byte(&self, address: u16) -> u8 {
        match address {
            0x8000..=0x9FFF => self.vram[(address - 0x8000) as usize],
            0xFE00..=0xFE9F => self.oam[(address - 0xFE00) as usize],
            0xFF40 => self.lcd_control,
            0xFF41 => self.lcd_status,
            0xFF42 => self.scroll_y,
            0xFF43 => self.scroll_x,
            0xFF44 => self.ly,
            0xFF45 => self.lyc,
            0xFF46 => self.dma,
            0xFF47 => self.bg_palette,
            0xFF48 => self.obj_palette0,
            0xFF49 => self.obj_palette1,
            0xFF4A => self.window_y,
            0xFF4B => self.window_x,
            _ => unreachable!("Invalid GPU address: 0x{:04X}", address),
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x8000..=0x9FFF => self.vram[(address - 0x8000) as usize] = value,
            0xFE00..=0xFE9F => self.oam[(address - 0xFE00) as usize] = value,
            0xFF40 => self.lcd_control = value,
            0xFF41 => self.lcd_status = value,
            0xFF42 => self.scroll_y = value,
            0xFF43 => self.scroll_x = value,
            0xFF44 => self.ly = value,
            0xFF45 => self.lyc = value,
            0xFF46 => self.dma = value,
            0xFF47 => self.bg_palette = value,
            0xFF48 => self.obj_palette0 = value,
            0xFF49 => self.obj_palette1 = value,
            0xFF4A => self.window_y = value,
            0xFF4B => self.window_x = value,
            _ => unreachable!("Invalid GPU address: 0x{:04X}", address),
        }
    }
}
