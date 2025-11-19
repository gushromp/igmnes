use crate::core::errors::EmulationError;
use crate::core::errors::EmulationError::MemoryAccess;
use crate::core::mappers::{CpuMapper, PpuMapper};
use crate::core::memory::{MemMapped, Ram};
use crate::core::rom::Rom;
use std::ops::Range;

const BANK_SIZE_BYTES: usize = 32_768;
const CHR_RAM_SIZE: usize = 8_192;

#[derive(Clone)]
pub struct AxROM {
    vram: Ram,
    prg_rom_bytes: Vec<u8>,
    chr_ram_bytes: Vec<u8>,

    bank_index: usize,
    nametable_index: usize,
}

impl AxROM {
    pub fn new(rom: &Rom) -> AxROM {
        let prg_rom_bytes = rom.prg_rom_bytes.clone(); // TODO use references!
        let chr_ram_bytes: Vec<u8> = vec![0; CHR_RAM_SIZE];
        AxROM {
            vram: Ram::default(),
            prg_rom_bytes,
            chr_ram_bytes,
            bank_index: 0,
            nametable_index: 0,
        }
    }

    fn get_prg_rom_index(&self, index: u16) -> usize {
        // Banks
        //     CPU $8000-$FFFF: 32 KB switchable PRG ROM bank
        (self.bank_index * BANK_SIZE_BYTES) + (index as usize & 0x7FFF)
    }

    fn select_bank(&mut self, byte: u8) {
        self.bank_index = (byte & 0b111) as usize;
        self.nametable_index = ((byte >> 4) & 0x1) as usize;
    }
}

impl CpuMapper for AxROM {
    fn read_prg_rom(&self, index: u16) -> Result<u8, EmulationError> {
        let index = self.get_prg_rom_index(index);
        Ok(self.prg_rom_bytes[index])
    }

    fn read_prg_ram(&self, _index: u16) -> Result<u8, EmulationError> {
        Ok(0)
    }

    fn write_prg_ram(&mut self, _index: u16, _byte: u8) -> Result<(), EmulationError> {
        Ok(())
    }
}

impl PpuMapper for AxROM {
    fn read_chr_rom(&self, index: u16) -> Result<u8, EmulationError> {
        Err(MemoryAccess(format!(
            "Attempted read from non-existent CHR ROM index (untranslated): 0x{:X}",
            index
        )))
    }

    fn read_chr_rom_range(&self, range: Range<u16>) -> Result<Vec<u8>, EmulationError> {
        Err(MemoryAccess(format!(
            "Attempted read from non-existent CHR ROM range (untranslated): 0x{:?}",
            range
        )))
    }

    fn read_chr_ram(&self, index: u16) -> Result<u8, EmulationError> {
        Ok(self.chr_ram_bytes[index as usize])
    }

    fn read_chr_ram_range(&self, range: Range<u16>) -> Result<Vec<u8>, EmulationError> {
        Ok(self.chr_ram_bytes[range.start as usize..range.end as usize].to_vec())
    }

    fn write_chr_ram(&mut self, index: u16, byte: u8) -> Result<(), EmulationError> {
        self.chr_ram_bytes[index as usize] = byte;
        Ok(())
    }

    fn get_mirrored_index(&self, index: u16) -> u16 {
        let index = index - 0x2000;
        let index = match self.nametable_index {
            0 => index % 0x400,
            1 => index % 0x400 + 0x400,
            _ => unreachable!(),
        };
        index
    }
}

impl MemMapped for AxROM {
    fn read(&mut self, index: u16) -> Result<u8, EmulationError> {
        match index {
            0..=0x1FFF => self.read_chr_ram(index),
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
            }
            0x8000..=0xFFFF => Ok(self.select_bank(byte)),
            _ => Ok(()),
        }
    }

    fn read_range(&self, range: Range<u16>) -> Result<Vec<u8>, EmulationError> {
        match range.start {
            0..=0x1FFF => self.read_chr_ram_range(range),
            _ => unimplemented!(),
        }
    }
}
