// PpuMemMap

use std::array;
use std::cell::RefCell;
use std::rc::Rc;
use sdl2::pixels::Palette;
use core::errors::EmulationError;
use core::mappers;
use core::mappers::Mapper;
use core::memory::{MemMapped, Ram};
use core::ppu::OamTable;
use core::ppu::palette::PpuPalette;

pub struct PpuMemMap {
    ram: Ram,
    pub oam_table: OamTable,
    pub palette: PpuPalette,
    mapper: Rc<RefCell<dyn Mapper>>,
}

impl Default for PpuMemMap {
    fn default() -> Self {
        let def_mapper = mappers::default_mapper();

        PpuMemMap {
            ram: Ram::default(),
            oam_table: OamTable::default(),
            palette: PpuPalette::default(),
            mapper: def_mapper,
        }
    }
}

impl PpuMemMap {
    pub fn new(mapper: Rc<RefCell<dyn Mapper>>) -> PpuMemMap {
        PpuMemMap {
            ram: Ram::default(),
            oam_table: OamTable::default(),
            palette: PpuPalette::default(),
            mapper,
        }
    }

    pub fn fetch_name_table_entry(&mut self, name_table_index: u8, name_table_entry_index: u16) -> Result<u8, EmulationError> {
        let base_addr = match name_table_index {
            0b00 => 0x2000,
            0b01 => 0x2400,
            0b10 => 0x2800,
            0b11 => 0x2C00,
            _ => unreachable!()
        };
        let name_table_entry_addr = base_addr + name_table_entry_index;
        self.read(name_table_entry_addr)
    }

    pub fn fetch_attribute_table_entry(&mut self, name_table_index: u8, attribute_table_entry_index: u16) -> Result<u8, EmulationError> {
        let base_addr = match name_table_index {
            0b00 => 0x23C0,
            0b01 => 0x27C0,
            0b10 => 0x2BC0,
            0b11 => 0x2FC0,
            _ => unreachable!()
        };
        let attribute_table_entry_addr = base_addr + attribute_table_entry_index;
        self.read(attribute_table_entry_addr)
    }

    pub fn fetch_pattern_table_entry(&mut self, pattern_table_index: u8, pattern_table_entry_index: u16) -> Result<[u8; 16], EmulationError> {
        let base_addr = match pattern_table_index {
            0b00 => 0x1000,
            0b01 => 0x2000,
            _ => unreachable!()
        };

        let pattern_table_entry_addr = base_addr + pattern_table_entry_index;
        Ok(array::from_fn(|index| {
            self.read(pattern_table_entry_addr + index as u16).unwrap()
        }))
    }
}

impl MemMapped for PpuMemMap {
    //      Address range	Size	Device
    //      $0000-$0FFF 	$1000 	Pattern table 0
    //      $1000-$1FFF 	$1000 	Pattern table 1
    //      $2000-$23FF 	$0400 	Nametable 0
    //      $2400-$27FF 	$0400 	Nametable 1
    //      $2800-$2BFF 	$0400 	Nametable 2
    //      $2C00-$2FFF 	$0400 	Nametable 3
    //      $3000-$3EFF 	$0F00 	Mirrors of $2000-$2EFF
    //      $3F00-$3F1F 	$0020 	Palette RAM indexes
    //      $3F20-$3FFF 	$00E0 	Mirrors of $3F00-$3F1F
    fn read(&mut self, index: u16) -> Result<u8, EmulationError> {
        match index {
            0x0000..=0x1FFF => {
                self.mapper.borrow_mut().read(index)
            }
            0x2000..=0x2FFF => {
                let index = (index - 0x2000) % 0x800; // TODO - mirroring via mapper
                self.ram.read(index)
            }
            0x3000..=0x3EFF => {
                // Mirrors 0f 0x2000..=0x2EFF
                let index = (index - 0x1000) % 0x800; // TODO - mirroring via mapper
                self.ram.read(index)
            }
            0x3F00..=0x3FFF => {
                let index = (index - 0x3F00) % 20;
                self.palette.read(index)
            }
            _ => unreachable!()
        }
    }

    fn write(&mut self, index: u16, byte: u8) -> Result<(), EmulationError> {
        match index {
            0x0000..=0x1FFF => {
                self.mapper.borrow_mut().write(index, byte)
            }
            0x2000..=0x2FFF => {
                let index = (index - 0x2000) % 0x800; // TODO - mirroring via mapper
                self.ram.write(index, byte)
            }
            0x3000..=0x3EFF => {
                // Mirrors 0f 0x2000..=0x2EFF
                let index = (index - 0x1000) % 0x800; // TODO - mirroring via mapper
                self.ram.write(index, byte)
            }
            0x3F00..=0x3FFF => {
                let index = (index - 0x3F00) % 32;
                self.palette.write(index, byte)
            }
            _ => Ok(())
        }
    }
}
