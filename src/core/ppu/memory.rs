// PpuMemMap

use std::array;
use std::cell::RefCell;
use std::rc::Rc;
use core::errors::EmulationError;
use core::mappers;
use core::mappers::Mapper;
use core::memory::MemMapped;
use core::ppu::OamTable;
use core::ppu::palette::PpuPalette;

pub struct PpuMemMap {
    pub oam_table: OamTable,
    pub palette: PpuPalette,
    mapper: Rc<RefCell<dyn Mapper>>,
}

impl Default for PpuMemMap {
    fn default() -> Self {
        let def_mapper = mappers::default_mapper();

        PpuMemMap {
            oam_table: OamTable::default(),
            palette: PpuPalette::default(),
            mapper: def_mapper,
        }
    }
}

impl PpuMemMap {
    pub fn new(mapper: Rc<RefCell<dyn Mapper>>) -> PpuMemMap {
        PpuMemMap {
            oam_table: OamTable::default(),
            palette: PpuPalette::default(),
            mapper,
        }
    }

    pub fn fetch_name_table_entry(&mut self, reg_v: u16) -> Result<u8, EmulationError> {
        let name_table_entry_addr = 0x2000 | (reg_v & 0x0FFF);
        self.read(name_table_entry_addr)
    }

    pub fn fetch_attribute_table_entry(&mut self, reg_v: u16) -> Result<u8, EmulationError> {
        // attribute address = 0x23C0 | (v & 0x0C00) | ((v >> 4) & 0x38) | ((v >> 2) & 0x07)
        let attribute_table_entry_addr = 0x23C0 | (reg_v & 0x0C00) | ((reg_v >> 4) & 0x38) | ((reg_v >> 2) & 0x07);
        self.read(attribute_table_entry_addr)
    }

    pub fn fetch_pattern_table_entry(&mut self, pattern_table_index: u8, name_table_entry: u8, pixel_y: u16) -> Result<[u8; 2], EmulationError> {
        // DCBA98 76543210
        // ---------------
        // 0HNNNN NNNNPyyy
        // |||||| |||||+++- T: Fine Y offset, the row number within a tile
        // |||||| ||||+---- P: Bit plane (0: less significant bit; 1: more significant bit)
        // ||++++-++++----- N: Tile number from name table
        // |+-------------- H: Half of pattern table (0: "left"; 1: "right")
        // +--------------- 0: Pattern table is at $0000-$1FFF

        let pattern_table_addr_low: u16 =
            (pattern_table_index as u16) << 13
                | (name_table_entry as u16) << 4
                | pixel_y;

        let pattern_table_addr_high: u16 =
            (pattern_table_index as u16) << 13
                | (name_table_entry as u16) << 4
                | 1 << 3
                | pixel_y;

        let pattern_table_byte_low = self.read(pattern_table_addr_low).unwrap();
        let pattern_table_byte_high = self.read(pattern_table_addr_high).unwrap();
        Ok([pattern_table_byte_low, pattern_table_byte_high])
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
                self.mapper.borrow_mut().read(index)
            }
            0x3000..=0x3EFF => {
                // Mirrors 0f 0x2000..=0x2EFF
                let index = index - 0x1000;
                self.mapper.borrow_mut().read(index)
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
                self.mapper.borrow_mut().write(index, byte)
            }
            0x3000..=0x3EFF => {
                // Mirrors 0f 0x2000..=0x2EFF
                let index = index - 0x1000;
                self.mapper.borrow_mut().write(index, byte)
            }
            0x3F00..=0x3FFF => {
                let index = (index - 0x3F00) % 32;
                self.palette.write(index, byte)
            }
            _ => Ok(())
        }
    }
}
