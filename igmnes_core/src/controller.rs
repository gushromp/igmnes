use crate::memory::{MemMapConfig, MemMapped};

#[derive(Clone, Copy)]
pub enum ControllerIndex {
    First = 0,
    Second = 1,
}

#[derive(Clone, Copy)]
pub enum ControllerButton {
    A = 0,
    B = 1,
    SELECT = 2,
    START = 3,
    UP = 4,
    DOWN = 5,
    LEFT = 6,
    RIGHT = 7,
}

pub type ControllerButtonState<'a> = &'a [ControllerButton];

#[derive(Clone, Copy, Default)]
pub struct Controller {
    pub button_state: u8,

    // While S (strobe) is high, the shift registers in the controllers are continuously reloaded
    // from the button states, and reading $4016/$4017 will keep returning the current state
    // of the first button (A)
    pub is_polling: bool,
    read_index: u8,

    mem_map_config: MemMapConfig,
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
        self.read_index = 0;
    }

    pub fn set_button_state(&mut self, state: ControllerButtonState) {
        let mut byte: u8 = 0;
        for button_state in state {
            byte |= 0b1 << *button_state as u8
        }
        self.button_state = byte;
    }
}

impl MemMapped for Controller {
    fn read(&mut self, _index: u16) -> u8 {
        if self.is_polling {
            self.button_state & 0b1
        } else {
            // After 8 bits are read, all subsequent bits will report 1 on a standard NES controller,
            // but third party and other controllers may report other values here.
            if self.read_index == 8 {
                0x1
            } else {
                let result = (self.button_state >> self.read_index) & 0b1;
                if self.is_mutating_read() {
                    self.read_index += 1;
                }
                result
            }
        }
    }

    fn read_range(&self, _range: std::ops::Range<u16>) -> &[u8] {
        unimplemented!()
    }

    fn write(&mut self, _index: u16, _byte: u8) {}

    fn is_mutating_read(&self) -> bool {
        self.mem_map_config.is_mutating_read
    }

    fn set_is_mutating_read(&mut self, is_mutating_read: bool) {
        self.mem_map_config.is_mutating_read = is_mutating_read;
    }
}
