//! Host keyboard → DMG joypad mapping.
//!
//! Translates winit [`KeyCode`]s into [`Button`] presses that the emulator
//! core understands.  The mapping mirrors the physical Game Boy layout on a
//! standard QWERTY keyboard:
//!
//! | Host key      | DMG button |
//! |---------------|------------|
//! | Arrow keys    | D-pad      |
//! | Z             | B          |
//! | X             | A          |
//! | Enter         | Start      |
//! | Right Shift   | Select     |

use rgb_core::Button;
use winit::keyboard::KeyCode;

/// Static lookup table from host key to DMG button.
const KEY_MAP: &[(KeyCode, Button)] = &[
    (KeyCode::ArrowRight, Button::Right),
    (KeyCode::ArrowLeft, Button::Left),
    (KeyCode::ArrowUp, Button::Up),
    (KeyCode::ArrowDown, Button::Down),
    (KeyCode::KeyZ, Button::B),
    (KeyCode::KeyX, Button::A),
    (KeyCode::Enter, Button::Start),
    (KeyCode::ShiftRight, Button::Select),
];

/// Map a host key to a DMG button, returning `None` for unmapped keys.
pub fn map_key(key: KeyCode) -> Option<Button> {
    KEY_MAP
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, button)| *button)
}
