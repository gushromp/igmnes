use std::ops::Range;
use core::mappers::{CpuMapper, Mapper, PpuMapper};
use core::memory::{MemMapped, Ram};
use core::rom::{MirroringMode, Rom};
use core::errors::EmulationError::{self, MemoryAccess};

#[derive(Clone)]
pub struct NRom {
    vram: Ram,
    prg_rom_bytes: Vec<u8>,
    chr_rom_bytes: Vec<u8>,
    prg_ram_bytes: Vec<u8>,
    mirroring_mode: MirroringMode
}

impl NRom {
    pub fn new(rom: &Rom) -> NRom {
        let prg_rom_bytes = rom.prg_rom_bytes.clone(); // TODO use references!
        let chr_rom_bytes = rom.chr_rom_bytes.clone();

        let prg_ram_size = rom.header.prg_ram_size;
        let prg_ram_bytes: Vec<u8> = vec![0; prg_ram_size as usize];

        NRom {
            vram: Ram::default(),
            prg_rom_bytes,
            chr_rom_bytes,
            prg_ram_bytes,
            mirroring_mode: rom.header.mirroring_mode
        }
    }

    // Mirrors if prg size is smaller than 32k
    fn get_prg_rom_index(&self, index: u16) -> usize {
        // CPU memory map maps the cart address space from 0x4020 to 0xFFFF
        // NROM starts mapping ROM at 0x8000
        let index = index - 0x8000;
        if index > 0x3FFF && self.prg_rom_bytes.len() < 0x8000 {
            (index - 0x4000) as usize
        }
        else {
            index as usize
        }
    }

    fn get_prg_ram_index(&self, index: u16) -> usize {
        // CPU memory map maps the cart address space from 0x4020 to 0xFFFF
        // NROM starts mapping RAM at 0x6000, so there's nothing mapped between 0x4020 and 0x6000
        (index - 0x6000) as usize
    }

}

impl CpuMapper for NRom {
    fn read_prg_rom(&self, index: u16) -> Result<u8, EmulationError> {
        let index: usize = self.get_prg_rom_index(index);
        Ok(self.prg_rom_bytes[index])
    }

    fn read_prg_ram(&self, index: u16) -> Result<u8, EmulationError> {
        let index: usize = self.get_prg_ram_index(index);
        Ok(self.prg_ram_bytes[index])
    }

    fn write_prg_ram(&mut self, index: u16, byte: u8) -> Result<(), EmulationError>{
        let index: usize = self.get_prg_ram_index(index);
        self.prg_ram_bytes[index] = byte;
        Ok(())
    }
}

impl PpuMapper for NRom {
    fn read_chr_rom(&self, index: u16) -> Result<u8, EmulationError> {
        if self.chr_rom_bytes.is_empty() {
            Ok(0)
        } else {
            Ok(self.chr_rom_bytes[index as usize])
        }
    }

    fn read_chr_rom_range(&self, range: Range<u16>) -> Result<Vec<u8>, EmulationError> {
        if self.chr_rom_bytes.len() == 0 {
            // Mainly for test roms that don't contain CHR
            Ok(vec![])
        } else {
            Ok(self.chr_rom_bytes[range.start as usize..range.end as usize].to_vec())
        }
    }

    fn read_chr_ram(&self, index: u16) -> Result<u8, EmulationError> {
        Err(MemoryAccess(format!("Attempted read from non-existent CHR RAM index (untranslated): 0x{:X}", index)))
    }

    fn read_chr_ram_range(&self, range: Range<u16>) -> Result<Vec<u8>, EmulationError> {
        Err(MemoryAccess(format!("Attempted read from non-existent CHR RAM range (untranslated): 0x{:?}", range)))
    }

    fn write_chr_ram(&mut self, index: u16, _byte: u8) -> Result<(), EmulationError> {
        Err(MemoryAccess(format!("Attempted read from non-existent CHR RAM index (untranslated): 0x{:X}", index)))
    }

    fn get_mirrored_index(&self, index: u16) -> u16 {
        let index = index - 0x2000;
        match self.mirroring_mode {
            MirroringMode::Horizontal => ((index / 0x800) * 0x400) + (index % 0x400),
            MirroringMode::Vertical => index % 0x800
        }
    }
}

impl Mapper for NRom { }

impl MemMapped for NRom {
    fn read(&mut self, index: u16) -> Result<u8, EmulationError> {
        match index {
            0..=0x1FFF => self.read_chr_rom(index),
            0x2000..=0x2FFF => {
                let index = self.get_mirrored_index(index);
                self.vram.read(index)
            }
            0x6000..=0x7FFF => self.read_prg_ram(index),
            0x8000..=0xFFFF => self.read_prg_rom(index),
            _ => {
                println!("Attempted read from unmapped address: 0x{:X}", index);
                Ok(0)

            }
        }
    }

    fn write(&mut self, index: u16, byte: u8) -> Result<(), EmulationError> {
        match index {
            0x2000..=0x2FFF => {
                let index = self.get_mirrored_index(index);
                self.vram.write(index, byte)
            }
            0x6000..=0x7FFF => self.write_prg_ram(index, byte),
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