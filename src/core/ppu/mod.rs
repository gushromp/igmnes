pub mod palette;
pub mod memory;

use std::fmt;
use std::fmt::{Binary, Display, Formatter};

use core::debug::Tracer;
use core::errors::EmulationError;
use core::errors::EmulationError::MemoryAccess;

use core::memory::MemMapped;
use core::ppu::memory::PpuMemMap;
use core::ppu::palette::PpuPaletteColor;

const BIT_MASK: u8 = 0b0000_0001;
const BIT_MASK_2: u8 = 0b0000_0011;

type Bit = u8;

// We use a whole byte for now, to avoid bit-packing, this type is merely for clarification
trait BitMask {
    fn get_bit(self: &Self, index: usize) -> bool;
    fn get_bit_u8(self: &Self, index: usize) -> u8;
}

impl BitMask for u8 {
    fn get_bit(self: &Self, index: usize) -> bool {
        self.get_bit_u8(index) != 0
    }

    fn get_bit_u8(self: &Self, index: usize) -> u8 {
        (self >> index) & BIT_MASK
    }
}

#[derive(Default, Copy, Clone)]
struct PpuCtrlReg {
    is_nmi_enabled: bool,
    is_master_enabled: bool,
    sprite_height: u8,
    background_pattern_table_index: u8,
    sprite_pattern_table_index: u8,
    is_increment_mode_32: bool, // VRAM address increment per CPU read/write of PPUDATA, (0: add 1, going across; 1: add 32, going down)
    // Name table index stored in reg_v
}

impl PpuCtrlReg {
    fn write(&mut self, byte: u8) {
        self.is_nmi_enabled = byte.get_bit(7);
        self.is_master_enabled = byte.get_bit(6);
        self.sprite_height = byte.get_bit_u8(5);
        self.background_pattern_table_index = byte.get_bit_u8(4);
        self.sprite_pattern_table_index = byte.get_bit_u8(3);
        self.is_increment_mode_32 = byte.get_bit(2);
    }

    fn hard_reset(&mut self) {
        *self = PpuCtrlReg::default();
    }

    fn soft_reset(&mut self) {}
}

impl Binary for PpuCtrlReg {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Binary::fmt(&(self.is_nmi_enabled as u8), f)?;
        Binary::fmt(&(self.is_master_enabled as u8), f)?;
        Binary::fmt(&self.sprite_height, f)?;
        Binary::fmt(&self.background_pattern_table_index, f)?;
        Binary::fmt(&self.sprite_pattern_table_index, f)?;
        Binary::fmt(&(self.is_increment_mode_32 as u8), f)?;
        Ok(())
    }
}

#[derive(Default, Copy, Clone)]
struct PpuMaskReg {
    // Green and red are swapped on PAL
    is_color_emphasis_blue: bool,
    is_color_emphasis_green: bool,
    is_color_emphasis_red: bool,

    is_show_sprites_enabled: bool,
    is_show_background_enabled: bool,

    is_show_sprites_enabled_leftmost: bool,
    is_show_background_enabled_leftmost: bool,

    is_greyscale_enabled: bool,
}

impl PpuMaskReg {
    fn write(&mut self, byte: u8) {
        self.is_color_emphasis_blue = byte.get_bit(7);
        self.is_color_emphasis_green = byte.get_bit(6);
        self.is_color_emphasis_red = byte.get_bit(5);
        self.is_show_sprites_enabled = byte.get_bit(4);
        self.is_show_background_enabled = byte.get_bit(3);
        self.is_show_sprites_enabled_leftmost = byte.get_bit(2);
        self.is_show_background_enabled_leftmost = byte.get_bit(1);
        self.is_greyscale_enabled = byte.get_bit(0);
    }

    fn hard_reset(&mut self) {
        *self = PpuMaskReg::default();
    }
}

impl Binary for PpuMaskReg {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Binary::fmt(&(self.is_color_emphasis_blue as u8), f)?;
        Binary::fmt(&(self.is_color_emphasis_green as u8), f)?;
        Binary::fmt(&(self.is_color_emphasis_red as u8), f)?;
        Binary::fmt(&(self.is_show_sprites_enabled as u8), f)?;
        Binary::fmt(&(self.is_show_background_enabled as u8), f)?;
        Binary::fmt(&(self.is_show_sprites_enabled_leftmost as u8), f)?;
        Binary::fmt(&(self.is_show_background_enabled_leftmost as u8), f)?;
        Binary::fmt(&(self.is_greyscale_enabled as u8), f)?;

        Ok(())
    }
}

#[derive(Default, Copy, Clone)]
struct PpuStatusReg {
    is_in_vblank: bool,
    is_sprite_0_hit: bool,
    is_sprite_overflow: bool,
}

impl PpuStatusReg {
    fn read(&mut self) -> u8 {
        let value = (self.is_in_vblank as u8) << 7 | (self.is_sprite_0_hit as u8) << 6 | (self.is_sprite_overflow as u8) << 5;
        value
    }

    fn hard_reset(&mut self) {
        self.is_in_vblank = false;
        self.is_sprite_0_hit = false;
        self.is_sprite_overflow = false;
    }

    fn soft_reset(&mut self) {
        self.is_sprite_0_hit = false;
        self.is_sprite_overflow = false;
    }
}

#[derive(Default, Copy, Clone)]
struct PpuScrollReg {
    x: u8,
    y: u8,
}

impl PpuScrollReg {
    fn write(&mut self, byte: u8, is_addr_latch_on: bool) {
        if is_addr_latch_on {
            self.y = byte;
        } else {
            self.x = byte;
        }
    }

    fn hard_reset(&mut self) {
        self.x = 0;
        self.y = 0;
    }

    fn soft_reset(&mut self) {}
}

#[derive(Default, Copy, Clone)]
struct OamTileIndex {
    //For 8x8 sprites, this is the tile number of this sprite within the pattern table selected in bit 3 of PPUCTRL ($2000).
    //
    // For 8x16 sprites (bit 5 of PPUCTRL set), the PPU ignores the pattern table selection and selects a pattern table from bit 0 of this number.
    tile_index: u8,
    bank_index: Bit, // Bank ($0000 or $1000) of tiles
}

#[derive(Copy, Clone)]
enum OamAttributePriority {
    FRONT = 0,
    BACK = 1,
}

impl Default for OamAttributePriority {
    fn default() -> Self {
        OamAttributePriority::FRONT
    }
}

impl From<u8> for OamAttributePriority {
    fn from(value: u8) -> Self {
        use core::ppu::OamAttributePriority::{BACK, FRONT};
        match value {
            0 => FRONT,
            1 => BACK,
            _ => unreachable!()
        }
    }
}

#[derive(Default, Copy, Clone)]
struct OamSpriteAttributes {
    // 76543210
    // ||||||||
    // ||||||++- Palette (4 to 7) of sprite
    // |||+++--- Unimplemented (read 0)
    // ||+------ Priority (0: in front of background; 1: behind background)
    // |+------- Flip sprite horizontally
    // +-------- Flip sprite vertically

    palette_index: u8,
    priority: OamAttributePriority,
    is_flipped_horizontally: bool,
    is_flipped_vertically: bool,
}

impl OamSpriteAttributes {
    fn write(&mut self, byte: u8) {
        self.palette_index = byte & 0b00001111;
        self.priority = OamAttributePriority::from(byte.get_bit_u8(5));
        self.is_flipped_horizontally = byte.get_bit(6);
        self.is_flipped_vertically = byte.get_bit(7);
    }
}

#[derive(Default, Copy, Clone)]
struct OamEntry {
    sprite_y: u8,
    tile_bank_index: u8,
    attributes: OamSpriteAttributes,
    sprite_x: u8,
}

#[derive(Copy, Clone)]
pub struct OamTable {
    oam_entries: [OamEntry; 64],
}

impl Default for OamTable {
    fn default() -> Self {
        OamTable { oam_entries: [OamEntry::default(); 64] }
    }
}

impl OamTable {
    pub fn write(&mut self, cpu_mem: Vec<u8>) -> Result<(), EmulationError> {
        if cpu_mem.len() != 0x100 {
            Err(MemoryAccess(format!("Attempted OAM DMA write with size {:2X}, expected size {:2X}", cpu_mem.len(), 0x100)))
        } else {
            Ok(())
        }
    }
}

#[derive(Copy, Clone)]
struct PpuMemMapConfig {
    is_mutating_read: bool,
    last_read_cycle: u16,
}

impl Default for PpuMemMapConfig {
    fn default() -> Self {
        PpuMemMapConfig {
            is_mutating_read: true,
            last_read_cycle: 0,
        }
    }
}

#[derive(Clone)]
pub struct PpuOutput {
    pub data: Box<[[PpuPaletteColor; 256]; 240]>,
}

impl Default for PpuOutput {
    fn default() -> Self {
        PpuOutput {
            data: Box::new([[PpuPaletteColor::default(); 256]; 240])
        }
    }
}

#[derive(Default)]
pub struct Ppu {
    //
    // PPU Registers
    //
    // Write only
    reg_ctrl: PpuCtrlReg,
    // Write only
    reg_mask: PpuMaskReg,
    // Read only
    reg_status: PpuStatusReg,

    // Write only
    reg_oam_addr: u8,
    // Read/write
    reg_oam_data: u8,

    // Write only, 2x
    reg_scroll: PpuScrollReg,

    // Write only, 2x (unused, combination of reg_v and reg_t during writing to 0x2005/0x2006)
    _reg_addr: u16,
    // Read/write (unused, written to/read from PPU memory map directly using reg_v as address)
    _reg_data: u8,

    //
    // Internal/operational registers
    //


    // yyy NN YYYYY XXXXX
    // ||| || ||||| +++++-- coarse X scroll
    // ||| || +++++-------- coarse Y scroll
    // ||| ++-------------- nametable select
    // +++----------------- fine Y scroll
    reg_v: u16,

    reg_t: u16,
    reg_x: u8,

    is_odd_frame: bool,
    is_address_latch_on: bool,

    //
    // Internal Data
    //
    curr_scanline: u16,
    curr_scanline_cycle: u16,

    cpu_cycles: u64,
    pub nmi_pending: bool,

    pub ppu_mem_map: PpuMemMap,
    mem_map_config: PpuMemMapConfig,

    // Rendering data
    curr_output: PpuOutput,
    last_output: Option<PpuOutput>,

    // Quirks

    // Reading $2002 within a few PPU clocks of when VBL is set results in special-case behavior.
    // Reading one PPU clock before reads it as clear and never sets the flag or generates NMI for that frame.
    // Reading on the same PPU clock or one later reads it as set, clears it, and suppresses the NMI for that frame.
    // Reading two or more PPU clocks before/after it's set behaves normally (reads flag's value, clears it, and doesn't affect NMI operation).
    // This suppression behavior is due to the $2002 read pulling the NMI line back up too quickly after it drops (NMI is active low) for the CPU to see it.
    // (CPU inputs like NMI are sampled each clock.)
    should_skip_vbl: bool,
}


impl Ppu {
    pub fn new(ppu_mem_map: PpuMemMap) -> Self {
        let mut ppu = Ppu {
            ppu_mem_map,
            ..Ppu::default()
        };
        ppu.hard_reset();
        ppu
    }

    #[inline]
    fn coarse_x_scroll(&self) -> u16 {
        (self.reg_v & 0b1_1111)
    }

    #[inline]
    fn incr_coarse_x_scroll(&mut self) {
        let curr_value = self.coarse_x_scroll();
    }

    #[inline]
    fn coarse_y_scroll(&self) -> u8 {
        ((self.reg_v >> 5) & 0b1_1111) as u8
    }

    #[inline]
    fn name_table_index(&self) -> u8 {
        ((self.reg_v >> 7) & 0b11) as u8
    }

    #[inline]
    fn fine_y_scroll(&self) -> u8 {
        ((self.reg_v >> 12) & 0b111) as u8
    }

    fn set_address_latch(&mut self) {
        self.is_address_latch_on = true;
    }

    fn reset_address_latch(&mut self) {
        self.is_address_latch_on = false;
    }

    fn reset_vblank_status(&mut self) {
        self.reg_status.is_in_vblank = false;
    }

    fn is_rendering_enabled(&self) -> bool {
        self.reg_mask.is_show_background_enabled || self.reg_mask.is_show_sprites_enabled
    }

    fn increment_addr_read(&mut self) {
        self.reg_v = if self.reg_ctrl.is_increment_mode_32 {
            self.reg_v.wrapping_add(32)
        } else {
            self.reg_v.wrapping_add(1)
        }
    }

    fn increment_addr_x(&mut self) {
        // if ((v & 0x001F) == 31) // if coarse X == 31
        //   v &= ~0x001F          // coarse X = 0
        //   v ^= 0x0400           // switch horizontal nametable
        // else
        //   v += 1                // increment coarse X

        if (self.reg_v & 0x001F) == 31 {
            self.reg_v &= !0x001F;
            self.reg_v ^= 0x400;
        } else {
            self.reg_v += 1;
        }
    }

    fn increment_addr_y(&mut self) {
        // if ((v & 0x7000) != 0x7000)        // if fine Y < 7
        //   v += 0x1000                      // increment fine Y
        // else
        //   v &= ~0x7000                     // fine Y = 0
        //   int y = (v & 0x03E0) >> 5        // let y = coarse Y
        //   if (y == 29)
        //     y = 0                          // coarse Y = 0
        //     v ^= 0x0800                    // switch vertical nametable
        //   else if (y == 31)
        //     y = 0                          // coarse Y = 0, nametable not switched
        //   else
        //     y += 1                         // increment coarse Y
        //   v = (v & ~0x03E0) | (y << 5)     // put coarse Y back into v

        if (self.reg_v & 0x7000) != 0x7000 {
            self.reg_v += 0x1000;
        } else {
            self.reg_v &= !0x7000;
            let mut y = (self.reg_v & 0x03E0) >> 5;
            if y == 29 {
                y = 0;
                self.reg_v ^= 0800;
            } else if y == 31 {
                y = 0;
            } else {
                y += 1;
            }
            self.reg_v = (self.reg_v & !0x03E0) | (y << 5);
        }
    }

    fn is_in_vblank(&self) -> bool {
        return (self.curr_scanline > 240 && self.curr_scanline <= 260) || !self.is_rendering_enabled();
    }

    pub fn clear_nmi(&mut self) {
        self.nmi_pending = false;
    }

    pub fn is_vblank_starting_cycle(&self) -> bool {
        self.curr_scanline == 241 && self.curr_scanline_cycle == 1
    }

    pub fn hard_reset(&mut self) {
        self.reg_ctrl.hard_reset();
        self.reg_mask.hard_reset();
        self.reg_status.hard_reset();
        self.reg_oam_addr = 0u8;
        self.reg_scroll.hard_reset();

        self.reg_v = 0;
        self.reg_t = 0;
        self.reg_x = 0;

        self.is_address_latch_on = false;
        self.is_odd_frame = false;

        self.curr_scanline = 261;
        self.curr_scanline_cycle = 0;
    }

    pub fn get_output(&mut self) -> &Option<PpuOutput> {
        &self.last_output
    }

    pub fn should_suppress_nmi(&self) -> bool {
        self.should_skip_vbl
    }

    #[inline]
    pub fn step(&mut self, cpu_cycles: u64, tracer: &mut Tracer) -> bool {
        let cycles_to_run = (cpu_cycles - self.cpu_cycles) * 3;

        for _ in 0..cycles_to_run {
            self.curr_scanline_cycle += 1;

            // Rendering scanlines & cycles
            let mut pixel_x = self.curr_scanline_cycle - 1;
            let pixel_y = self.curr_scanline;
            if self.is_rendering_enabled() && pixel_y < 240 && pixel_x % 8 == 0 && pixel_x <= 256 - 8 {
                // Background
                let name_table_entry = self.ppu_mem_map.fetch_name_table_entry(self.reg_v).unwrap();
                let attribute_table_entry = self.ppu_mem_map.fetch_attribute_table_entry(self.reg_v).unwrap();

                let chr_table_entry_index = (name_table_entry as u16) * 16;
                let chr_table_entry = self.ppu_mem_map.fetch_pattern_table_entry(self.reg_ctrl.background_pattern_table_index, chr_table_entry_index).unwrap();

                self.render_background(attribute_table_entry, &chr_table_entry);
            }

            if self.is_rendering_enabled() && (self.curr_scanline < 240) {
                // if (pixel_x >= 8 && pixel_x <= 256) && pixel_x % 8 == 0 {
                if (pixel_x <= 256 - 8) && pixel_x % 8 == 0 {
                    // If rendering is enabled, the PPU increments the horizontal position in v many times across the scanline,
                    // it begins at dots 328 and 336, and will continue through the next scanline at 8, 16, 24... 240, 248, 256
                    // (every 8 dots across the scanline until 256).
                    // Across the scanline the effective coarse X scroll coordinate is incremented repeatedly,
                    // which will also wrap to the next nametable appropriately.
                    self.increment_addr_x()
                }
                if pixel_x == 256 - 8 {
                    // If rendering is enabled, the PPU increments the vertical position in v.
                    // The effective Y scroll coordinate is incremented, which is a complex operation that will correctly skip the attribute table memory regions,
                    // and wrap to the next nametable appropriately.
                    self.increment_addr_y()
                }

                if pixel_x == 257 - 8 {
                    // If rendering is enabled, the PPU copies all bits related to horizontal position from t to v:
                    // reg_v: .....A.. ...BCDEF <- reg_t: ....A.. ...BCDEF
                    let mask = 0b0000_0100_0001_1111;
                    self.reg_v = (self.reg_v & !mask) | (self.reg_t & mask);
                }
            }


            if self.is_rendering_enabled() && self.curr_scanline == 261 && pixel_x == 304 - 8 {
                // If rendering is enabled, at the end of vblank,
                // shortly after the horizontal bits are copied from t to v at dot 257,
                // the PPU will repeatedly copy the vertical bits from t to v from dots 280 to 304,
                // completing the full initialization of v from t
                // reg_v: .GHIA.BC DEF..... <- reg_t: GHIA.BC DEF.....
                let mask = 0b0111_1011_1110_0000;
                self.reg_v = (self.reg_v & !mask) | (self.reg_t & mask);
            }

            if self.curr_scanline_cycle >= 257 && self.curr_scanline_cycle <= 320 {
                self.reg_oam_addr = 0;
            }

            if self.curr_scanline_cycle == 341 || self.curr_scanline == 261 && self.curr_scanline_cycle == 340 && self.is_odd_frame && self.is_rendering_enabled() {
                self.curr_scanline_cycle = 0;
                self.curr_scanline += 1;
            }

            if self.is_vblank_starting_cycle() && !self.should_skip_vbl {
                self.reg_status.is_in_vblank = true;
                if self.reg_ctrl.is_nmi_enabled {
                    self.nmi_pending = true;
                }
            } else if self.curr_scanline == 261 && self.curr_scanline_cycle == 1 {
                self.reg_status.is_in_vblank = false;
            }

            if self.is_rendering_enabled() && self.curr_scanline == 240 && self.curr_scanline_cycle == 1 {
                self.last_output = Some(self.curr_output.clone())
            }

            if self.curr_scanline == 262 {
                self.curr_scanline = 0;
                self.is_odd_frame = !self.is_odd_frame;

                if self.should_skip_vbl {
                    self.should_skip_vbl = false;
                }
            }
        }

        if tracer.is_enabled() {
            tracer.add_ppu_trace(&self);
        }

        self.cpu_cycles = cpu_cycles;
        self.reg_status.is_in_vblank && self.nmi_pending
    }

    #[inline]
    fn render_background(&mut self, attribute_table_entry: u8, pattern_table_entry: &[u8]) {
        let coarse_index_y = ((self.reg_v >> 5) & 0x1F) as usize;
        let coarse_index_x = (self.reg_v & 0x1F) as usize;
        let fine_index_y = ((self.reg_v & 0x7000) >> 12) as usize;
        let fine_index_x = self.reg_x as usize;

        let index_x = coarse_index_x * 8;
        let index_y = (coarse_index_y * 8) + fine_index_y;
        let pixel_attribute_index_y = coarse_index_y;
        let pattern_index = fine_index_y % 8;

        let pattern_bit_plane_1 = pattern_table_entry[pattern_index];
        let pattern_bit_plane_2 = pattern_table_entry[pattern_index + 8];

        for pixel_x in index_x..=index_x + 7 {
            let pixel_attribute_index_x = pixel_x % 32;
            let attribute_shift = match (pixel_attribute_index_x, pixel_attribute_index_y) {
                (0..=15, 0..=15) => 0,
                (16..=31, 0..=15) => 2,
                (0..=15, 16..=31) => 4,
                (16..=31, 16..=31) => 6,
                _ => unreachable!()
            };
            let palette_index = (attribute_table_entry >> attribute_shift) & 0b11;

            let bit_index = 7 - (pixel_attribute_index_x % 8);
            let color_index = (pattern_bit_plane_2.get_bit_u8(bit_index) << 1) | pattern_bit_plane_1.get_bit_u8(bit_index);

            let color = self.ppu_mem_map.palette.get_background_color(palette_index, color_index);
            self.curr_output.data[index_y][pixel_x] = color
        }
    }
}

impl MemMapped for Ppu {
    fn read(&mut self, index: u16) -> Result<u8, EmulationError> {
        match index {
            0 | 1 | 3 | 5 | 6 => Ok(0), // Err(MemoryAccess(format!("Attempted read from write-only PPU register with index {}.", index))),
            2 => {
                // PPUSTATUS
                let value = self.reg_status.read();

                // Reading $2002 within a few PPU clocks of when VBL is set results in special-case behavior.
                // Reading one PPU clock before reads it as clear and never sets the flag or generates NMI for that frame.
                // Reading on the same PPU clock or one later reads it as set, clears it, and suppresses the NMI for that frame.
                // Reading two or more PPU clocks before/after it's set behaves normally (reads flag's value, clears it, and doesn't affect NMI operation).
                // This suppression behavior is due to the $2002 read pulling the NMI line back up too quickly after it drops (NMI is active low) for the CPU to see it.
                // (CPU inputs like NMI are sampled each clock.)
                if self.is_mutating_read() {
                    if self.curr_scanline == 241 && (self.curr_scanline_cycle == 0 || self.curr_scanline_cycle == 1 || self.curr_scanline_cycle == 2) {
                        self.should_skip_vbl = true;
                        self.nmi_pending = false;
                    }

                    // Reading from this register also resets the write latch and vblank active flag
                    self.reset_address_latch();
                    self.reset_vblank_status();
                }

                Ok(value)
            }
            4 => {
                // OAMDATA
                Ok(self.reg_oam_addr)
            }
            7 => {
                // PPUDATA
                let data = self.ppu_mem_map.read(self.reg_v)?;
                if self.is_mutating_read() {
                    self.increment_addr_read();
                }
                Ok(data)
            }
            _ => unreachable!()
        }
    }

    fn write(&mut self, index: u16, byte: u8) -> Result<(), EmulationError> {
        match index {
            0 => {
                // TODO: For better accuracy, replace old_is_nmi_enabled check with PPU cycle count
                let old_is_nmi_enabled = self.reg_ctrl.is_nmi_enabled;
                self.reg_ctrl.write(byte);
                if !old_is_nmi_enabled && self.reg_ctrl.is_nmi_enabled && self.reg_status.is_in_vblank {
                    self.nmi_pending = true;
                }

                // reg_t: ...GH.. ........ <- byte: ......GH
                let name_table_index = (byte & BIT_MASK_2) as u16;
                let mask: u16 = !0b0000_1100_0000_0000;
                self.reg_t &= mask;
                self.reg_t |= name_table_index << 10;
                Ok(())
            }
            1 => Ok(self.reg_mask.write(byte)),
            3 => {
                self.reg_oam_addr = byte;
                Ok(())
            }
            4 => {
                self.reg_oam_data = byte;
                self.reg_oam_addr += 1;
                Ok(())
            }
            2 => Ok(()),
            5 => {
                if !self.is_address_latch_on {
                    // First write
                    // reg_t: ....... ...ABCDE <- byte: ABCDE...
                    // reg_x:              FGH <- byte: .....FGH
                    let mask_t: u16 = 0b0000_0000_0001_111;
                    let data_t = ((byte & 0b1111_1000) >> 3) as u16;
                    self.reg_t = (self.reg_t & !mask_t) | (data_t & mask_t);

                    let data_x = byte & 0b0000_0111;
                    self.reg_x = data_x;
                    self.set_address_latch();
                } else {
                    // Second write
                    // reg_t: .FGH..AB CDE..... <- byte: ABCDEFGH
                    let mask = 0b0111_0011_1110_0000;
                    let mask_fgh = 0b0000_0111;
                    let mask_ab = 0b1100_0000;
                    let mask_cde = 0b0011_1000;
                    let acc = (((byte & mask_fgh) as u16) << 12) | (((byte & mask_ab) as u16) << 2) | (((byte & mask_cde) as u16) << 2);
                    self.reg_t = (self.reg_t & !mask) | acc;
                    self.reset_address_latch();
                }
                Ok(())
            }
            6 => {
                if !self.is_address_latch_on {
                    // First write
                    // reg_t: .CDEFGH ........ <- byte: ..CDEFGH
                    //             <unused>    <- byte: AB......
                    // reg_t: Z...... ........ <- 0 (bit Z is cleared)
                    let mask_t: u16 = 0b0011_1111_0000_0000;
                    let mask_byte: u8 = 0b0011_1111;
                    let mask_z: u16 = 0b0011_1111_1111_1111;
                    let data = ((byte & mask_byte) as u16) << 8;
                    self.reg_t = (self.reg_t & !mask_t) | data;
                    self.reg_t = self.reg_t & mask_z;
                    self.set_address_latch();
                } else {
                    // Second write
                    // reg_t: ....... ABCDEFGH <- byte: ABCDEFGH
                    // reg_v: <...all bits...> <- byte: <...all bits...>
                    let mask: u16 = 0b0000_0000_1111_1111;
                    self.reg_t = (self.reg_t & !mask) | (byte as u16);
                    self.reg_v = self.reg_t;
                    self.reset_address_latch();
                }
                Ok(())
            }
            7 => {
                let result = self.ppu_mem_map.write(self.reg_v, byte);
                self.increment_addr_read();
                result
            }
            _ => unreachable!()
        }
    }

    fn is_mutating_read(&self) -> bool {
        self.mem_map_config.is_mutating_read
    }

    fn set_is_mutating_read(&mut self, is_mutating_read: bool) {
        self.mem_map_config.is_mutating_read = is_mutating_read;
    }
}

impl Display for Ppu {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PPU: {}, {}, vbl: {}, skp_vbl: {}, ctrl: {:b} mask: {:b}, reg_v: 0x{:04X}, w_latch: {}",
               self.curr_scanline,
               self.curr_scanline_cycle,
               self.reg_status.is_in_vblank,
               self.should_skip_vbl,
               self.reg_ctrl,
               self.reg_mask,
               self.reg_v,
               self.is_address_latch_on)
    }
}
