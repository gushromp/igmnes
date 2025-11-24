pub mod memory;
pub mod palette;

use bitflags::bitflags;

use std::fmt::Display;
use std::{array, fmt};

use crate::debug::Tracer;

use crate::mappers::MapperIrq;
use crate::memory::{MemMapConfig, MemMapped};
use crate::ppu::memory::PpuMemMap;
use crate::ppu::palette::PpuPaletteColor;

const BIT_MASK: u8 = 0b0000_0001;
const BIT_MASK_2: u8 = 0b0000_0011;

pub type PpuFrame<'a> = &'a [PpuPaletteColor];

// We use a whole byte for now, to avoid bit-packing, this type is merely for clarification
trait BitOps {
    fn get_bit(self: &Self, index: usize) -> bool;
    fn get_bit_u8(self: &Self, index: usize) -> u8;
}

impl BitOps for u8 {
    #[inline(always)]
    fn get_bit(self: &Self, index: usize) -> bool {
        self.get_bit_u8(index) != 0
    }

    #[inline(always)]
    fn get_bit_u8(self: &Self, index: usize) -> u8 {
        (self >> index) & BIT_MASK
    }
}

bitflags! {
    #[derive(Default, Copy, Clone)]
    struct PpuCtrlReg: u8 {
        const IS_NMI_ENABLED                    = 0b1000_0000;
        const IS_SLAVE_MODE                     = 0b0100_0000;
        const IS_SPRITE_HEIGHT_16               = 0b0010_0000;
        const BACKGROUND_PATTERN_TABLE_INDEX    = 0b0001_0000;
        const SPRITE_PATTERN_TABLE_INDEX        = 0b0000_1000;
        const IS_INCREMENT_MODE_32              = 0b0000_0100;  // VRAM address increment per CPU read/write of PPUDATA, (0: add 1, going across; 1: add 32, going down)
        // Name table index stored in reg_v
    }
}

impl PpuCtrlReg {
    fn hard_reset(&mut self) {
        *self = PpuCtrlReg::default();
    }
}

bitflags! {
    #[derive(Default, Copy, Clone)]
    struct PpuMaskReg: u8 {
        const IS_GREYSCALE_ENABLED = 0b0000_0001;
        const IS_SHOW_BACKGROUND_ENABLED_LEFTMOST = 0b0000_0010;
        const IS_SHOW_SPRITES_ENABLED_LEFTMOST = 0b0000_0100;
        const IS_SHOW_BACKGROUND_ENABLED = 0b0000_1000;

        // Green and red are swapped on PAL
        const IS_SHOW_SPRITES_ENABLED = 0b0001_0000;
        const IS_COLOR_EMPHASIS_RED = 0b0010_0000;
        const IS_COLOR_EMPHASIS_GREEN = 0b0100_0000;
        const IS_COLOR_EMPHASIS_BLUE = 0b1000_0000;
    }
}

impl PpuMaskReg {
    fn hard_reset(&mut self) {
        *self = PpuMaskReg::default();
    }
}

bitflags! {
    #[derive(Default, Copy, Clone)]
    struct PpuStatusReg: u8 {
        const IS_IN_VBLANK          = 0b1000_0000;
        const IS_SPRITE_0_HIT       = 0b0100_0000;
        const IS_SPRITE_OVERFLOW    = 0b0010_0000;
    }
}

impl PpuStatusReg {
    fn hard_reset(&mut self) {
        *self = PpuStatusReg::default();
    }

    fn soft_reset(&mut self) {
        // self.is_sprite_0_hit = false;
        // self.is_sprite_overflow = false;
    }
}

#[derive(Default, Copy, Clone)]
struct PpuScrollReg {
    x: u8,
    y: u8,
}

impl PpuScrollReg {
    fn hard_reset(&mut self) {
        self.x = 0;
        self.y = 0;
    }
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
        use crate::ppu::OamAttributePriority::{BACK, FRONT};
        match value {
            0 => FRONT,
            1 => BACK,
            _ => unreachable!(),
        }
    }
}

impl Into<u8> for OamAttributePriority {
    fn into(self) -> u8 {
        use crate::ppu::OamAttributePriority::{BACK, FRONT};
        match self {
            FRONT => 0,
            BACK => 1,
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

impl From<u8> for OamSpriteAttributes {
    fn from(value: u8) -> Self {
        let palette_index = value & 0b11;
        let priority_value = value.get_bit_u8(5);
        let priority = OamAttributePriority::from(priority_value);
        let is_flipped_horizontally = value.get_bit(6);
        let is_flipped_vertically = value.get_bit(7);
        OamSpriteAttributes {
            palette_index,
            priority,
            is_flipped_horizontally,
            is_flipped_vertically,
        }
    }
}

impl Into<u8> for OamSpriteAttributes {
    fn into(self) -> u8 {
        let priority_u8: u8 = self.priority.into();
        ((self.is_flipped_vertically as u8) << 7)
            | ((self.is_flipped_horizontally as u8) << 6)
            | (priority_u8 << 5)
            | self.palette_index
    }
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

impl From<&[u8]> for OamEntry {
    fn from(value: &[u8]) -> Self {
        if value.len() != 4 {
            panic!("OAM Entry size must be exactly 4 bytes")
        } else {
            let sprite_y = value[0];
            let tile_bank_index = value[1];
            let attributes = OamSpriteAttributes::from(value[2]);
            let sprite_x = value[3];
            OamEntry {
                sprite_y,
                tile_bank_index,
                attributes,
                sprite_x,
            }
        }
    }
}

impl OamEntry {
    fn write_u8(&mut self, index: usize, byte: u8) {
        match index {
            0 => self.sprite_y = byte,
            1 => self.tile_bank_index = byte,
            2 => self.attributes.write(byte),
            3 => self.sprite_x = byte,
            _ => unreachable!(),
        }
    }
    fn read(&self, index: usize) -> u8 {
        match index {
            0 => self.sprite_y,
            1 => self.tile_bank_index,
            2 => self.attributes.into(),
            3 => self.sprite_x,
            _ => unreachable!(),
        }
    }
}

#[derive(Copy, Clone)]
pub struct OamTable {
    oam_entries: [OamEntry; 64],
}

impl Default for OamTable {
    fn default() -> Self {
        OamTable {
            oam_entries: [OamEntry::default(); 64],
        }
    }
}

impl OamTable {
    pub fn write(&mut self, cpu_mem: &[u8]) {
        if cpu_mem.len() != 0x100 {
            panic!(
                "Attempted OAM DMA write with size {:2X}, expected size {:2X}",
                cpu_mem.len(),
                0x100
            )
        } else {
            for (index, chunk) in cpu_mem.chunks(4).enumerate() {
                self.oam_entries[index] = OamEntry::from(chunk);
            }
        }
    }

    pub fn write_u8(&mut self, index: u8, byte: u8) {
        let oam_entry_index = (index / 4) as usize;
        let oam_byte_index = (index % 4) as usize;

        self.oam_entries[oam_entry_index].write_u8(oam_byte_index, byte)
    }
    pub fn read(&self, index: u8) -> u8 {
        let oam_entry_index = (index / 4) as usize;
        let oam_byte_index = (index % 4) as usize;
        self.oam_entries[oam_entry_index].read(oam_byte_index)
    }
}

#[derive(Default, Copy, Clone)]
struct SecondaryOamEntry {
    oam_entry: OamEntry,
    sprite_index: usize,
}

#[derive(Default, Copy, Clone)]
struct SecondaryOam {
    oam_entries: [SecondaryOamEntry; 8],
    count: usize,
}

#[derive(Default, Copy, Clone)]
struct SpriteOutputUnit {
    secondary_oam_entry: SecondaryOamEntry,
    pattern_data: [[u8; 2]; 16],
}

#[derive(Default)]
struct SpriteOutputUnits {
    units: [SpriteOutputUnit; 8],
    count: usize,
}

#[derive(Default)]
struct SpritePixel {
    color: PpuPaletteColor,
    priority: OamAttributePriority,
    sprite_index: usize,
    is_transparent: bool,
}

#[derive(Default)]
struct BackgroundPixel {
    color: PpuPaletteColor,
    is_transparent: bool,
}

#[derive(Clone)]
pub struct PpuOutput {
    pub data: Box<[PpuPaletteColor; 256 * 240]>,
}

impl Default for PpuOutput {
    fn default() -> Self {
        PpuOutput {
            data: Box::new([PpuPaletteColor::default(); 256 * 240]),
        }
    }
}

#[derive(Default, Copy, Clone)]
struct PpuTile {
    attribute_table_entry: u8,
    pattern_table_entry: [u8; 2],
}

#[derive(Default, Copy, Clone)]
struct PpuShiftRegisters {
    reg_high_plane: u16,
    reg_low_plane: u16,
    attribute_latch_high: bool,
    attribute_latch_low: bool,

    palette_index_high: u8,
    palette_index_low: u8,
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
    pub reg_oam_addr: u8,
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
    mem_map_config: MemMapConfig,

    // Rendering data
    shift_regs: PpuShiftRegisters,
    secondary_oam: SecondaryOam,
    sprite_output_units: SpriteOutputUnits,

    curr_frame: PpuOutput,

    is_frame_ready: bool,
    output: PpuOutput,

    // Quirks

    // Reading $2002 within a few PPU clocks of when VBL is set results in special-case behavior.
    // Reading one PPU clock before reads it as clear and never sets the flag or generates NMI for that frame.
    // Reading on the same PPU clock or one later reads it as set, clears it, and suppresses the NMI for that frame.
    // Reading two or more PPU clocks before/after it's set behaves normally (reads flag's value, clears it, and doesn't affect NMI operation).
    // This suppression behavior is due to the $2002 read pulling the NMI line back up too quickly after it drops (NMI is active low) for the CPU to see it.
    // (CPU inputs like NMI are sampled each clock.)
    should_skip_vbl: bool,
    read_buffer: u8,
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
        self.reg_v & 0b1_1111
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

    #[inline]
    fn set_address_latch(&mut self) {
        self.is_address_latch_on = true;
    }

    #[inline]
    fn reset_address_latch(&mut self) {
        self.is_address_latch_on = false;
    }

    #[inline(always)]
    fn reset_vblank_status(&mut self) {
        self.reg_status.set(PpuStatusReg::IS_IN_VBLANK, false);
    }

    #[inline]
    fn is_rendering_enabled(&self) -> bool {
        let flags = PpuMaskReg::IS_SHOW_BACKGROUND_ENABLED | PpuMaskReg::IS_SHOW_SPRITES_ENABLED;
        self.reg_mask.contains(flags)
    }

    #[inline(always)]
    fn increment_addr_read(&mut self) {
        self.reg_v = if self.reg_ctrl.contains(PpuCtrlReg::IS_INCREMENT_MODE_32) {
            self.reg_v.wrapping_add(32)
        } else {
            self.reg_v.wrapping_add(1)
        }
    }

    #[inline]
    fn increment_addr_x(&mut self) {
        // if ((v & 0x001F) == 31) // if coarse X == 31
        //   v &= ~0x001F          // coarse X = 0
        //   v ^= 0x0400           // switch horizontal nametable
        // else
        //   v += 1                // increment coarse X

        if (self.reg_v & 0x001F) == 31 {
            self.reg_v &= !0x001F;
            self.reg_v ^= 0x0400;
        } else {
            self.reg_v += 1;
        }
    }

    #[inline]
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
                self.reg_v ^= 0x0800;
            } else if y == 31 {
                y = 0;
            } else {
                y += 1;
            }
            self.reg_v = (self.reg_v & !0x03E0) | (y << 5);
        }
    }

    #[inline(always)]
    pub fn clear_nmi(&mut self) {
        self.nmi_pending = false;
    }

    #[inline(always)]
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

        self.curr_scanline = 0;
        self.curr_scanline_cycle = 0;
        self.cpu_cycles = 0;

        self.output = PpuOutput::default();
        self.curr_frame = PpuOutput::default();
    }

    #[inline(always)]
    pub fn should_suppress_nmi(&self) -> bool {
        self.should_skip_vbl
    }

    #[inline(always)]
    fn fetch_tile(&mut self) -> PpuTile {
        let addr = self.reg_v;
        let name_table_entry = self.ppu_mem_map.fetch_name_table_entry(addr);
        let attribute_table_entry = self.ppu_mem_map.fetch_attribute_table_entry(addr);
        let pixel_y = (self.reg_v & 0x7000) >> 12;
        let pattern_table_entry = self
            .ppu_mem_map
            .fetch_pattern_table_entry(
                self.reg_ctrl
                    .contains(PpuCtrlReg::BACKGROUND_PATTERN_TABLE_INDEX) as u8,
                name_table_entry,
                pixel_y,
            )
            .unwrap();
        PpuTile {
            attribute_table_entry,
            pattern_table_entry,
        }
    }

    #[inline]
    fn load_shift_registers(&mut self) {
        let tile = self.fetch_tile();

        let byte_low_plane = tile.pattern_table_entry[0] as u16;
        let byte_high_plane = tile.pattern_table_entry[1] as u16;
        let mask = 0b0000_0000_1111_1111;
        self.shift_regs.reg_low_plane = (self.shift_regs.reg_low_plane & !mask) | byte_low_plane;
        self.shift_regs.reg_high_plane = (self.shift_regs.reg_high_plane & !mask) | byte_high_plane;

        let attribute_table_entry = tile.attribute_table_entry;

        let coarse_x_bit1 = ((self.reg_v & 0x1F) >> 1) & 0b1;
        let coarse_y_bit1 = (((self.reg_v >> 5) & 0x1F) >> 1) & 0b1;

        let attribute_shift: u8 = match (coarse_x_bit1, coarse_y_bit1) {
            (0, 0) => 0,
            (1, 0) => 2,
            (0, 1) => 4,
            (1, 1) => 6,
            _ => unreachable!(),
        };

        let palette_index_bits = (attribute_table_entry >> attribute_shift) & 0b11;

        let palette_index_high = (palette_index_bits >> 1) & 0b1 == 1;
        let palette_index_low = palette_index_bits & 0b1 == 1;
        self.shift_regs.attribute_latch_high = palette_index_high;
        self.shift_regs.attribute_latch_low = palette_index_low;
    }

    #[inline(always)]
    fn shift_registers_left(&mut self) {
        self.shift_regs.reg_high_plane <<= 1;
        self.shift_regs.reg_low_plane <<= 1;
        self.shift_regs.palette_index_high =
            (self.shift_regs.palette_index_high << 1) | self.shift_regs.attribute_latch_high as u8;
        self.shift_regs.palette_index_low =
            (self.shift_regs.palette_index_low << 1) | self.shift_regs.attribute_latch_low as u8;
    }

    pub fn step(&mut self, cpu_cycles: u64, tracer: &mut Tracer) -> bool {
        let cycles_to_run = (cpu_cycles - self.cpu_cycles) * 3;

        for _ in 0..cycles_to_run {
            // Rendering scanlines & cycles
            let pixel_x = self.curr_scanline_cycle.wrapping_sub(1) as usize;
            let pixel_y = self.curr_scanline as usize;
            if self.is_rendering_enabled() && pixel_y < 240 && pixel_x < 256 {
                // Background
                let background_pixel = self.get_background_pixel(pixel_x, pixel_y);
                let sprite_pixel = self.get_sprite_pixel(pixel_x, pixel_y);

                let output_color = match (
                    sprite_pixel.priority,
                    sprite_pixel.is_transparent,
                    background_pixel.is_transparent,
                ) {
                    (OamAttributePriority::FRONT, false, _)
                    | (OamAttributePriority::BACK, false, true) => sprite_pixel.color,
                    _ => background_pixel.color,
                };

                let is_sprite_0_hit = self.is_rendering_enabled()
                    && sprite_pixel.sprite_index == 0
                    && !sprite_pixel.is_transparent
                    && !background_pixel.is_transparent;
                if is_sprite_0_hit {
                    self.reg_status.set(PpuStatusReg::IS_SPRITE_0_HIT, true);
                }

                self.curr_frame.data[pixel_y * 256 + pixel_x] = output_color;
            }

            if self.is_rendering_enabled()
                && (self.curr_scanline < 240 || self.curr_scanline == 261)
            {
                if (self.curr_scanline_cycle >= 1 && self.curr_scanline_cycle <= 256)
                    || (self.curr_scanline_cycle >= 321 && self.curr_scanline_cycle <= 336)
                {
                    self.shift_registers_left();
                }

                if (self.curr_scanline_cycle >= 8 && self.curr_scanline_cycle <= 256
                    || self.curr_scanline_cycle == 328
                    || self.curr_scanline_cycle == 336)
                    && self.curr_scanline_cycle % 8 == 0
                {
                    // If rendering is enabled, the PPU increments the horizontal position in v many times across the scanline,
                    // it begins at dots 328 and 336, and will continue through the next scanline at 8, 16, 24... 240, 248, 256
                    // (every 8 dots across the scanline until 256).
                    // Across the scanline the effective coarse X scroll coordinate is incremented repeatedly,
                    // which will also wrap to the next nametable appropriately.
                    self.load_shift_registers();
                    self.increment_addr_x();
                }

                if self.curr_scanline_cycle == 256 {
                    // If rendering is enabled, the PPU increments the vertical position in v.
                    // The effective Y scroll coordinate is incremented, which is a complex operation that will correctly skip the attribute table memory regions,
                    // and wrap to the next nametable appropriately.
                    self.increment_addr_y();
                }

                if self.curr_scanline_cycle == 258 {
                    // If rendering is enabled, the PPU copies all bits related to horizontal position from t to v:
                    // reg_v: .....A.. ...BCDEF <- reg_t: .....A.. ...BCDEF
                    let mask = 0b0000_0100_0001_1111;
                    self.reg_v = (self.reg_v & !mask) | (self.reg_t & mask);

                    // We perform sprite evaluation here, to fill secondary OAM
                    self.evaluate_sprites();
                    // We fill the sprite output units based on the sprite evaluation that was previously performed
                    self.prepare_sprite_units();
                }

                if self.curr_scanline == 261
                    && self.curr_scanline_cycle >= 280
                    && self.curr_scanline_cycle <= 304
                {
                    // If rendering is enabled, at the end of vblank,
                    // shortly after the horizontal bits are copied from t to v at dot 257,
                    // the PPU will repeatedly copy the vertical bits from t to v from dots 280 to 304,
                    // completing the full initialization of v from t
                    // reg_v: .GHIA.BC DEF..... <- reg_t: .GHIA.BC DEF.....
                    let mask = 0b1111_1011_1110_0000;
                    self.reg_v = (self.reg_v & !mask) | (self.reg_t & mask);
                }
            }

            if self.curr_scanline_cycle >= 257 && self.curr_scanline_cycle <= 320 {
                self.reg_oam_addr = 0;
            }

            if self.is_vblank_starting_cycle() && !self.should_skip_vbl {
                self.reg_status.set(PpuStatusReg::IS_IN_VBLANK, true);
            }

            if self.curr_scanline == 241 && self.curr_scanline_cycle == 1 {
                if self.is_rendering_enabled() {
                    std::mem::swap(&mut self.output, &mut self.curr_frame)
                }
                self.is_frame_ready = true;
                if self.reg_ctrl.contains(PpuCtrlReg::IS_NMI_ENABLED) && !self.should_skip_vbl {
                    self.nmi_pending = true;
                }
            }

            if self.curr_scanline == 261 && self.curr_scanline_cycle == 1 {
                self.reg_status = PpuStatusReg::empty();
                self.is_odd_frame = !self.is_odd_frame;
                self.should_skip_vbl = false;
                self.nmi_pending = false;
                self.is_frame_ready = false;
            }

            if self.curr_scanline_cycle == 341
                || (self.curr_scanline == 261
                    && self.curr_scanline_cycle == 340
                    && self.is_odd_frame
                    && self.is_rendering_enabled())
            {
                self.curr_scanline_cycle = 0;
                self.curr_scanline += 1;
            }
            if self.curr_scanline == 262 {
                self.curr_scanline = 0;
            }
            self.curr_scanline_cycle += 1;
        }

        if tracer.is_enabled() {
            tracer.add_ppu_trace(&self);
        }

        self.cpu_cycles = cpu_cycles;
        self.reg_status.contains(PpuStatusReg::IS_IN_VBLANK) && self.nmi_pending
    }

    fn get_background_pixel(&mut self, pixel_x: usize, _pixel_y: usize) -> BackgroundPixel {
        if pixel_x < 8
            && !self
                .reg_mask
                .contains(PpuMaskReg::IS_SHOW_BACKGROUND_ENABLED_LEFTMOST)
        {
            let color = self.ppu_mem_map.palette.get_background_color(0, 0);
            BackgroundPixel {
                color,
                is_transparent: true,
            }
        } else {
            let pixel_index_x = 15 - self.reg_x as usize;
            let pattern_bit_plane_low = (self.shift_regs.reg_low_plane >> pixel_index_x) & 0b1;
            let pattern_bit_plane_high = (self.shift_regs.reg_high_plane >> pixel_index_x) & 0b1;
            let palette_index_high =
                (self.shift_regs.palette_index_high >> pixel_index_x % 8) & 0b1;
            let palette_index_low = (self.shift_regs.palette_index_low >> pixel_index_x % 8) & 0b1;
            let palette_index = palette_index_high << 1 | palette_index_low;
            let color_index = (pattern_bit_plane_high << 1 | pattern_bit_plane_low) as u8;
            let color = self
                .ppu_mem_map
                .palette
                .get_background_color(palette_index, color_index);

            BackgroundPixel {
                color,
                is_transparent: color_index == 0,
            }
        }
    }

    #[inline]
    fn get_sprite_pixel(&self, pixel_x: usize, pixel_y: usize) -> SpritePixel {
        let mut color = self.ppu_mem_map.palette.get_transparent_color();
        let mut priority = OamAttributePriority::default();
        let mut sprite_index = 0;
        let mut is_transparent = true;

        let sprite_height_pixels = if self.reg_ctrl.contains(PpuCtrlReg::IS_SPRITE_HEIGHT_16) {
            16
        } else {
            8
        };

        for index in (0..self.sprite_output_units.count).rev() {
            let unit = self.sprite_output_units.units[index];
            if pixel_x < 8
                && !self
                    .reg_mask
                    .contains(PpuMaskReg::IS_SHOW_SPRITES_ENABLED_LEFTMOST)
            {
                color = self.ppu_mem_map.palette.get_sprite_color(0, 0);
                priority = unit.secondary_oam_entry.oam_entry.attributes.priority;
                sprite_index = unit.secondary_oam_entry.sprite_index;
                is_transparent = true;
            } else {
                let sprite_first_pixel_x = unit.secondary_oam_entry.oam_entry.sprite_x as usize;
                let sprite_first_pixel_y =
                    (unit.secondary_oam_entry.oam_entry.sprite_y.wrapping_add(1)) as usize;
                let sprite_index_y = pixel_y - sprite_first_pixel_y;
                if pixel_x < sprite_first_pixel_x
                    || pixel_x > sprite_first_pixel_x + 7
                    || sprite_index_y > (sprite_height_pixels - 1)
                {
                    continue;
                }

                let pixel_line = unit.pattern_data[sprite_index_y];

                let pixel_index_x =
                    7 - (pixel_x - unit.secondary_oam_entry.oam_entry.sprite_x as usize);
                let pattern_bit_plane_low = (pixel_line[0] >> pixel_index_x) & 0b1;
                let pattern_bit_plane_high = (pixel_line[1] >> pixel_index_x) & 0b1;

                let palette_index = unit.secondary_oam_entry.oam_entry.attributes.palette_index;
                let color_index = (pattern_bit_plane_high << 1) | pattern_bit_plane_low;
                if color_index > 0 {
                    color = self
                        .ppu_mem_map
                        .palette
                        .get_sprite_color(palette_index, color_index);
                    priority = unit.secondary_oam_entry.oam_entry.attributes.priority;
                    sprite_index = unit.secondary_oam_entry.sprite_index;
                    is_transparent = false;
                }
            }
        }
        SpritePixel {
            color,
            priority,
            sprite_index,
            is_transparent,
        }
    }

    fn evaluate_sprites(&mut self) {
        self.secondary_oam.count = 0;

        let sprite_height_pixels = if self.reg_ctrl.contains(PpuCtrlReg::IS_SPRITE_HEIGHT_16) {
            16
        } else {
            8
        };

        let next_scanline_index = ((self.curr_scanline + 1) % 262) as usize;
        let mut num_found_sprites = 0;
        for (sprite_index, oam_entry) in self.ppu_mem_map.oam_table.oam_entries.iter().enumerate() {
            let sprite_y_first_pixel = oam_entry.sprite_y.saturating_add(1) as usize;
            let sprite_y_last_pixel = sprite_y_first_pixel + sprite_height_pixels - 1;
            let is_overflowing_y = sprite_y_last_pixel >= 240;
            if next_scanline_index > 0
                && next_scanline_index >= sprite_y_first_pixel
                && (next_scanline_index <= sprite_y_last_pixel || is_overflowing_y)
            {
                if num_found_sprites < 8 {
                    self.secondary_oam.oam_entries[num_found_sprites] = SecondaryOamEntry {
                        oam_entry: *oam_entry,
                        sprite_index,
                    };
                    num_found_sprites += 1;
                } else {
                    if sprite_y_first_pixel > 0 && sprite_y_first_pixel <= 240 {
                        self.reg_status.set(PpuStatusReg::IS_SPRITE_OVERFLOW, true);
                    }
                }
            }
        }
        self.secondary_oam.count = num_found_sprites;
    }

    fn prepare_sprite_units(&mut self) {
        self.sprite_output_units.count = self.secondary_oam.count;

        for index in 0..self.secondary_oam.count {
            let secondary_oam_entry = self.secondary_oam.oam_entries[index];

            let mut pattern_data_bitplanes: [[u8; 2]; 16] = [[0; 2]; 16];

            if self.reg_ctrl.contains(PpuCtrlReg::IS_SPRITE_HEIGHT_16) {
                // 8x16 sprites
                let pattern_entry_byte = secondary_oam_entry.oam_entry.tile_bank_index;
                let pattern_table_index = pattern_entry_byte & 0b1;

                let pattern_entry_index_top = pattern_entry_byte & 0xFE;
                let pattern_entry_index_bottom = pattern_entry_index_top + 1;

                let mut pattern_data_top = self
                    .ppu_mem_map
                    .fetch_sprite_pattern(pattern_table_index, pattern_entry_index_top);
                let mut pattern_data_bottom = self
                    .ppu_mem_map
                    .fetch_sprite_pattern(pattern_table_index, pattern_entry_index_bottom);

                if secondary_oam_entry
                    .oam_entry
                    .attributes
                    .is_flipped_vertically
                {
                    let temp = pattern_data_top;
                    pattern_data_top = pattern_data_bottom;
                    pattern_data_bottom = temp;

                    pattern_data_top = Self::flip_pattern_data_vertically(pattern_data_top);
                    pattern_data_bottom = Self::flip_pattern_data_vertically(pattern_data_bottom);
                }

                if secondary_oam_entry
                    .oam_entry
                    .attributes
                    .is_flipped_horizontally
                {
                    pattern_data_top = Self::flip_pattern_data_horizontally(pattern_data_top);
                    pattern_data_bottom = Self::flip_pattern_data_horizontally(pattern_data_bottom);
                }

                for index in 0..8 {
                    pattern_data_bitplanes[index][0] = pattern_data_top[index];
                    pattern_data_bitplanes[index][1] = pattern_data_top[index + 8];
                }

                for index in 8..16 {
                    pattern_data_bitplanes[index][0] = pattern_data_bottom[index - 8];
                    pattern_data_bitplanes[index][1] = pattern_data_bottom[index];
                }
            } else {
                // 8x8 sprites
                let pattern_table_index =
                    self.reg_ctrl
                        .contains(PpuCtrlReg::SPRITE_PATTERN_TABLE_INDEX) as u8;
                let pattern_entry_index = secondary_oam_entry.oam_entry.tile_bank_index;
                let mut pattern_data = self
                    .ppu_mem_map
                    .fetch_sprite_pattern(pattern_table_index, pattern_entry_index);

                if secondary_oam_entry
                    .oam_entry
                    .attributes
                    .is_flipped_vertically
                {
                    pattern_data = Self::flip_pattern_data_vertically(pattern_data);
                }

                if secondary_oam_entry
                    .oam_entry
                    .attributes
                    .is_flipped_horizontally
                {
                    pattern_data = Self::flip_pattern_data_horizontally(pattern_data);
                }

                for index in 0..8 {
                    pattern_data_bitplanes[index][0] = pattern_data[index];
                    pattern_data_bitplanes[index][1] = pattern_data[index + 8];
                }
            }

            self.sprite_output_units.units[index] = SpriteOutputUnit {
                secondary_oam_entry: secondary_oam_entry,
                pattern_data: pattern_data_bitplanes,
            };
        }

        for _ in self.secondary_oam.count..8 {
            // We must fetch pattern data even if no sprite exists to toggle A12.
            // The PPU typically fetches the pattern for tile 0xFF in this case.
            let dummy_tile_index = 0xFF;
            if self.reg_ctrl.contains(PpuCtrlReg::IS_SPRITE_HEIGHT_16) {
                let pattern_table_index = dummy_tile_index & 0b1;
                let _ = self
                    .ppu_mem_map
                    .fetch_sprite_pattern(pattern_table_index, dummy_tile_index & 0xFE);
                let _ = self
                    .ppu_mem_map
                    .fetch_sprite_pattern(pattern_table_index, (dummy_tile_index & 0xFE) + 1);
            } else {
                let pattern_table_index =
                    self.reg_ctrl
                        .contains(PpuCtrlReg::SPRITE_PATTERN_TABLE_INDEX) as u8;
                let _ = self
                    .ppu_mem_map
                    .fetch_sprite_pattern(pattern_table_index, dummy_tile_index);
            }
        }
    }

    fn flip_pattern_data_vertically(pattern_data: [u8; 16]) -> [u8; 16] {
        array::from_fn(|i| {
            if i < 8 {
                // Flip the low plane (indices 0-7)
                pattern_data[7 - i]
            } else {
                // Flip the high plane (indices 8-15)
                // (i - 8) gets us back to 0-7 range, flip it, then add offset back
                pattern_data[8 + (7 - (i - 8))]
            }
        })
    }

    #[inline(always)]
    fn flip_pattern_data_horizontally(mut pattern_data: [u8; 16]) -> [u8; 16] {
        for x in &mut pattern_data {
            *x = x.reverse_bits();
        }
        pattern_data
    }

    #[inline(always)]
    pub fn is_frame_ready(&self) -> bool {
        self.is_frame_ready
    }

    #[inline(always)]
    pub fn get_frame(&mut self) -> PpuFrame<'_> {
        self.is_frame_ready = false;
        &self.output.data.as_slice()
    }

    #[inline(always)]
    fn clock_mapper_irq(&mut self) {
        self.ppu_mem_map.mapper.clock_irq(self.reg_v);
    }
}

//

impl MemMapped for Ppu {
    fn read(&mut self, index: u16) -> u8 {
        match index {
            0 | 1 | 3 | 5 | 6 => 0, // Err(MemoryAccess(format!("Attempted read from write-only PPU register with index {}.", index))),
            2 => {
                // PPUSTATUS
                let value = self.reg_status.bits();

                // Reading $2002 within a few PPU clocks of when VBL is set results in special-case behavior.
                // Reading one PPU clock before reads it as clear and never sets the flag or generates NMI for that frame.
                // Reading on the same PPU clock or one later reads it as set, clears it, and suppresses the NMI for that frame.
                // Reading two or more PPU clocks before/after it's set behaves normally (reads flag's value, clears it, and doesn't affect NMI operation).
                // This suppression behavior is due to the $2002 read pulling the NMI line back up too quickly after it drops (NMI is active low) for the CPU to see it.
                // (CPU inputs like NMI are sampled each clock.)
                if self.is_mutating_read() {
                    if self.curr_scanline == 241 {
                        if self.curr_scanline_cycle == 0 {
                            self.should_skip_vbl = true;
                            self.nmi_pending = false;
                        } else if self.curr_scanline_cycle == 1
                            || self.curr_scanline_cycle == 2
                            || self.curr_scanline_cycle == 3
                        {
                            self.should_skip_vbl = true;
                            self.nmi_pending = false;
                        }
                    }

                    // Reading from this register also resets the write latch and vblank active flag
                    self.reset_address_latch();
                    self.reset_vblank_status();
                }

                value
            }
            4 => {
                // OAMDATA
                if self.is_mutating_read() {
                    self.reg_oam_data = self.ppu_mem_map.oam_table.read(self.reg_oam_addr);
                }
                self.reg_oam_data
            }
            7 => {
                // PPUDATA
                let data = if (0x3F00..=0x3FFF).contains(&self.reg_v) {
                    // Reads from palette RAM are not buffered
                    self.ppu_mem_map.read(self.reg_v)
                } else {
                    self.read_buffer
                };
                if self.is_mutating_read() {
                    self.read_buffer = self.ppu_mem_map.read(self.reg_v);
                    self.increment_addr_read();
                    self.clock_mapper_irq();
                }
                data
            }
            _ => unreachable!(),
        }
    }

    fn write(&mut self, index: u16, byte: u8) {
        match index {
            0 => {
                // TODO: For better accuracy, replace old_is_nmi_enabled check with PPU cycle count
                let old_is_nmi_enabled = self.reg_ctrl.contains(PpuCtrlReg::IS_NMI_ENABLED);
                self.reg_ctrl = PpuCtrlReg::from_bits_truncate(byte);

                if !old_is_nmi_enabled
                    && self.reg_ctrl.contains(PpuCtrlReg::IS_NMI_ENABLED)
                    && self.reg_status.contains(PpuStatusReg::IS_IN_VBLANK)
                    && self.curr_scanline_cycle > 3
                {
                    self.nmi_pending = true;
                }
                if !self.reg_ctrl.contains(PpuCtrlReg::IS_NMI_ENABLED) {
                    self.nmi_pending = false;
                }

                // reg_t: ....GH.. ........ <- byte: ......GH
                let name_table_index = ((byte & BIT_MASK_2) as u16) << 10;
                let mask: u16 = 0b0000_1100_0000_0000;
                self.reg_t = (self.reg_t & !mask) | (name_table_index & mask);
            }
            1 => self.reg_mask = PpuMaskReg::from_bits_truncate(byte),
            2 => (),
            3 => {
                self.reg_oam_addr = byte;
            }
            4 => {
                self.ppu_mem_map.oam_table.write_u8(self.reg_oam_addr, byte);
                self.reg_oam_addr = self.reg_oam_addr.wrapping_add(1);
            }
            5 => {
                if !self.is_address_latch_on {
                    // First write
                    // reg_t: ....... ...ABCDE <- byte: ABCDE...
                    // reg_x:              FGH <- byte: .....FGH
                    let mask_t: u16 = 0b0000_0000_0011_111;
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
                    let acc = (((byte & mask_fgh) as u16) << 12)
                        | ((((byte & mask_ab) as u16) >> 6) << 8)
                        | ((((byte & mask_cde) as u16) >> 3) << 5);
                    self.reg_t = (self.reg_t & !mask) | acc;
                    self.reset_address_latch();
                }
            }
            6 => {
                if !self.is_address_latch_on {
                    // First write
                    // reg_t: ..CDEFGH ........ <- byte: ..CDEFGH
                    //             <unused>    <- byte: AB......
                    // reg_t: Z...... ........ <- 0 (bit Z is cleared)
                    let mask_t: u16 = 0b0011_1111_0000_0000;
                    let mask_byte: u8 = 0b0011_1111;
                    let mask_z: u16 = 0b1011_1111_1111_1111;
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
                self.clock_mapper_irq();
            }
            7 => {
                let result = self.ppu_mem_map.write(self.reg_v, byte);
                self.increment_addr_read();
                self.clock_mapper_irq();
                result
            }
            _ => unreachable!(),
        }
    }

    fn is_mutating_read(&self) -> bool {
        self.mem_map_config.is_mutating_read
    }

    fn set_is_mutating_read(&mut self, is_mutating_read: bool) {
        self.mem_map_config.is_mutating_read = is_mutating_read;
    }

    fn read_range(&mut self, _range: std::ops::Range<u16>) -> &[u8] {
        unimplemented!()
    }
}

impl Display for Ppu {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PPU: {}, {}, vbl: {}, skp_vbl: {}, ctrl: {:b} mask: {:b}, reg_v: 0x{:04X}, w_latch: {}",
               self.curr_scanline,
               self.curr_scanline_cycle,
               self.reg_status.contains(PpuStatusReg::IS_IN_VBLANK),
               self.should_skip_vbl,
               self.reg_ctrl,
               self.reg_mask,
               self.reg_v,
               self.is_address_latch_on)
    }
}
