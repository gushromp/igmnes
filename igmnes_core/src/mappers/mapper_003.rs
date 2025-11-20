use crate::mappers::{CpuMapper, PpuMapper};
use crate::memory::{MemMapped, Ram};
use crate::rom::{MirroringMode, Rom};
use std::ops::Range;

const BANK_SIZE_BYTES: usize = 8_192;

#[derive(Clone)]
pub struct CNROM {
    vram: Ram,
    prg_rom_bytes: Vec<u8>,
    chr_rom_bytes: Vec<u8>,
    mirroring_mode: MirroringMode,

    bank_index: usize,
}

impl CNROM {
    pub fn new(rom: &Rom) -> CNROM {
        let prg_rom_bytes = rom.prg_rom_bytes.clone(); // TODO use references!
        let chr_rom_bytes = rom.chr_rom_bytes.clone();
        CNROM {
            vram: Ram::default(),
            prg_rom_bytes,
            chr_rom_bytes,
            mirroring_mode: rom.header.mirroring_mode,
            bank_index: 0,
        }
    }

    fn get_prg_rom_index(&self, index: u16) -> usize {
        // Banks
        //      PRG ROM size: 16 KiB or 32 KiB
        //      PRG ROM bank size: Not bankswitched
        (index - 0x8000) as usize
    }

    fn get_chr_rom_index(&self, index: u16) -> usize {
        // Banks
        //      CHR capacity: Up to 2048 KiB ROM
        //      CHR bank size: 8 KiB
        (self.bank_index * BANK_SIZE_BYTES) + index as usize
    }

    fn select_bank(&mut self, index: u16, byte: u8) {
        let byte_in_rom = self.read_prg_rom(index);
        let resulting_byte = (byte & 0b11) & byte_in_rom;
        self.bank_index = resulting_byte as usize;
    }
}

impl CpuMapper for CNROM {
    fn read_prg_rom(&self, index: u16) -> u8 {
        let index = self.get_prg_rom_index(index);
        self.prg_rom_bytes[index]
    }

    fn read_prg_ram(&self, _index: u16) -> u8 {
        0
    }

    fn write_prg_ram(&mut self, _index: u16, _byte: u8) {}
}

impl PpuMapper for CNROM {
    fn read_chr_rom(&self, index: u16) -> u8 {
        let index = self.get_chr_rom_index(index);
        self.chr_rom_bytes[index]
    }

    fn read_chr_rom_range(&self, range: Range<u16>) -> Vec<u8> {
        let adjusted_range_start_index = self.get_chr_rom_index(range.start);
        let adjusted_range = adjusted_range_start_index..adjusted_range_start_index + range.len();
        self.chr_rom_bytes[adjusted_range].to_vec()
    }

    fn read_chr_ram(&self, index: u16) -> u8 {
        panic!(
            "Attempted read from non-existent CHR RAM index (untranslated): 0x{:X}",
            index
        )
    }

    fn read_chr_ram_range(&self, range: Range<u16>) -> Vec<u8> {
        panic!(
            "Attempted read from non-existent CHR RAM range (untranslated): 0x{:?}",
            range
        )
    }

    fn write_chr_ram(&mut self, _index: u16, _byte: u8) {}

    fn get_mirrored_index(&self, index: u16) -> u16 {
        let index = index - 0x2000;
        match self.mirroring_mode {
            MirroringMode::Horizontal => ((index / 0x800) * 0x400) + (index % 0x400),
            MirroringMode::Vertical => index % 0x800,
        }
    }
}

impl MemMapped for CNROM {
    fn read(&mut self, index: u16) -> u8 {
        match index {
            0..=0x1FFF => self.read_chr_rom(index),
            0x2000..=0x2FFF => {
                let index = self.get_mirrored_index(index);
                self.vram.read(index)
            }
            0x8000..=0xFFFF => self.read_prg_rom(index),
            _ => {
                println!("Attempted read from unmapped address: 0x{:X}", index);
                0
            }
        }
    }

    fn write(&mut self, index: u16, byte: u8) {
        match index {
            0..=0x1FFF => self.write_chr_ram(index, byte),
            0x2000..=0x2FFF => {
                let index = self.get_mirrored_index(index);
                self.vram.write(index, byte)
            }
            0x8000..=0xFFFF => self.select_bank(index, byte),
            _ => (),
        }
    }

    fn read_range(&self, range: Range<u16>) -> Vec<u8> {
        match range.start {
            0..=0x1FFF => self.read_chr_rom_range(range),
            _ => unimplemented!(),
        }
    }
}
