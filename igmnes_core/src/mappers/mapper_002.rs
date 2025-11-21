use crate::mappers::{CpuMapper, PpuMapper};
use crate::memory::{MemMapped, Ram};
use crate::rom::{MirroringMode, Rom};
use std::ops::Range;

const BANK_SIZE_BYTES: usize = 16_384;
const CHR_RAM_SIZE: usize = 8_192;

#[derive(Clone)]
pub struct UxROM {
    vram: Ram,
    prg_rom_bytes: Vec<u8>,
    chr_ram_bytes: Vec<u8>,
    mirroring_mode: MirroringMode,

    bank_index: usize,
}

impl UxROM {
    pub fn new(rom: &Rom) -> UxROM {
        let prg_rom_bytes = rom.prg_rom_bytes.clone(); // TODO use references!
        let chr_ram_bytes: Vec<u8> = vec![0; CHR_RAM_SIZE];
        UxROM {
            vram: Ram::default(),
            prg_rom_bytes,
            chr_ram_bytes,
            mirroring_mode: rom.header.mirroring_mode,
            bank_index: 0,
        }
    }

    fn get_prg_rom_index(&self, index: u16) -> usize {
        // Banks
        //     CPU $8000-$BFFF: 16 KB switchable PRG ROM bank
        //     CPU $C000-$FFFF: 16 KB PRG ROM bank, fixed to the last bank
        match index {
            0x8000..=0xBFFF => {
                let byte_index = (index as usize) - 0x8000;
                (self.bank_index * BANK_SIZE_BYTES) + byte_index
            }
            0xC000..=0xFFFF => {
                let byte_index = (index as usize) - 0xC000;
                (self.prg_rom_bytes.len() - BANK_SIZE_BYTES) + byte_index
            }
            _ => unreachable!(),
        }
    }

    fn select_bank(&mut self, byte: u8) {
        self.bank_index = byte as usize;
    }
}

impl CpuMapper for UxROM {
    fn read_prg_rom(&self, index: u16) -> u8 {
        let index = self.get_prg_rom_index(index);
        self.prg_rom_bytes[index]
    }

    fn read_prg_ram(&self, _index: u16) -> u8 {
        0
    }

    fn write_prg_ram(&mut self, _index: u16, _byte: u8) {}
}

impl PpuMapper for UxROM {
    fn read_chr_rom(&self, index: u16) -> u8 {
        panic!(
            "Attempted read from non-existent CHR ROM index (untranslated): 0x{:X}",
            index
        )
    }

    fn read_chr_rom_range(&self, range: Range<u16>) -> Vec<u8> {
        panic!(
            "Attempted read from non-existent CHR ROM range (untranslated): 0x{:?}",
            range
        )
    }

    fn read_chr_rom_range_ref(&self, range: Range<u16>) -> &[u8] {
        panic!(
            "Attempted read from non-existent CHR ROM range (untranslated): 0x{:?}",
            range
        )
    }

    fn read_chr_ram(&self, index: u16) -> u8 {
        self.chr_ram_bytes[index as usize]
    }

    fn read_chr_ram_range(&self, range: Range<u16>) -> Vec<u8> {
        self.read_chr_ram_range_ref(range).to_vec()
    }

    fn read_chr_ram_range_ref(&self, range: Range<u16>) -> &[u8] {
        &self.chr_ram_bytes[range.start as usize..range.end as usize]
    }

    fn write_chr_ram(&mut self, index: u16, byte: u8) {
        self.chr_ram_bytes[index as usize] = byte;
    }

    fn get_mirrored_index(&self, index: u16) -> u16 {
        let index = index - 0x2000;
        match self.mirroring_mode {
            MirroringMode::Horizontal => ((index / 0x800) * 0x400) + (index % 0x400),
            MirroringMode::Vertical => index % 0x800,
        }
    }
}

impl MemMapped for UxROM {
    fn read(&mut self, index: u16) -> u8 {
        match index {
            0..=0x1FFF => self.read_chr_ram(index),
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
            0x8000..=0xFFFF => self.select_bank(byte),
            _ => return,
        }
    }

    fn read_range(&self, range: Range<u16>) -> Vec<u8> {
        match range.start {
            0..=0x1FFF => self.read_chr_ram_range(range),
            _ => unimplemented!(),
        }
    }
}
