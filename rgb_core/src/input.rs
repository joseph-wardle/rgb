// The Game Boy joypad has eight buttons arranged in a 2x4 matrix. The joypad state is
// represented by a single byte, where each bit corresponds to a button. A button being
// pressed is represented by a 0 bit, and a button being released is represented by a 1 bit.
//
// The joypad has two modes for reading button states:
// - Button mode: Reads the state of the Start, Select, B, and A buttons.
// - D-pad mode: Reads the state of the Down, Up, Left, and Right buttons.
//
// The mode is selected by writing to the upper nibble of the joypad state byte.

pub(crate) struct Joypad {
    pub(crate) state: u8,
}

impl Default for Joypad {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Copy, Clone, Debug)]
#[expect(
    dead_code,
    reason = "Input handling will select buttons once input is wired up"
)]
pub(crate) enum Button {
    Start,
    Select,
    B,
    A,
    Down,
    Up,
    Left,
    Right,
}

#[expect(
    dead_code,
    reason = "Joypad polling will use these helpers when input is implemented"
)]
impl Joypad {
    const SELECT_BUTTONS: u8 = 0b0010_0000;
    const SELECT_DPAD: u8 = 0b0001_0000;

    pub(crate) fn new() -> Self {
        Joypad { state: 0xFF }
    }

    pub(crate) fn write_select(&mut self, value: u8) {
        let previous = self.state;
        self.state = (self.state & 0x0F) | (value & 0xF0);
        let buttons_selected = (self.state & Joypad::SELECT_BUTTONS) == 0;
        let dpad_selected = (self.state & Joypad::SELECT_DPAD) == 0;
        self.log_select_updated(previous, self.state, value, buttons_selected, dpad_selected);
    }

    fn is_correct_select_pressed(&self, button: Button) -> bool {
        use Button::*;
        match button {
            Start | Select | B | A => self.state & Joypad::SELECT_BUTTONS == 0,
            Down | Up | Left | Right => self.state & Joypad::SELECT_DPAD == 0,
        }
    }

    pub(crate) fn is_pressed(&self, button: Button) -> bool {
        let button_mask = match button {
            Button::Start => 0b0000_1000,
            Button::Select => 0b0000_0100,
            Button::B => 0b0000_0010,
            Button::A => 0b0000_0001,
            Button::Down => 0b0000_1000,
            Button::Up => 0b0000_0100,
            Button::Left => 0b0000_0010,
            Button::Right => 0b0000_0010,
        };

        let pressed = self.is_correct_select_pressed(button) && (self.state & button_mask == 0);
        self.log_button_query(button, pressed);
        pressed
    }
}
