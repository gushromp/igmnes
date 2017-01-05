use core::mappers::Mapper;
use core::rom::Rom;

pub struct NRom {
    prg_rom_bytes: Vec<u8>,
    chr_rom_bytes: Vec<u8>,
}

impl NRom {
    pub fn new(rom: &Rom) -> NRom {
        let prg_rom_bytes = rom.prg_rom_bytes.clone(); // TODO use references!
        let chr_rom_bytes = rom.chr_rom_bytes.clone();

        NRom {
            prg_rom_bytes: prg_rom_bytes,
            chr_rom_bytes: chr_rom_bytes,
        }
    }

    // Mirror if prg size is smaller than 32k
    fn get_correct_index(&self, index: u16) -> usize {
        // NROM starts mapping at 0x6000, so there's nothing mapped between 0x4020 and 0x6000
        let index = index - 0x8000;
        if index > 0xBFFF && self.prg_rom_bytes.len() <= 0x8000 {
            (index % 0x4000) as usize
        }
        else {
            index as usize
        }
    }
}



impl Mapper for NRom {
    fn read_prg(&self, index: u16) -> u8 {
        let index = self.get_correct_index(index);
        self.prg_rom_bytes[index]
    }

    fn write_prg(&mut self, index: u16, byte: u8) {
        let index = self.get_correct_index(index);
        self.prg_rom_bytes[index] = byte;
    }

    fn read_chr(&self, index: u16) -> u8 {
        let index = self.get_correct_index(index);
        self.chr_rom_bytes[index]
    }

    fn write_chr(&mut self, index: u16, byte: u8) {
        let index = self.get_correct_index(index);
        self.chr_rom_bytes[index] = byte;
    }
}