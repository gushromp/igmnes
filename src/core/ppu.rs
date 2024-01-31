use core::errors::EmulationError;
use core::errors::EmulationError::MemoryAccess;
use core::memory::{MemMapped, PpuMemMap};

const BIT_MASK: u8 = 0b0000_0001;
const BIT_MASK_2: u8 = 0b0000_0011;

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

#[derive(Default)]
struct PpuCtrlReg {
    is_nmi_enabled: bool,
    is_master_enabled: bool,
    sprite_height: u8,
    background_tile: u8,
    sprite_tile: u8,
    is_increment_mode: bool,
    name_table: u8,
}

impl PpuCtrlReg {
    fn write(&mut self, byte: u8) {
        self.is_nmi_enabled = byte.get_bit(7);
        self.is_master_enabled = byte.get_bit(6);
        self.sprite_height = byte.get_bit_u8(5);
        self.background_tile = byte.get_bit_u8(4);
        self.sprite_tile = byte.get_bit_u8(3);
        self.is_increment_mode = byte.get_bit(2);
        self.name_table = byte & BIT_MASK_2;
    }
}

#[derive(Default)]
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
}

#[derive(Default)]
struct PpuStatusReg {
    is_in_vblank: bool,
    is_sprite_0_hit: bool,
    is_sprite_overflow: bool,
}

impl PpuStatusReg {
    fn read(&mut self) -> u8 {
        let value = (self.is_in_vblank as u8) << 7 | (self.is_sprite_0_hit as u8) << 6 | (self.is_sprite_overflow as u8) << 5;
        self.is_in_vblank = false;
        return value;
    }
}

#[derive(Default)]
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
}

#[derive(Default)]
pub struct Ppu {
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

    // Write only, 2x
    reg_addr: u8,
    // Read/write
    reg_data: u8,

    // Internal/operational flags
    is_address_latch_on: bool,

    memory_map: PpuMemMap
}

impl Ppu {
    pub fn new(memory_map: PpuMemMap) -> Self {
        Ppu {
            memory_map,
            .. Self::default()
        }
    }
    fn toggle_address_latch(&mut self) {
        self.is_address_latch_on = !self.is_address_latch_on;
    }

    fn reset_address_latch(&mut self) {
        self.is_address_latch_on = false;
    }
}

impl MemMapped for Ppu {
    fn read(&mut self, index: u16) -> Result<u8, EmulationError> {
        match index {
            0x2000..=0x3FFF => {
                let index = index % 8;
                match index {
                    0 | 1 | 3 | 5 => Err(MemoryAccess(format!("Attempted read from write-only PPU register with index {}.", index))),
                    2 => {
                        // PPUSTATUS
                        let value = self.reg_status.read();

                        // Reading from this register also resets the write latch
                        self.reset_address_latch();

                        Ok(value)
                    },
                    4 => {
                        // OAMDATA
                        Ok(self.reg_oam_addr)
                    },
                    8 => {
                        // PPUDATA
                        Ok(self.reg_data)
                    },
                    _ => unreachable!()
                }
            },
            _ => unreachable!()
        }
    }

    fn write(&mut self, index: u16, byte: u8) -> Result<(), EmulationError> {
        todo!()
    }
}