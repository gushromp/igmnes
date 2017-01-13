mod mapper_000;

use self::mapper_000::NRom;
use core::memory::MemMapped;
use core::rom::Rom;
use core::errors::EmulationError;

pub trait Mapper : MemMapped {

    // Reads from PRG ROM
    fn read_prg_rom(&self, index: u16) -> Result<u8, EmulationError>;
    // Reads/Writes to PRG RAM
    fn read_prg_ram(&self, index: u16) -> Result<u8, EmulationError>;
    fn write_prg_ram(&mut self, index: u16, byte: u8) -> Result<(), EmulationError>;
    // Reads from CHR ROM
    fn read_chr_rom(&self, index: u16) -> Result<u8, EmulationError>;
    // Reads/Writes to CHR RAM
    fn read_chr_ram(&self, index: u16) -> Result<u8, EmulationError>;
    fn write_chr_ram(&mut self, index: u16, byte: u8) -> Result<(), EmulationError>;
}

pub fn load_mapper_for_rom(rom: &Rom) -> Result<Box<Mapper>, String> {
    match rom.header.mapper_number {
        0 => Ok(Box::new(NRom::new(rom)) as Box<Mapper>),
        mapper_num @ _ => Err(format!("Unsupported mapper number: {}", mapper_num)),
    }
}

pub fn default_mapper() -> Box<Mapper> {
    let def_rom = Rom::default();

    Box::new(NRom::new(&def_rom)) as Box<Mapper>
}