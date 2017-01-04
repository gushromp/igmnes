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
}

impl Mapper for NRom {
    fn read_prg(&self, index: usize) -> u8 {
        self.prg_rom_bytes[index]
    }

    fn write_prg(&mut self, index: usize, byte: u8) {
        self.prg_rom_bytes[index] = byte;
    }

    fn read_chr(&self, index: usize) -> u8 {
        self.chr_rom_bytes[index]
    }

    fn write_chr(&mut self, index: usize, byte: u8) {
        self.chr_rom_bytes[index] = byte;
    }
}