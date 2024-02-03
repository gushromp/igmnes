use std::cell::RefCell;
use std::default::Default;
use std::rc::Rc;
use core::rom::Rom;
use core::apu::Apu;
use core::mappers::{self, Mapper};
use core::errors::EmulationError;
use core::ppu::Ppu;

const RAM_SIZE: usize = 0x800;

pub trait MemMapped {
    fn read(&mut self, index: u16) -> Result<u8, EmulationError>;
    fn write(&mut self, index: u16, byte: u8) -> Result<(), EmulationError>;

    fn read_word(&mut self, index: u16) -> Result<u16, EmulationError> {

        // little-endian!
        let nibble_low = self.read(index)?;
        let nibble_high = self.read(index + 1)?;

        let word: u16 = ((nibble_high as u16) << 8) | nibble_low as u16;

        Ok(word)
    }

    fn is_mutating_read(&self) -> bool {
        true
    }

    fn set_is_mutating_read(&mut self, is_mutating_read: bool) { }
}

pub trait CpuMemMapped: MemMapped {}

pub trait PpuMemMapped: MemMapped {}

#[derive(Clone)]
pub struct Ram {
    ram: [u8; RAM_SIZE],
}

impl Default for Ram {
    fn default() -> Ram {
        Ram::new()
    }
}

impl Ram {
    pub fn new() -> Ram {
        Ram {
            ram: [0; RAM_SIZE]
        }
    }
}

impl MemMapped for Ram {
    fn read(&mut self, index: u16) -> Result<u8, EmulationError> {
        Ok(self.ram[index as usize])
    }

    fn write(&mut self, index: u16, byte: u8) -> Result<(), EmulationError> {
        self.ram[index as usize] = byte;
        Ok(())
    }
}

pub struct PpuMemMap {
    ram: Ram,
    mapper: Rc<RefCell<dyn Mapper>>,
}

impl Default for PpuMemMap {
    fn default() -> Self {
        let def_mapper = mappers::default_mapper();

        PpuMemMap {
            ram: Ram::default(),
            mapper: def_mapper,
        }
    }
}

impl PpuMemMap {
    pub fn new(mapper: Rc<RefCell<dyn Mapper>>) -> PpuMemMap {
        PpuMemMap {
            ram: Ram::default(),
            mapper,
        }
    }
}

pub struct CpuMemMap {
    rom: Rom,
    ram: Ram,
    pub apu: Apu,
    pub ppu: Ppu,
    pub ppu_mem_map: PpuMemMap,
    mapper: Rc<RefCell<dyn Mapper>>
}


impl Default for CpuMemMap {
    fn default() -> CpuMemMap {
        let def_mapper = mappers::default_mapper();

        CpuMemMap {
            rom: Rom::default(),
            ram: Ram::default(),
            apu: Apu::default(),
            ppu: Ppu::default(),
            ppu_mem_map: PpuMemMap::default(),
            mapper: def_mapper
        }
    }
}

impl CpuMemMap {
    pub fn new(rom: Rom) -> CpuMemMap {
        let mapper = mappers::load_mapper_for_rom(&rom).unwrap();

        let ppu_mem_map = PpuMemMap::new(mapper.clone());
        let mem_map = CpuMemMap {
            rom,
            ram: Ram::new(),
            apu: Apu::new(),
            ppu: Ppu::new(),
            ppu_mem_map,
            mapper: mapper.clone()
        };

        mem_map
    }
}

//

impl MemMapped for CpuMemMap {
    //        Address range	Size	Device
    //        $0000-$07FF	$0800	2KB internal RAM
    //        $0800-$0FFF	$0800	Mirrors of $0000-$07FF
    //        $1000-$17FF	$0800
    //        $1800-$1FFF	$0800
    //        $2000-$2007	$0008	NES PPU registers
    //        $2008-$3FFF	$1FF8	Mirrors of $2000-2007 (repeats every 8 bytes)
    //        $4000-$4017	$0018	NES APU and I/O registers
    //        $4018-$401F	$0008	APU and I/O functionality that is normally disabled. See CPU Test Mode.
    //        $4020-$FFFF	$BFE0	Cartridge space: PRG ROM, PRG RAM, and mapper registers (See Note)
    #[inline]
    fn read(&mut self, index: u16) -> Result<u8, EmulationError> {
        match index {
            // RAM
            0..=0x1FFF => {
                let index = index % 0x800;
                self.ram.read(index)
            }
            // PPU
            0x2000..=0x3FFF => {
                let index = index % 0x8;
                self.ppu.read(index)
            }
            // APU
            0x4000..=0x4013 | 0x4015 => {
                self.apu.read(index)
            }
            // OAM DMA register
            0x4014 => {
                //println!("Attempted read from unimplemented OAM DMA register");
                Ok(0)
            }
            // I/O
            0x4016 => {
                // self.apu.read(index)
                //println!("Attempted unimplemented read from I/O register: 0x{:04X}", index);
                Ok(0)
            }
            // I/O, Apu: This address is shared by both the APU and I/O so we can from read either one
            0x4017 => {
                self.apu.read(index)
            }
            0x4018..=0x401f => {
                let _index = index % 0x4018;
                //println!("Attempted unimplemented read from CPU Test Register: 0x{:04X}", index);
                Ok(0)
            }
            0x4020..=0xFFFF => {
                self.mapper.borrow_mut().read(index)
            }
            _ => unreachable!()
        }
    }

    #[inline]
    fn write(&mut self, index: u16, byte: u8) -> Result<(), EmulationError> {
        match index {
            // RAM
            0..=0x1FFF => {
                let index = index % 0x800;
                self.ram.write(index, byte)
            }
            // PPU
            0x2000..=0x3FFF => {
                let index = index % 0x8;
                self.ppu.write(index, byte)
            }
            // APU
            0x4000..=0x4013 | 0x4015 => {
                self.apu.write(index, byte)
            }
            // OAM DMA register
            0x4014 => {
                println!("Attempted write to unimplemented OAM DMA register: 0x{:04X}", index);
                Ok(())
            }
            // I/O
            0x4016 => {
                println!("Attempted unimplemented write to I/O register: 0x{:X}", index);
                Ok(())
            }
            // This address is shared by both APU and I/O so we need to write the value to both
            0x4017 => {
                self.apu.write(index, byte)
                //self.io.write(addr, byte);
            }
            0x4018..=0x401F => {
                let index = index % 0x4018;
                println!("Attempted unimplemented write to CPU Test Register: 0x{:X}", index);
                Ok(())
            }
            0x4020..=0xFFFF => {
                self.mapper.borrow_mut().write(index, byte)
            }
            _ => unreachable!()
        }
    }

    fn set_is_mutating_read(&mut self, is_mutating_read: bool) {
        self.ppu.set_is_mutating_read(is_mutating_read);
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
                self.ram.read(index)
            }
            0x3000..=0x3EFF => {
                // Mirror of 0x2000..=0x2EFF
                self.ram.read(index - 0x1000)
            }
            0x3F00..=0x3FFF => {
                let index = index % 20;
                // TODO Palette RAM
                Ok(0)
            }
            _ => unreachable!()
        }
    }

    fn write(&mut self, index: u16, byte: u8) -> Result<(), EmulationError> {
        Ok(())
    }
}

impl PpuMemMapped for PpuMemMap {}


