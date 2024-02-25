
#[derive(Debug, Clone, Copy)]
pub enum ControllerButton {
    A = 0,
    B = 1,
    SELECT = 2,
    START = 3,
    UP = 4,
    DOWN = 5,
    LEFT = 6,
    RIGHT = 7
}

pub type ControllerButtonState<'a> = &'a [ControllerButton];

#[derive(Debug, Clone, Copy, Default)]
pub struct Controller {
    pub button_state: u8,

    // While S (strobe) is high, the shift registers in the controllers are continuously reloaded
    // from the button states, and reading $4016/$4017 will keep returning the current state
    // of the first button (A)
    pub is_polling: bool,
    read_index: u8
}

impl Controller {
    pub fn new() -> Controller {
        Controller::default()
    }

    pub fn start_polling(&mut self) {
        self.is_polling = true;
    }

    pub fn stop_polling(&mut self) {
        self.is_polling = false;
    }

    pub fn set_button_state(&mut self, state: ControllerButtonState) {
        if !self.is_polling {
            return;
        }
        let mut byte: u8 = 0;
        for button_state in state {
            byte |= 0b1 << *button_state as u8
        }
        self.read_index = 0;
        self.button_state = byte;
    }

    pub fn read(&mut self) -> u8 {
        if self.is_polling {
            self.button_state & 0b1
        } else {
            // After 8 bits are read, all subsequent bits will report 1 on a standard NES controller,
            // but third party and other controllers may report other values here.
            if self.read_index == 8 {
                0b0
            } else {
                let result = (self.button_state >> self.read_index) & 0b1;
                self.read_index += 1;
                result
            }
        }
    }
}