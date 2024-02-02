mod mapper_000;

use std::cell::RefCell;
use std::rc::Rc;
use self::mapper_000::NRom;
use core::memory::MemMapped;
use core::rom::Rom;
use core::errors::EmulationError;

pub trait CpuMapper : MemMapped {

    // Reads from PRG ROM
    fn read_prg_rom(&self, index: u16) -> Result<u8, EmulationError>;
    // Reads/Writes to PRG RAM
    fn read_prg_ram(&self, index: u16) -> Result<u8, EmulationError>;
    fn write_prg_ram(&mut self, index: u16, byte: u8) -> Result<(), EmulationError>;
}

pub trait PpuMapper : MemMapped {
    // Reads from CHR ROM
    fn read_chr_rom(&self, index: u16) -> Result<u8, EmulationError>;
    // Reads/Writes to CHR RAM
    fn read_chr_ram(&self, index: u16) -> Result<u8, EmulationError>;
    fn write_chr_ram(&mut self, index: u16, byte: u8) -> Result<(), EmulationError>;
}

pub trait Mapper : CpuMapper + PpuMapper {}

pub fn load_mapper_for_rom(rom: &Rom) -> Result<Rc<RefCell<dyn Mapper>>, String> {
    match rom.header.mapper_number {
        0 => Ok(Rc::new(RefCell::new(NRom::new(rom)))),
        mapper_num @ _ => Err(format!("Unsupported mapper number: {}", mapper_num)),
    }
}

pub fn default_mapper() -> Rc<RefCell<dyn Mapper>> {
    let def_rom = Rom::default();

    Rc::new(RefCell::new(NRom::new(&def_rom))) 
}