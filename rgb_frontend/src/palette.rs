//! DMG palette definitions.
//!
//! The PPU produces shade indices 0–3 for each pixel.  A [`Palette`] maps
//! those indices to RGBA colours that the renderer writes into the pixel
//! buffer.  Two built-in palettes are provided; adding more is as simple
//! as defining another `const Palette`.

/// Four RGBA colours, one per shade index.
///
/// Index 0 is the lightest shade (background/off), index 3 is the darkest.
/// Each entry is `[R, G, B, A]` with alpha always 0xFF.
#[derive(Clone, Copy)]
pub struct Palette {
    colors: [[u8; 4]; 4],
}

impl Palette {
    /// Return the RGBA bytes for a shade index (0–3).
    #[inline]
    pub fn rgba(&self, shade: u8) -> [u8; 4] {
        self.colors[shade as usize & 0x03]
    }
}

/// Classic DMG green — the colours most people associate with the original
/// Game Boy screen.
pub const CLASSIC_GREEN: Palette = Palette {
    colors: [
        [0xE0, 0xF8, 0xD0, 0xFF], // shade 0: lightest (off-white green)
        [0x88, 0xC0, 0x70, 0xFF], // shade 1: light
        [0x34, 0x68, 0x56, 0xFF], // shade 2: dark
        [0x08, 0x18, 0x20, 0xFF], // shade 3: darkest
    ],
};

/// Neutral grayscale — useful for debugging or as an accessibility option.
pub const GRAYSCALE: Palette = Palette {
    colors: [
        [0xFF, 0xFF, 0xFF, 0xFF], // shade 0: white
        [0xAA, 0xAA, 0xAA, 0xFF], // shade 1: light grey
        [0x55, 0x55, 0x55, 0xFF], // shade 2: dark grey
        [0x00, 0x00, 0x00, 0xFF], // shade 3: black
    ],
};
