// PpuMemMap

use crate::errors::EmulationError;
use crate::mappers::SharedMapper;
use crate::memory::MemMapped;
use crate::ppu::palette::PpuPalette;
use crate::ppu::OamTable;
use std::array;
use std::ops::Range;

pub struct PpuMemMap {
    pub oam_table: OamTable,
    pub palette: PpuPalette,
    pub mapper: SharedMapper,
}

impl Default for PpuMemMap {
    fn default() -> Self {
        PpuMemMap {
            oam_table: OamTable::default(),
            palette: PpuPalette::default(),
            mapper: SharedMapper::default(),
        }
    }
}

impl PpuMemMap {
    pub fn new(mapper: SharedMapper) -> PpuMemMap {
        PpuMemMap {
            oam_table: OamTable::default(),
            palette: PpuPalette::default(),
            mapper,
        }
    }

    #[inline(always)]
    pub fn fetch_name_table_entry(&mut self, reg_v: u16) -> u8 {
        let name_table_entry_addr = 0x2000 | (reg_v & 0x0FFF);
        self.read(name_table_entry_addr)
    }

    #[inline(always)]
    pub fn fetch_attribute_table_entry(&mut self, reg_v: u16) -> u8 {
        // attribute address =                 0x23C0 | (v & 0x0C00) | ((v >> 4) & 0x38) | ((v >> 2) & 0x07)
        let attribute_table_entry_addr =
            0x23C0 | (reg_v & 0x0C00) | ((reg_v >> 4) & 0x38) | ((reg_v >> 2) & 0x07);
        self.read(attribute_table_entry_addr)
    }

    pub fn fetch_pattern_table_entry(
        &mut self,
        pattern_table_index: u8,
        name_table_entry: u8,
        pixel_y: u16,
    ) -> Result<[u8; 2], EmulationError> {
        // PPU addresses within the pattern tables can be decoded as follows:
        // DCBA98 76543210
        // ---------------
        // 0HNNNN NNNNPyyy
        // |||||| |||||+++- T: Fine Y offset, the row number within a tile
        // |||||| ||||+---- P: Bit plane (0: less significant bit; 1: more significant bit)
        // ||++++-++++----- N: Tile number from name table
        // |+-------------- H: Half of pattern table (0: "left"; 1: "right")
        // +--------------- 0: Pattern table is at $0000-$1FFF

        let pattern_table_addr_low: u16 =
            (pattern_table_index as u16) << 12 | (name_table_entry as u16) << 4 | pixel_y;

        let pattern_table_addr_high: u16 =
            (pattern_table_index as u16) << 12 | (name_table_entry as u16) << 4 | 1 << 3 | pixel_y;

        let pattern_table_byte_low = self.read(pattern_table_addr_low);
        let pattern_table_byte_high = self.read(pattern_table_addr_high);
        Ok([pattern_table_byte_low, pattern_table_byte_high])
    }

    pub fn fetch_sprite_pattern(
        &mut self,
        pattern_table_index: u8,
        pattern_entry_index: u8,
    ) -> Result<[u8; 16], EmulationError> {
        let base_addr = (pattern_table_index as u16) << 12;
        let pattern_entry_addr = base_addr + (pattern_entry_index as u16 * 16);

        let byte_slice = self.read_range(pattern_entry_addr..pattern_entry_addr + 16);
        let result: [u8; 16] = if byte_slice.len() == 0 {
            [0; 16]
        } else {
            array::from_fn(|index| byte_slice[index])
        };
        Ok(result)
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
    #[inline(always)]
    fn read(&mut self, index: u16) -> u8 {
        match index {
            0x0000..=0x1FFF => {
                // CHR ROM/RAM
                self.mapper.read(index)
            }
            0x2000..=0x2FFF => {
                // VRAM
                self.mapper.read(index)
            }
            0x3000..=0x3EFF => {
                // Mirrors 0f 0x2000..=0x2EFF
                let index = index - 0x1000;
                self.mapper.read(index)
            }
            0x3F00..=0x3FFF => {
                // PPU Palette RAM
                let index = (index - 0x3F00) % 20;
                self.palette.read(index)
            }
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn write(&mut self, index: u16, byte: u8) {
        match index {
            0x0000..=0x1FFF => {
                // CHR ROM/RAM
                self.mapper.write(index, byte)
            }
            0x2000..=0x2FFF => {
                // VRAM
                self.mapper.write(index, byte)
            }
            0x3000..=0x3EFF => {
                // Mirrors 0f 0x2000..=0x2EFF
                let index = index - 0x1000;
                self.mapper.write(index, byte)
            }
            0x3F00..=0x3FFF => {
                // PPU Palette RAM
                let index = (index - 0x3F00) % 32;
                self.palette.write(index, byte)
            }
            _ => (),
        }
    }

    #[inline(always)]
    fn read_range(&mut self, range: Range<u16>) -> &[u8] {
        self.mapper.read_range(range)
    }
}
