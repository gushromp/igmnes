mod mapper_000;
mod mapper_002;
mod mapper_003;

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use enum_dispatch::enum_dispatch;
use self::mapper_000::NRom;
use crate::core::memory::MemMapped;
use crate::core::rom::Rom;
use crate::core::errors::EmulationError;
use crate::core::mappers::mapper_002::UxROM;
use crate::core::mappers::mapper_003::CNROM;

#[enum_dispatch]
pub trait CpuMapper: MemMapped {

    // Reads from PRG ROM
    fn read_prg_rom(&self, index: u16) -> Result<u8, EmulationError>;
    // Reads/Writes to PRG RAM
    fn read_prg_ram(&self, index: u16) -> Result<u8, EmulationError>;
    fn write_prg_ram(&mut self, index: u16, byte: u8) -> Result<(), EmulationError>;
}

#[enum_dispatch]
pub trait PpuMapper: MemMapped {
    // Reads from CHR ROM
    fn read_chr_rom(&self, index: u16) -> Result<u8, EmulationError>;
    fn read_chr_rom_range(&self, range: Range<u16>) -> Result<Vec<u8>, EmulationError>;

    // Reads/Writes to CHR RAM
    fn read_chr_ram(&self, index: u16) -> Result<u8, EmulationError>;
    fn read_chr_ram_range(&self, range: Range<u16>) -> Result<Vec<u8>, EmulationError>;
    fn write_chr_ram(&mut self, index: u16, byte: u8) -> Result<(), EmulationError>;

    fn get_mirrored_index(&self, index: u16) -> u16;
}

// pub trait Mapper : CpuMapper + PpuMapper {}

#[enum_dispatch(CpuMapper, PpuMapper, MemMapped)]
pub enum MapperImpl {
    Mapper000(NRom),
    Mapper002(UxROM),
    Mapper003(CNROM),
}

pub fn load_mapper_for_rom(rom: &Rom) -> Result<Rc<RefCell<MapperImpl>>, String> {
    let mapper: MapperImpl = match rom.header.mapper_number {
        0 => NRom::new(rom).into(),
        2 => UxROM::new(rom).into(),
        3 => CNROM::new(rom).into(),
        mapper_num @ _ => return Err(format!("Unsupported mapper number: {}", mapper_num)),
    };
    Ok(Rc::new(RefCell::new(mapper)))
}

pub fn default_mapper() -> Rc<RefCell<MapperImpl>> {
    let def_rom = Rom::default();

    Rc::new(RefCell::new(NRom::new(&def_rom).into()))
}