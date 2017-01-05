mod mapper_000;

use self::mapper_000::NRom;
use core::rom::Rom;

pub trait Mapper {
    fn read_prg(&self, index: u16) -> u8;
    fn write_prg(&mut self, index: u16, byte: u8);
    fn read_chr(&self, index: u16) -> u8;
    fn write_chr(&mut self, index: u16, byte: u8);
}

pub fn load_mapper_for_rom(rom: &Rom) -> Result<Box<Mapper>, String> {
    match rom.header.mapper_number {
        0 => Ok(Box::new(NRom::new(rom)) as Box<Mapper>),
        mapper_num @ _ => Err(format!("Unsupported mapper number: {}", mapper_num)),
    }
}