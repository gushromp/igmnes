
const RAM_SIZE: usize = 0x800;

pub trait MemMap {
    fn read(&self, index: usize) -> u8;
    fn write(&mut self, index: usize, byte: u8);
} 

pub enum AddressingMode {

}

pub struct Memory {
    ram: [u8; RAM_SIZE],
}

impl Memory {
    pub fn new() -> Memory {
        Memory {
            ram: [0; RAM_SIZE]
        }
    }
}

impl MemMap for Memory {
    fn read(&self, index: usize) -> u8 {
        self.ram[index]
    }

    fn write(&mut self, index: usize, byte: u8) {
        self.ram[index] = byte;
    }
}

