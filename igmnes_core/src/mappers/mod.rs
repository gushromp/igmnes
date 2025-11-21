mod mapper_000;
mod mapper_002;
mod mapper_003;
mod mapper_007;

use self::mapper_000::NRom;
use crate::mappers::mapper_002::UxROM;
use crate::mappers::mapper_003::CNROM;
use crate::mappers::mapper_007::AxROM;
use crate::memory::MemMapped;
use crate::rom::Rom;
use enum_dispatch::enum_dispatch;
use std::ops::{Deref, DerefMut, Range};

#[enum_dispatch]
pub trait Mapper: Sized {
    fn hard_reset(&mut self, rom: &Rom);
}

#[enum_dispatch]
pub trait CpuMapper: MemMapped {
    // Reads from PRG ROM
    fn read_prg_rom(&self, index: u16) -> u8;
    // Reads/Writes to PRG RAM
    fn read_prg_ram(&self, index: u16) -> u8;
    fn write_prg_ram(&mut self, index: u16, byte: u8);
}

#[enum_dispatch]
pub trait PpuMapper: MemMapped {
    // Reads from CHR ROM
    fn read_chr_rom(&self, index: u16) -> u8;
    fn read_chr_rom_range(&self, range: Range<u16>) -> &[u8];

    // Reads/Writes to CHR RAM
    fn read_chr_ram(&self, index: u16) -> u8;
    fn read_chr_ram_range(&self, range: Range<u16>) -> &[u8];
    fn write_chr_ram(&mut self, index: u16, byte: u8);

    fn get_mirrored_index(&self, index: u16) -> u16;
}

// pub trait Mapper : CpuMapper + PpuMapper {}

#[enum_dispatch(Mapper, CpuMapper, PpuMapper, MemMapped)]
pub enum MapperImpl {
    Mapper000(NRom),
    Mapper002(UxROM),
    Mapper003(CNROM),
    Mapper007(AxROM),
}

pub fn load_mapper_for_rom(rom: &Rom) -> Result<MapperImpl, String> {
    let mapper: MapperImpl = match rom.header.mapper_number {
        0 => NRom::new(rom).into(),
        2 => UxROM::new(rom).into(),
        3 => CNROM::new(rom).into(),
        7 => AxROM::new(rom).into(),
        mapper_num @ _ => return Err(format!("Unsupported mapper number: {}", mapper_num)),
    };
    Ok(mapper)
}

#[derive(Clone, Copy)]
pub struct SharedMapper {
    ptr: *mut MapperImpl,
}

impl SharedMapper {
    pub fn new(mapper: &mut MapperImpl) -> Self {
        Self {
            ptr: mapper as *mut _,
        }
    }

    // This is unsafe because the compiler can't guarantee aliasing rules,
    // but WE know the CPU and PPU don't run simultaneously.
    #[inline(always)]
    pub fn get(&self) -> &mut MapperImpl {
        unsafe { &mut *self.ptr }
    }
}

impl Default for SharedMapper {
    fn default() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
        }
    }
}

impl Deref for SharedMapper {
    type Target = MapperImpl;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl DerefMut for SharedMapper {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}

pub fn default_mapper() -> MapperImpl {
    let def_rom = Rom::default();
    NRom::new(&def_rom).into()
}
