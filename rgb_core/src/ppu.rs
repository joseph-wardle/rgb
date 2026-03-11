//! DMG Picture Processing Unit.
//!
//! The PPU drives the 160×144 LCD. It operates on a fixed 456-dot scanline
//! grid across 154 lines (144 visible + 10 VBlank), cycling through four
//! modes per visible scanline:
//!
//!   Mode 2 (OAM Scan) →  80 dots — locate sprites for this scanline
//!   Mode 3 (Drawing)  → 172 dots — pixel pipeline pushes pixels to the LCD
//!   Mode 0 (HBlank)   → 204 dots — horizontal blank between scanlines
//!
//! During VBlank (scanlines 144–153) the PPU remains in Mode 1 for the
//! entire scanline. One full frame is 70,224 dots (~59.7 fps).
//!
//! This module implements the timing state machine: the LY counter, dot
//! counter, mode transitions, and VBlank/STAT interrupt generation. The
//! pixel pipeline is not yet implemented; the framebuffer reads all-zero
//! until Phase 3.

use crate::memory::Memory;

// ---------------------------------------------------------------------------
// Screen dimensions (public — consumed by the frontend)
// ---------------------------------------------------------------------------

/// The DMG screen is 160×144 pixels.
pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

// ---------------------------------------------------------------------------
// Timing constants
// ---------------------------------------------------------------------------

/// Every scanline — visible or VBlank — is exactly 456 T-cycles (dots).
const DOTS_PER_SCANLINE: u16 = 456;

/// Scanlines 0–143 produce pixels. Scanlines 144–153 are VBlank.
const VISIBLE_SCANLINES: u8 = 144;

/// 144 visible + 10 VBlank = 154 total scanlines per frame.
const TOTAL_SCANLINES: u8 = 154;

/// Mode 2 (OAM Scan): the PPU reads OAM for the first 80 dots of each
/// visible scanline to build the list of sprites that appear on this line.
const OAM_SCAN_DOTS: u16 = 80;

/// Mode 3 (Drawing): pixel pipeline runs for at least 172 dots.
/// The actual length grows with SCX scroll, sprite hits, and window
/// activation — modeled in Phase 3. Mode 0 (HBlank) fills the remainder:
///   456 − 80 − 172 = 204 dots minimum.
const DRAWING_DOTS: u16 = 172;

// ---------------------------------------------------------------------------
// Register bit masks — named for their register and function
// ---------------------------------------------------------------------------

// LCDC (FF40)
const LCDC_LCD_ENABLE: u8 = 1 << 7; // bit 7: LCD and PPU on/off

// STAT (FF41) — interrupt-enable bits (read/write by the CPU)
const STAT_LYC_INT:    u8 = 1 << 6; // fire STAT interrupt when LYC=LY
const STAT_OAM_INT:    u8 = 1 << 5; // fire STAT interrupt on Mode 2 entry
const STAT_VBLANK_INT: u8 = 1 << 4; // fire STAT interrupt on Mode 1 entry
const STAT_HBLANK_INT: u8 = 1 << 3; // fire STAT interrupt on Mode 0 entry

// STAT (FF41) — status bits (read-only; maintained by the PPU, not the CPU)
const STAT_LYC_FLAG:  u8 = 1 << 2;      // set when LY == LYC
const STAT_MODE_MASK: u8 = 0b0000_0011; // PPU mode in bits 0–1

// IF (FF0F) — interrupt flag bits
const IF_VBLANK: u8 = 1 << 0;
const IF_STAT:   u8 = 1 << 1;

// ---------------------------------------------------------------------------
// PPU mode
// ---------------------------------------------------------------------------

/// The four modes the PPU cycles through each frame.
///
/// The discriminant value matches the two-bit mode field in STAT bits 0–1,
/// so `mode as u8` can be stored directly into the register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    HBlank  = 0, // Mode 0: horizontal blank; CPU may freely access VRAM/OAM
    VBlank  = 1, // Mode 1: vertical blank (scanlines 144–153); VRAM accessible
    OamScan = 2, // Mode 2: PPU locks OAM and scans for sprites on this line
    Drawing = 3, // Mode 3: pixel pipeline is rendering; VRAM and OAM locked
}

// ---------------------------------------------------------------------------
// PPU
// ---------------------------------------------------------------------------

#[expect(clippy::upper_case_acronyms)]
pub(crate) struct PPU {
    // VRAM and OAM
    vram: [u8; 0x2000], // Video RAM        (8 KiB, 0x8000–0x9FFF)
    oam:  [u8; 0xA0],   // Object Attr Mem  (160 B, 0xFE00–0xFE9F)

    // LCD control and status
    lcd_control: u8, // LCDC (FF40): LCD/PPU enable, tile map/data area selects, etc.
    lcd_status:  u8, // STAT (FF41): mode bits (RO), LYC=LY flag (RO), int enables (RW)

    // Scroll and position
    scroll_y: u8, // SCY (FF42): background viewport Y
    scroll_x: u8, // SCX (FF43): background viewport X
    ly:       u8, // LY  (FF44): current scanline, 0–153
    lyc:      u8, // LYC (FF45): LY compare — triggers STAT interrupt when LY==LYC

    // DMA and palettes
    dma:          u8, // DMA  (FF46): initiates OAM DMA transfer (not yet modeled)
    bg_palette:   u8, // BGP  (FF47): background palette data
    obj_palette0: u8, // OBP0 (FF48): sprite palette 0
    obj_palette1: u8, // OBP1 (FF49): sprite palette 1

    // Window position
    window_y: u8, // WY (FF4A): window Y position
    window_x: u8, // WX (FF4B): window X position (pixel column = WX − 7)

    // Timing state machine
    dot:  u16,  // T-cycle position within the current scanline (0–455)
    mode: Mode, // current PPU mode; kept in sync with lcd_status bits 0–1

    // The STAT interrupt fires on the rising edge of a combined signal that is
    // the logical OR of all currently active and enabled STAT sources. This
    // field tracks the signal level from the previous step to detect that edge.
    stat_line: bool,

    // Output framebuffer: one byte per pixel, value 0–3 (DMG shade index).
    // Populated by the pixel pipeline (Phase 3); all-zero until then.
    framebuffer: [u8; SCREEN_WIDTH * SCREEN_HEIGHT],
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
            oam:  [0; 0xA0],
            lcd_control:  0,
            lcd_status:   0,
            scroll_y:     0,
            scroll_x:     0,
            ly:           0,
            lyc:          0,
            dma:          0,
            bg_palette:   0,
            obj_palette0: 0,
            obj_palette1: 0,
            window_y:     0,
            window_x:     0,
            dot:          0,
            mode:         Mode::OamScan, // (ly=0, dot=0) → Mode 2 per timing table
            stat_line:    false,
            framebuffer: [0; SCREEN_WIDTH * SCREEN_HEIGHT],
        }
    }

    pub(crate) fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    /// Advance the PPU by `cycles` T-cycles and raise any pending interrupts.
    pub(crate) fn step(&mut self, cycles: u16, interrupt_flag: &mut u8) {
        if !self.lcd_enabled() {
            // When the LCD is disabled the PPU halts, LY is held at 0, and
            // STAT reports Mode 0. On re-enable the PPU restarts from dot 0
            // of line 0. (Real hardware leaves the first frame blank; not
            // modeled here.)
            self.ly = 0;
            self.dot = 0;
            self.mode = Mode::HBlank;
            self.lcd_status &= !STAT_MODE_MASK; // mode bits → 0 (HBlank)
            return;
        }

        self.dot += cycles;

        // Each scanline is 456 dots. The longest SM83 instruction is 24
        // cycles, so at most one scanline boundary can be crossed per step.
        if self.dot >= DOTS_PER_SCANLINE {
            self.dot -= DOTS_PER_SCANLINE;
            self.ly = (self.ly + 1) % TOTAL_SCANLINES;
        }

        self.update_stat_and_interrupts(interrupt_flag);
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn lcd_enabled(&self) -> bool {
        (self.lcd_control & LCDC_LCD_ENABLE) != 0
    }

    /// Derive the PPU mode from the current scanline and dot position.
    ///
    /// This is the ground truth for mode; `self.mode` is kept in sync with
    /// it by `update_stat_and_interrupts` after every dot advance.
    fn current_mode(&self) -> Mode {
        if self.ly >= VISIBLE_SCANLINES {
            Mode::VBlank
        } else if self.dot < OAM_SCAN_DOTS {
            Mode::OamScan
        } else if self.dot < OAM_SCAN_DOTS + DRAWING_DOTS {
            Mode::Drawing
        } else {
            Mode::HBlank
        }
    }

    /// Synchronize the STAT register and fire interrupts for any transitions
    /// that occurred since the last step.
    fn update_stat_and_interrupts(&mut self, interrupt_flag: &mut u8) {
        let new_mode  = self.current_mode();
        let prev_mode = self.mode;

        // --- Maintain read-only STAT bits -----------------------------------
        //
        // Bits 0–1: PPU mode.
        self.lcd_status = (self.lcd_status & !STAT_MODE_MASK) | (new_mode as u8);

        // Bit 2: LYC=LY comparison. The hardware compares these registers
        // continuously; we update once per step (instruction granularity).
        if self.ly == self.lyc {
            self.lcd_status |= STAT_LYC_FLAG;
        } else {
            self.lcd_status &= !STAT_LYC_FLAG;
        }

        self.mode = new_mode;

        // --- Fire interrupts ------------------------------------------------
        //
        // VBlank fires unconditionally on the transition into Mode 1.
        // (Whether the CPU services it is controlled by IE bit 0, not here.)
        if prev_mode != Mode::VBlank && new_mode == Mode::VBlank {
            *interrupt_flag |= IF_VBLANK;
        }

        // The STAT interrupt fires on the rising edge of a combined signal
        // that is the logical OR of all active and enabled sources.
        // "STAT blocking": consecutive sources that keep the signal high do
        // not retrigger — there is no low-to-high transition to detect.
        let stat_line = self.stat_line_active();
        if stat_line && !self.stat_line {
            *interrupt_flag |= IF_STAT;
        }
        self.stat_line = stat_line;
    }

    /// Whether the STAT interrupt signal is currently active (high).
    ///
    /// The signal is the logical OR of all enabled interrupt sources.
    fn stat_line_active(&self) -> bool {
        let lyc_match = (self.lcd_status & STAT_LYC_FLAG) != 0;
        (self.mode == Mode::HBlank  && (self.lcd_status & STAT_HBLANK_INT) != 0)
            || (self.mode == Mode::VBlank  && (self.lcd_status & STAT_VBLANK_INT) != 0)
            || (self.mode == Mode::OamScan && (self.lcd_status & STAT_OAM_INT)    != 0)
            || (lyc_match                  && (self.lcd_status & STAT_LYC_INT)    != 0)
    }
}

// ---------------------------------------------------------------------------
// Memory map
// ---------------------------------------------------------------------------

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
            _ => unreachable!("PPU read: unmapped address {:#06X}", address),
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x8000..=0x9FFF => self.vram[(address - 0x8000) as usize] = value,
            0xFE00..=0xFE9F => self.oam[(address - 0xFE00) as usize] = value,
            0xFF40 => self.lcd_control = value,
            0xFF41 => {
                // Bits 3–6 are writable (interrupt enables). Bits 0–2 are
                // read-only (PPU mode and LYC=LY flag) — preserve them.
                // Note: a DMG hardware quirk causes a spurious STAT interrupt
                // when writing to this register during certain modes. Not yet
                // modeled.
                self.lcd_status = (self.lcd_status & 0b0000_0111) | (value & 0b0111_1000);
            }
            0xFF42 => self.scroll_y = value,
            0xFF43 => self.scroll_x = value,
            0xFF44 => {} // LY is read-only; CPU writes are ignored
            0xFF45 => self.lyc = value,
            0xFF46 => self.dma = value,
            0xFF47 => self.bg_palette = value,
            0xFF48 => self.obj_palette0 = value,
            0xFF49 => self.obj_palette1 = value,
            0xFF4A => self.window_y = value,
            0xFF4B => self.window_x = value,
            _ => unreachable!("PPU write: unmapped address {:#06X}", address),
        }
    }
}
