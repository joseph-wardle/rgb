//! DMG joypad input.
//!
//! The joypad register (FF00 / P1) exposes an 8-button 2×4 matrix. The CPU
//! selects which row to read by writing to bits 4–5, then reads the button
//! states from bits 0–3 (active-low: 0 = pressed, 1 = released).
//!
//! ```text
//!   bit 5 (P15) = 0 → expose direction pad row  (Down / Up / Left / Right)
//!   bit 4 (P14) = 0 → expose action button row  (Start / Select / B / A)
//!   bits 3–0          button state for selected row(s), active-low
//! ```
//!
//! If both rows are selected simultaneously the lower nibble is the AND of
//! both rows, so a bit is 0 if *either* row has that bit pressed.

/// The eight DMG buttons, named as they appear on the hardware.
#[derive(Copy, Clone, Debug)]
pub enum Button {
    // Direction pad
    Right,
    Left,
    Up,
    Down,
    // Action buttons
    A,
    B,
    Select,
    Start,
}

pub(crate) struct Joypad {
    // The CPU writes bits 4–5 to choose which row(s) to expose on bits 0–3.
    select: u8,   // bit 5 = P15 (dpad row), bit 4 = P14 (button row); active-low select

    // Button states stored per row, independent of what is currently selected.
    // Both rows are always maintained so switching the select bits reads correctly
    // without re-pressing buttons.
    dpad:    u8,  // bits 3–0: Down / Up / Left / Right, active-low (0 = pressed)
    buttons: u8,  // bits 3–0: Start / Select / B / A,   active-low (0 = pressed)
}

// Select-bit masks for each row (written to bits 5-4 of FF00).
const SELECT_DPAD:    u8 = 1 << 5; // P15: 0 = direction pad row is readable
const SELECT_BUTTONS: u8 = 1 << 4; // P14: 0 = action button row is readable

impl Default for Joypad {
    fn default() -> Self {
        Self::new()
    }
}

impl Joypad {
    pub(crate) fn new() -> Self {
        Joypad {
            select:  0x30, // both rows deselected at power-on (bits 4-5 = 1)
            dpad:    0x0F, // all directions released
            buttons: 0x0F, // all buttons released
        }
    }

    /// Read the current FF00 value.
    ///
    /// Bits 7–6 are unused and always read 1. Bits 5–4 reflect the select
    /// lines. Bits 3–0 are the AND of all enabled rows (active-low).
    pub(crate) fn read(&self) -> u8 {
        let mut lower = 0x0F; // assume all released
        if (self.select & SELECT_DPAD) == 0 {
            lower &= self.dpad;
        }
        if (self.select & SELECT_BUTTONS) == 0 {
            lower &= self.buttons;
        }
        0xC0 | self.select | lower
    }

    /// Handle a CPU write to FF00. Only bits 4–5 (the select lines) are
    /// writable; the lower nibble is read-only hardware output.
    pub(crate) fn write_select(&mut self, value: u8) {
        let previous = self.read();
        self.select = value & 0x30;
        self.log_select_updated(previous, self.read(), value);
    }

    /// Record a button press from the host. Sets the button's bit to 0
    /// (active-low). Returns `true` when the joypad interrupt should fire —
    /// i.e. the button was previously released *and* its row is currently
    /// selected by the CPU.
    pub(crate) fn press(&mut self, button: Button) -> bool {
        let mask = Self::button_mask(button);
        match button {
            Button::Down | Button::Up | Button::Left | Button::Right => {
                let fires = (self.dpad & mask) != 0 && (self.select & SELECT_DPAD) == 0;
                self.dpad &= !mask;
                fires
            }
            Button::Start | Button::Select | Button::B | Button::A => {
                let fires = (self.buttons & mask) != 0 && (self.select & SELECT_BUTTONS) == 0;
                self.buttons &= !mask;
                fires
            }
        }
    }

    /// Record a button release from the host. Sets the button's bit back to
    /// 1 (active-low released). No interrupt fires on release.
    pub(crate) fn release(&mut self, button: Button) {
        let mask = Self::button_mask(button);
        match button {
            Button::Down | Button::Up | Button::Left | Button::Right => self.dpad    |= mask,
            Button::Start | Button::Select | Button::B | Button::A   => self.buttons |= mask,
        }
    }

    /// Bit position within its row for each button.
    ///
    /// Both rows share the same bit layout:
    ///   bit 3 = Down  / Start
    ///   bit 2 = Up    / Select
    ///   bit 1 = Left  / B
    ///   bit 0 = Right / A
    fn button_mask(button: Button) -> u8 {
        match button {
            Button::Down  | Button::Start  => 1 << 3,
            Button::Up    | Button::Select => 1 << 2,
            Button::Left  | Button::B      => 1 << 1,
            Button::Right | Button::A      => 1 << 0,
        }
    }
}
