use std::default::Default;
use core::rom::Rom;
use core::apu::Apu;

const RAM_SIZE: usize = 0x800;

pub trait MemMapped {
    fn read(&self, u16: u16) -> u8;
    fn write(&mut self, index: u16, byte: u8);
}

pub struct Ram {
    ram: [u8; RAM_SIZE],
}

impl Default for Ram {
    fn default() -> Ram {
        Ram::new()
    }
}


impl Ram {
    pub fn new() -> Ram {
        Ram {
            ram: [0; RAM_SIZE]
        }
    }
}

impl MemMapped for Ram {
    fn read(&self, index: u16) -> u8 {
        self.ram[index as usize]
    }

    fn write(&mut self, index: u16, byte: u8) {
        self.ram[index as usize] = byte;
    }
}

#[derive(Default)]
pub struct MemMap {
    rom: Rom,
    ram: Ram,
    apu: Apu,

}

impl MemMap {
    pub fn new(rom: Rom) -> MemMap {
        MemMap {
            rom: rom,
            ram: Ram::new(),
            apu: Apu,
        }
    }
}



