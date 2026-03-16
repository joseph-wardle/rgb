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
//! # What is implemented
//!
//! **Timing state machine** (Phase 2): LY counter, dot counter, mode
//! transitions, VBlank and STAT interrupt generation.
//!
//! **Background rendering** (Phase 3): tile data fetch, tile map lookup,
//! SCX/SCY viewport scroll, BGP palette application. Each scanline is
//! rendered in batch at the Drawing→HBlank transition. The output is a
//! `[u8; 160×144]` framebuffer of shade indices 0–3.
//!
//! **Sprites / OBJ layer** (Phase 5): OAM scan, 10-sprite-per-scanline limit,
//! sprite/BG priority, X/Y flip, 8×16 mode, OBP0/OBP1 palettes.
//!
//! **Window layer** (Phase 5): second BG plane anchored at (WX−7, WY) with its
//! own internal line counter and tile map select.
//!
//! **VRAM/OAM access restrictions**: the CPU reads 0xFF and writes are
//! silently dropped when accessing VRAM during Mode 3, or OAM during
//! Mode 2 or Mode 3 — matching DMG hardware. OAM DMA bypasses this
//! via [`PPU::write_oam_direct`].
//!
//! # What is not yet implemented
//!
//! - Mode 3 variable-length timing from SCX/sprite/window penalties

use std::cell::Cell;

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

/// Mode 3 (Drawing): the pixel pipeline runs for at least 172 dots.
/// The actual length grows with SCX fine-scroll, sprite hits, and window
/// activation (not yet modeled; see Phase 5). Mode 0 (HBlank) fills the
/// remainder: 456 − 80 − 172 = 204 dots minimum.
const DRAWING_DOTS: u16 = 172;

// ---------------------------------------------------------------------------
// Register bit masks — named for their register and function
// ---------------------------------------------------------------------------

// LCDC (FF40) — LCD Control register bit masks
const LCDC_LCD_ENABLE: u8 = 1 << 7; // bit 7: LCD and PPU on/off
const LCDC_WINDOW_MAP: u8 = 1 << 6; // bit 6: window tile map — 0 = 0x9800, 1 = 0x9C00
const LCDC_WINDOW_ENABLE: u8 = 1 << 5; // bit 5: window layer on/off
const LCDC_TILE_DATA: u8 = 1 << 4; // bit 4: tile data — 0 = 0x8800 signed, 1 = 0x8000 unsigned
const LCDC_BG_MAP: u8 = 1 << 3; // bit 3: BG tile map — 0 = 0x9800, 1 = 0x9C00
const LCDC_OBJ_SIZE: u8 = 1 << 2; // bit 2: sprite height — 0 = 8 px, 1 = 16 px
const LCDC_OBJ_ENABLE: u8 = 1 << 1; // bit 1: sprite (OBJ) layer on/off
const LCDC_BG_ENABLE: u8 = 1 << 0; // bit 0: BG/window on/off (0 = blank white on DMG)

// OAM attribute byte (byte 3 of each OAM entry) bit masks
const ATTR_BG_PRIORITY: u8 = 1 << 7; // 0 = sprite above BG/window; 1 = behind BG/window colors 1–3
const ATTR_Y_FLIP: u8 = 1 << 6; // vertical flip
const ATTR_X_FLIP: u8 = 1 << 5; // horizontal flip
const ATTR_PALETTE: u8 = 1 << 4; // palette select: 0 = OBP0, 1 = OBP1

// STAT (FF41) — interrupt-enable bits (read/write by the CPU)
const STAT_LYC_INT: u8 = 1 << 6; // fire STAT interrupt when LYC=LY
const STAT_OAM_INT: u8 = 1 << 5; // fire STAT interrupt on Mode 2 entry
const STAT_VBLANK_INT: u8 = 1 << 4; // fire STAT interrupt on Mode 1 entry
const STAT_HBLANK_INT: u8 = 1 << 3; // fire STAT interrupt on Mode 0 entry

// STAT (FF41) — status bits (read-only; maintained by the PPU, not the CPU)
const STAT_LYC_FLAG: u8 = 1 << 2; // set when LY == LYC
const STAT_MODE_MASK: u8 = 0b0000_0011; // PPU mode in bits 0–1

// IF (FF0F) — interrupt flag bits
const IF_VBLANK: u8 = 1 << 0;
const IF_STAT: u8 = 1 << 1;

// ---------------------------------------------------------------------------
// PPU mode
// ---------------------------------------------------------------------------

/// The four modes the PPU cycles through each frame.
///
/// The discriminant value matches the two-bit mode field in STAT bits 0–1,
/// so `mode as u8` can be stored directly into the register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    HBlank = 0,  // Mode 0: horizontal blank; CPU may freely access VRAM/OAM
    VBlank = 1,  // Mode 1: vertical blank (scanlines 144–153); VRAM accessible
    OamScan = 2, // Mode 2: PPU locks OAM and scans for sprites on this line
    Drawing = 3, // Mode 3: pixel pipeline is rendering; VRAM and OAM locked
}

// ---------------------------------------------------------------------------
// OAM corruption (Mode 2)
// ---------------------------------------------------------------------------

/// The OAM corruption pattern triggered by a CPU bus access during Mode 2.
///
/// On DMG hardware, certain CPU instructions accidentally drive an OAM-range
/// address on the external address bus while the PPU is scanning OAM.  This
/// confuses the PPU's OAM arbiter and corrupts the row it is currently reading.
///
/// `Read`  — triggered by a CPU read from the OAM address range (0xFE00–0xFE9F).
/// `Write` — triggered by a CPU write to the OAM address range.
///
/// A third "combined read+write" pattern exists for instructions that access
/// OAM twice (e.g. PUSH causes a write while the IDU also drives a read address),
/// but it is not yet modelled here.
enum OamCorruptionKind {
    Read,
    Write,
}

// ---------------------------------------------------------------------------
// Sprite (OBJ)
// ---------------------------------------------------------------------------

/// One decoded OAM entry.
///
/// The Game Boy stores 40 sprites as 4-byte records in OAM (0xFE00–0xFE9F).
/// Each record contains a raw Y and X with hardware biases: the sprite appears
/// at screen row `y − 16` and column `x − 8`, so a sprite fully off screen has
/// Y = 0 or X = 0, and one at the top-left corner has Y = 16, X = 8.
struct Sprite {
    y: u8,       // OAM Y (screen row  = y − 16)
    x: u8,       // OAM X (screen col  = x − 8)
    tile: u8,    // tile index; sprites always use the 0x8000 unsigned method
    attrs: u8,   // ATTR_* flags: priority, flip, palette
    oam_idx: u8, // position in OAM (0–39); breaks X ties — lower index wins
}

// ---------------------------------------------------------------------------
// PPU
// ---------------------------------------------------------------------------

#[expect(clippy::upper_case_acronyms)]
pub(crate) struct PPU {
    // VRAM and OAM
    vram: [u8; 0x2000], // Video RAM        (8 KiB, 0x8000–0x9FFF)
    oam: [u8; 0xA0],    // Object Attr Mem  (160 B, 0xFE00–0xFE9F)

    // LCD control and status
    lcd_control: u8, // LCDC (FF40): LCD/PPU enable, tile map/data area selects, etc.
    lcd_status: u8,  // STAT (FF41): mode bits (RO), LYC=LY flag (RO), int enables (RW)

    // Scroll and position
    scroll_y: u8, // SCY (FF42): background viewport Y
    scroll_x: u8, // SCX (FF43): background viewport X
    ly: u8,       // LY  (FF44): current scanline, 0–153
    lyc: u8,      // LYC (FF45): LY compare — triggers STAT interrupt when LY==LYC

    // DMA and palettes
    dma: u8,          // DMA  (FF46): last value written; actual transfer handled by MMU
    bg_palette: u8,   // BGP  (FF47): background palette data
    obj_palette0: u8, // OBP0 (FF48): sprite palette 0
    obj_palette1: u8, // OBP1 (FF49): sprite palette 1

    // Window position
    window_y: u8, // WY (FF4A): window Y position
    window_x: u8, // WX (FF4B): window X position (pixel column = WX − 7)

    // Window internal line counter.
    //
    // The window has its own scanline counter that is independent of LY. It
    // increments each time a scanline actually draws window pixels — it does
    // NOT increment on VBlank lines or on scanlines where LY < WY. This
    // keeps the window tile map aligned to the rows the window has actually
    // appeared on, allowing games to scroll WY mid-frame without glitching
    // the window tile fetch address. The counter resets to 0 at the start of
    // every new frame (when LY wraps from 153 back to 0).
    window_line: u8,

    // Timing state machine
    dot: u16,   // T-cycle position within the current scanline (0–455)
    mode: Mode, // current PPU mode; kept in sync with lcd_status bits 0–1

    // The STAT interrupt fires on the rising edge of a combined signal that is
    // the logical OR of all currently active and enabled STAT sources. This
    // field tracks the signal level from the previous step to detect that edge.
    stat_line: bool,

    // Output framebuffer: one byte per pixel, value 0–3 (DMG shade index).
    // 0 = white, 1 = light gray, 2 = dark gray, 3 = black.
    // The frontend maps these indices to actual colors at display time.
    framebuffer: [u8; SCREEN_WIDTH * SCREEN_HEIGHT],

    // OAM corruption state.
    //
    // When the CPU reads from OAM during Mode 2, the PPU's OAM arbiter is
    // confused and corrupts the row it is currently reading.  Because
    // `read_byte` takes `&self`, we record the pending corruption here via
    // `Cell` and commit it at the start of the next `step` call (which
    // takes `&mut self`), where the dot position is still correct.
    oam_cpu_read_pending: Cell<bool>,
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
            ly: 0,
            lyc: 0,
            dma: 0,
            bg_palette: 0,
            obj_palette0: 0,
            obj_palette1: 0,
            window_y: 0,
            window_x: 0,
            window_line: 0,
            dot: 0,
            mode: Mode::OamScan, // (ly=0, dot=0) → Mode 2 per timing table
            stat_line: false,
            framebuffer: [0; SCREEN_WIDTH * SCREEN_HEIGHT],
            oam_cpu_read_pending: Cell::new(false),
        }
    }

    /// Write directly into OAM, bypassing the mode-based access restriction.
    ///
    /// OAM DMA operates on the PPU's internal bus, not the CPU bus, so it
    /// can write to OAM at any time — even during Mode 2 or Mode 3.
    pub(crate) fn write_oam_direct(&mut self, offset: usize, value: u8) {
        self.oam[offset] = value;
    }

    // -----------------------------------------------------------------------
    // OAM corruption (Mode 2)
    // -----------------------------------------------------------------------

    /// Apply OAM corruption for the row the PPU is currently scanning.
    ///
    /// During Mode 2 (OAM scan) the PPU reads one 8-byte row per M-cycle.
    /// At dot `D` the current row is `D / 4` (rows 0–19).  Row 0 is immune;
    /// rows 1–19 are corrupted by the formula appropriate to `kind`.
    ///
    /// Variable names follow the Pan Docs notation for the corruption formulas:
    ///
    /// ```text
    ///   a  = word 0 of row N     (OAM bytes N×8 .. N×8+2)
    ///   b  = word 0 of row N−1  (OAM bytes (N−1)×8 .. (N−1)×8+2)
    ///   c  = word 2 of row N−1  (OAM bytes (N−1)×8+4 .. (N−1)×8+6)
    /// ```
    ///
    /// **Read corruption:** only word 0 of row N is changed: `b | (a & c)`
    ///
    /// **Write corruption:** word 0 = `((a ^ c) & (b ^ c)) ^ c`;
    /// words 1–3 of row N are overwritten with words 1–3 from row N−1.
    fn corrupt_oam(&mut self, kind: OamCorruptionKind) {
        let row = (self.dot / 4) as usize;
        if row == 0 || row > 19 {
            return; // row 0 is immune; guard against out-of-Mode-2 calls
        }

        let n = row * 8; // byte offset of row N in OAM
        let p = (row - 1) * 8; // byte offset of row N−1 in OAM

        // Read the three 16-bit words used by both formulas.
        let a = oam_word(&self.oam, n);
        let b = oam_word(&self.oam, p);
        let c = oam_word(&self.oam, p + 4);

        match kind {
            OamCorruptionKind::Read => {
                // Only word 0 of row N is affected.
                oam_write_word(&mut self.oam, n, b | (a & c));
            }
            OamCorruptionKind::Write => {
                // Word 0 uses the formula; words 1–3 are replaced with row N−1's words 1–3.
                oam_write_word(&mut self.oam, n, ((a ^ c) & (b ^ c)) ^ c);
                // copy_within is safe here: source (p+2..p+8) and dest (n+2..)
                // are separated by exactly 2 bytes (n = p + 8).
                self.oam.copy_within(p + 2..p + 8, n + 2);
            }
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
            self.oam_cpu_read_pending.take(); // clear any stale flag while LCD is off
            self.ly = 0;
            self.dot = 0;
            self.mode = Mode::HBlank;
            self.lcd_status &= !STAT_MODE_MASK; // mode bits → 0 (HBlank)
            return;
        }

        // Commit any OAM read-corruption flagged during the previous CPU instruction.
        // (The flag was set via Cell in read_byte; we apply it here where &mut self
        // is available and the dot position still reflects the access moment.)
        if self.oam_cpu_read_pending.take() {
            self.corrupt_oam(OamCorruptionKind::Read);
        }

        self.dot += cycles;

        // Each scanline is 456 dots. The longest SM83 instruction is 24
        // cycles, so at most one scanline boundary can be crossed per step.
        if self.dot >= DOTS_PER_SCANLINE {
            self.dot -= DOTS_PER_SCANLINE;
            self.ly = (self.ly + 1) % TOTAL_SCANLINES;
            if self.ly == 0 {
                self.window_line = 0; // new frame — reset the window's internal row counter
            }
        }

        let prev_mode = self.mode;
        self.update_stat_and_interrupts(interrupt_flag);

        // Render the scanline when Mode 3 ends. On hardware, the pixel pipeline
        // pushes pixels one-by-one throughout Mode 3; here we produce the whole
        // scanline at once at the Drawing→HBlank boundary. This is accurate for
        // games that do not scroll mid-scanline or rely on mid-scanline raster
        // effects (variable Mode 3 length from SCX/sprite/window penalties is
        // not yet modelled).
        if prev_mode == Mode::Drawing && self.mode == Mode::HBlank {
            self.render_scanline();
        }
    }

    // -----------------------------------------------------------------------
    // Timing helpers (Phase 2)
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
        let new_mode = self.current_mode();
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
        (self.mode == Mode::HBlank && (self.lcd_status & STAT_HBLANK_INT) != 0)
            || (self.mode == Mode::VBlank && (self.lcd_status & STAT_VBLANK_INT) != 0)
            || (self.mode == Mode::OamScan && (self.lcd_status & STAT_OAM_INT) != 0)
            || (lyc_match && (self.lcd_status & STAT_LYC_INT) != 0)
    }

    // -----------------------------------------------------------------------
    // Scanline rendering (Phase 3 + 5)
    // -----------------------------------------------------------------------

    /// Render the current scanline (`self.ly`) into the framebuffer.
    ///
    /// Called once per visible scanline, at the Drawing→HBlank boundary.
    ///
    /// Compositing order (back to front):
    ///   1. Background (or blank white if LCDC bit 0 is clear)
    ///   2. Window (Phase 5 — rendered by `render_window_scanline`)
    ///   3. Sprites (Phase 5 — composited with BG-priority checking)
    ///
    /// Two parallel buffers thread through all three layers:
    ///   `color_ids` — raw color index (0–3) used for sprite priority checks
    ///   `shades`    — final shade value (0–3) copied to the framebuffer
    fn render_scanline(&mut self) {
        let mut color_ids = [0u8; SCREEN_WIDTH]; // color indices for priority checks
        let mut shades = [0u8; SCREEN_WIDTH]; // output shades (default: white)

        // --- Background layer -------------------------------------------
        if (self.lcd_control & LCDC_BG_ENABLE) != 0 {
            // Precompute scanline-level values once before the per-pixel loop.
            let map_base = self.bg_tile_map_base();
            let map_y = self.ly.wrapping_add(self.scroll_y);
            let tile_row = (map_y / 8) as u16;
            let pixel_row = (map_y % 8) as u16;
            let scroll_x = self.scroll_x;
            let bg_palette = self.bg_palette;

            for x in 0..SCREEN_WIDTH as u8 {
                let map_x = x.wrapping_add(scroll_x);
                let tile_col = (map_x / 8) as u16;

                // The tile map is 32 tiles wide; each entry is one byte (tile ID).
                let tile_id = self.vram[(map_base + tile_row * 32 + tile_col) as usize];

                // Each tile is 16 bytes (2 bytes × 8 pixel rows).
                let tile_base = self.tile_data_offset(tile_id);
                let lo = self.vram[(tile_base + pixel_row * 2) as usize];
                let hi = self.vram[(tile_base + pixel_row * 2 + 1) as usize];

                // Bit 7 of each byte is the leftmost pixel of the row.
                let bit = 7 - (map_x % 8);
                let color_id = ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1);

                color_ids[x as usize] = color_id;
                shades[x as usize] = apply_palette(bg_palette, color_id);
            }
        }

        // --- Window layer (Phase 5) -------------------------------------
        self.render_window_scanline(&mut color_ids, &mut shades);

        // --- Sprite layer -----------------------------------------------
        if (self.lcd_control & LCDC_OBJ_ENABLE) != 0 {
            let sprites = self.scan_oam_for_scanline();
            let sprite_height: u8 = if (self.lcd_control & LCDC_OBJ_SIZE) != 0 {
                16
            } else {
                8
            };
            let obj_palette0 = self.obj_palette0;
            let obj_palette1 = self.obj_palette1;

            for sprite in &sprites {
                let palette = if (sprite.attrs & ATTR_PALETTE) != 0 {
                    obj_palette1
                } else {
                    obj_palette0
                };

                // Compute the pixel row within this sprite's tile(s) for this scanline.
                let row_within = self.ly.wrapping_add(16).wrapping_sub(sprite.y) as u16;
                let pixel_row = if (sprite.attrs & ATTR_Y_FLIP) != 0 {
                    (sprite_height as u16 - 1) - row_within
                } else {
                    row_within
                };

                // 8×16 sprites are two consecutive tiles. The top half uses the
                // even tile (bit 0 cleared) and the bottom half the odd tile.
                let tile = if sprite_height == 16 {
                    if pixel_row < 8 {
                        sprite.tile & 0xFE
                    } else {
                        sprite.tile | 0x01
                    }
                } else {
                    sprite.tile
                };

                // Sprites always use the 0x8000 unsigned addressing method.
                let tile_base = tile as u16 * 16;
                let tile_row = pixel_row % 8;
                let lo = self.vram[(tile_base + tile_row * 2) as usize];
                let hi = self.vram[(tile_base + tile_row * 2 + 1) as usize];

                // Iterate the 8 pixel columns of this sprite.
                for col in 0..8u8 {
                    let screen_x = sprite.x.wrapping_sub(8).wrapping_add(col) as usize;
                    if screen_x >= SCREEN_WIDTH {
                        continue;
                    }

                    // Bit 7 is leftmost; x-flip reverses the column order.
                    let bit = if (sprite.attrs & ATTR_X_FLIP) != 0 {
                        col
                    } else {
                        7 - col
                    };
                    let color_id = ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1);

                    if color_id == 0 {
                        continue; // color 0 is always transparent for sprites
                    }
                    if (sprite.attrs & ATTR_BG_PRIORITY) != 0 && color_ids[screen_x] != 0 {
                        continue; // BG/window colors 1–3 win over this sprite
                    }

                    shades[screen_x] = apply_palette(palette, color_id);
                }
            }
        }

        // Copy final shades into the framebuffer row for this scanline.
        let y = self.ly as usize;
        self.framebuffer[y * SCREEN_WIDTH..(y + 1) * SCREEN_WIDTH].copy_from_slice(&shades);
    }

    /// Collect the (up to 10) sprites that overlap the current scanline.
    ///
    /// The hardware scans OAM in index order and takes the first 10 hits.
    /// Sprites are returned sorted for draw order: lowest priority last so
    /// the final iteration leaves the highest-priority pixel on top.
    ///
    /// Priority rule: lower OAM X wins; ties broken by lower OAM index.
    /// Sorting descending by (X, oam_idx) puts the lowest-priority sprite
    /// first — each successive draw overwrites it, so the last-drawn
    /// (lowest X, lowest oam_idx) sprite wins.
    fn scan_oam_for_scanline(&self) -> Vec<Sprite> {
        let sprite_height: u8 = if (self.lcd_control & LCDC_OBJ_SIZE) != 0 {
            16
        } else {
            8
        };
        let ly = self.ly;

        let mut sprites: Vec<Sprite> = self
            .oam
            .chunks_exact(4)
            .enumerate()
            .filter_map(|(i, entry)| {
                let (y, x, tile, attrs) = (entry[0], entry[1], entry[2], entry[3]);
                // Sprite covers scanline ly when: 0 ≤ (ly + 16 − y) < height.
                // Wrapping subtraction gives a value ≥ height for non-overlapping
                // sprites (wrapping around u8 produces a large number).
                let row_within = ly.wrapping_add(16).wrapping_sub(y);
                if row_within < sprite_height {
                    Some(Sprite {
                        y,
                        x,
                        tile,
                        attrs,
                        oam_idx: i as u8,
                    })
                } else {
                    None
                }
            })
            .take(10)
            .collect();

        // Descending by (X, oam_idx): lowest priority first, highest priority last.
        sprites.sort_by(|a, b| b.x.cmp(&a.x).then(b.oam_idx.cmp(&a.oam_idx)));
        sprites
    }

    /// VRAM byte offset of the active background tile map.
    ///
    /// LCDC bit 3 selects between the two 32×32 tile maps:
    ///   0 → map at 0x9800 (VRAM offset 0x1800)
    ///   1 → map at 0x9C00 (VRAM offset 0x1C00)
    fn bg_tile_map_base(&self) -> u16 {
        if (self.lcd_control & LCDC_BG_MAP) != 0 {
            0x1C00
        } else {
            0x1800
        }
    }

    /// Render the window layer into `color_ids` and `shades` for this scanline,
    /// overwriting BG pixels wherever the window is visible.
    ///
    /// The window is a second background plane that covers the screen from
    /// (WX−7, WY) downward. It uses its own 32×32 tile map (selected by
    /// LCDC bit 6) and the same tile data addressing as the BG. The window
    /// always uses BGP for palette mapping.
    ///
    /// Unlike the BG, the window is not scrollable: column 0 of the window
    /// tile map always aligns to screen column WX−7. The window's internal
    /// line counter (`window_line`) tracks how many scanlines have actually
    /// drawn window pixels, so that the tile row advances correctly even if
    /// WY is changed mid-frame or the window is toggled.
    fn render_window_scanline(
        &mut self,
        color_ids: &mut [u8; SCREEN_WIDTH],
        shades: &mut [u8; SCREEN_WIDTH],
    ) {
        if (self.lcd_control & LCDC_WINDOW_ENABLE) == 0 {
            return;
        }
        // Window only appears on scanlines at or below its top edge.
        if self.ly < self.window_y {
            return;
        }
        // WX encodes the left screen column as WX−7. WX < 7 is off-screen;
        // WX > 166 (screen column 159) would also be off-screen.
        let screen_left = (self.window_x as usize).saturating_sub(7);
        if screen_left >= SCREEN_WIDTH {
            return;
        }

        let map_base = self.window_tile_map_base();
        let tile_row = (self.window_line / 8) as u16;
        let pixel_row = (self.window_line % 8) as u16;
        let bg_palette = self.bg_palette;

        for screen_x in screen_left..SCREEN_WIDTH {
            // Column within the window tile map (0-based from the window's left edge).
            let win_col = (screen_x - screen_left) as u8;
            let tile_col = (win_col / 8) as u16;

            let tile_id = self.vram[(map_base + tile_row * 32 + tile_col) as usize];
            let tile_base = self.tile_data_offset(tile_id);
            let lo = self.vram[(tile_base + pixel_row * 2) as usize];
            let hi = self.vram[(tile_base + pixel_row * 2 + 1) as usize];

            // Bit 7 of each byte is the leftmost pixel of the row.
            let bit = 7 - (win_col % 8);
            let color_id = ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1);

            color_ids[screen_x] = color_id;
            shades[screen_x] = apply_palette(bg_palette, color_id);
        }

        // The window line counter advances only on scanlines that draw window
        // pixels — not on VBlank lines or scanlines above WY.
        self.window_line += 1;
    }

    /// VRAM byte offset of the active window tile map.
    ///
    /// LCDC bit 6 selects between the two 32×32 tile maps:
    ///   0 → map at 0x9800 (VRAM offset 0x1800)
    ///   1 → map at 0x9C00 (VRAM offset 0x1C00)
    fn window_tile_map_base(&self) -> u16 {
        if (self.lcd_control & LCDC_WINDOW_MAP) != 0 {
            0x1C00
        } else {
            0x1800
        }
    }

    /// VRAM byte offset of the first byte of tile data for `tile_id`.
    ///
    /// LCDC bit 4 selects the addressing mode:
    ///
    ///   **0x8000 method** (bit 4 = 1): tile_id is an unsigned index (0–255).
    ///   Each tile is 16 bytes; tile 0 starts at VRAM offset 0x0000.
    ///
    ///   **0x8800 method** (bit 4 = 0): tile_id is a signed offset (−128–127).
    ///   The base pointer is 0x9000 (VRAM offset 0x1000). Tile IDs 0–127
    ///   land in block 2 (0x9000–0x97FF); IDs 128–255 (i.e. −128 to −1)
    ///   land in block 1 (0x8800–0x8FFF).
    fn tile_data_offset(&self, tile_id: u8) -> u16 {
        if (self.lcd_control & LCDC_TILE_DATA) != 0 {
            // 0x8000 method: unsigned, base at VRAM offset 0x0000.
            tile_id as u16 * 16
        } else {
            // 0x8800 method: signed, base at VRAM offset 0x1000.
            let signed_id = tile_id as i8 as i16;
            (0x1000 + signed_id * 16) as u16
        }
    }
}

// ---------------------------------------------------------------------------
// OAM word helpers
// ---------------------------------------------------------------------------

/// Read a little-endian 16-bit word from OAM at `offset`.
fn oam_word(oam: &[u8; 0xA0], offset: usize) -> u16 {
    oam[offset] as u16 | ((oam[offset + 1] as u16) << 8)
}

/// Write a little-endian 16-bit word to OAM at `offset`.
fn oam_write_word(oam: &mut [u8; 0xA0], offset: usize, value: u16) {
    oam[offset] = value as u8;
    oam[offset + 1] = (value >> 8) as u8;
}

// ---------------------------------------------------------------------------
// Palette lookup
// ---------------------------------------------------------------------------

/// Extract a DMG shade (0–3) from a palette register for the given color index.
///
/// DMG palette registers (BGP, OBP0, OBP1) pack four 2-bit shade values, one
/// per color index:
///
///   bits 7–6 → shade for color index 3
///   bits 5–4 → shade for color index 2
///   bits 3–2 → shade for color index 1
///   bits 1–0 → shade for color index 0
///
/// Shades: 0 = white, 1 = light gray, 2 = dark gray, 3 = black.
///
/// For sprites, color index 0 is transparent; the caller must check for it
/// before calling this function.
fn apply_palette(palette: u8, color_id: u8) -> u8 {
    (palette >> (color_id * 2)) & 0b11
}

// ---------------------------------------------------------------------------
// Memory map
// ---------------------------------------------------------------------------

impl Memory for PPU {
    fn read_byte(&self, address: u16) -> u8 {
        match address {
            // VRAM is locked during Mode 3 (Drawing); the CPU bus floats to 0xFF.
            0x8000..=0x9FFF => {
                if self.mode == Mode::Drawing {
                    0xFF
                } else {
                    self.vram[(address - 0x8000) as usize]
                }
            }
            // OAM is locked during Mode 2 (OAM Scan) and Mode 3 (Drawing).
            // During Mode 2 a CPU read also triggers OAM corruption: flag it via
            // Cell so it can be applied in the next `step` call (&mut self).
            0xFE00..=0xFE9F => match self.mode {
                Mode::OamScan => {
                    self.oam_cpu_read_pending.set(true);
                    0xFF
                }
                Mode::Drawing => 0xFF,
                _ => self.oam[(address - 0xFE00) as usize],
            },
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
            _ => 0xFF,
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            // VRAM is locked during Mode 3; CPU writes are silently ignored.
            0x8000..=0x9FFF => {
                if self.mode != Mode::Drawing {
                    self.vram[(address - 0x8000) as usize] = value;
                }
            }
            // OAM is locked during Mode 2 and 3.  A CPU write during Mode 2
            // is silently dropped but triggers OAM corruption; Mode 3 drops
            // the write without corruption.
            0xFE00..=0xFE9F => match self.mode {
                Mode::OamScan => self.corrupt_oam(OamCorruptionKind::Write),
                Mode::Drawing => {}
                _ => self.oam[(address - 0xFE00) as usize] = value,
            },
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
            _ => {}
        }
    }
}
