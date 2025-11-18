use std::ops::Range;
use crate::core::errors::EmulationError;
use crate::core::errors::EmulationError::MemoryAccess;
use crate::core::mappers::{CpuMapper, PpuMapper};
use crate::core::memory::{MemMapped, Ram};
use crate::core::rom::{MirroringMode, Rom};

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
            bank_index: 0
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
        let byte_in_rom = self.read_prg_rom(index).unwrap();
        let resulting_byte = (byte & 0b11) & byte_in_rom;
        self.bank_index = resulting_byte as usize;
    }

}

impl CpuMapper for CNROM {
    fn read_prg_rom(&self, index: u16) -> Result<u8, EmulationError> {
        let index = self.get_prg_rom_index(index);
        Ok(self.prg_rom_bytes[index])
    }

    fn read_prg_ram(&self, _index: u16) -> Result<u8, EmulationError> {
        Ok(0)
    }

    fn write_prg_ram(&mut self, _index: u16, _byte: u8) -> Result<(), EmulationError> { Ok(()) }
}

impl PpuMapper for CNROM {
    fn read_chr_rom(&self, index: u16) -> Result<u8, EmulationError> {
        let index = self.get_chr_rom_index(index);
        Ok(self.chr_rom_bytes[index])
    }

    fn read_chr_rom_range(&self, range: Range<u16>) -> Result<Vec<u8>, EmulationError> {
        let adjusted_range_start_index = self.get_chr_rom_index(range.start);
        let adjusted_range = adjusted_range_start_index..adjusted_range_start_index + range.len();
        Ok(self.chr_rom_bytes[adjusted_range].to_vec())
    }

    fn read_chr_ram(&self, index: u16) -> Result<u8, EmulationError> {
        Err(MemoryAccess(format!("Attempted read from non-existent CHR RAM index (untranslated): 0x{:X}", index)))
    }

    fn read_chr_ram_range(&self, range: Range<u16>) -> Result<Vec<u8>, EmulationError> {
        Err(MemoryAccess(format!("Attempted read from non-existent CHR RAM range (untranslated): 0x{:?}", range)))
    }

    fn write_chr_ram(&mut self, _index: u16, _byte: u8) -> Result<(), EmulationError> {
        Ok(())
    }

    fn get_mirrored_index(&self, index: u16) -> u16 {
        let index = index - 0x2000;
        match self.mirroring_mode {
            MirroringMode::Horizontal => ((index / 0x800) * 0x400) + (index % 0x400),
            MirroringMode::Vertical => index % 0x800
        }
    }
}

impl MemMapped for CNROM {
    fn read(&mut self, index: u16) -> Result<u8, EmulationError> {
        match index {
            0..=0x1FFF => self.read_chr_rom(index),
            0x2000..=0x2FFF => {
                let index = self.get_mirrored_index(index);
                self.vram.read(index)
            }
            0x8000..=0xFFFF => self.read_prg_rom(index),
            _ => {
                println!("Attempted read from unmapped address: 0x{:X}", index);
                Ok(0)

            }
        }
    }

    fn write(&mut self, index: u16, byte: u8) -> Result<(), EmulationError> {
        match index {
            0..=0x1FFF => self.write_chr_ram(index, byte),
            0x2000..=0x2FFF => {
                let index = self.get_mirrored_index(index);
                self.vram.write(index, byte)
            },
            0x8000..=0xFFFF => Ok(self.select_bank(index, byte)),
            _ => {
                Ok(())
            }
        }
    }

    fn read_range(&self, range: Range<u16>) -> Result<Vec<u8>, EmulationError> {
        match range.start {
            0..=0x1FFF => self.read_chr_rom_range(range),
            _ => unimplemented!()
        }
    }
}