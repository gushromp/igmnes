use std::default::Default;
use core::rom::Rom;
use core::apu::Apu;
use core::mappers::{self, Mapper};
use core::errors::EmulationError;
use core::dyn_clone::clone_box;

const RAM_SIZE: usize = 0x800;

pub trait MemMapped {
    fn read(&self, index: u16) -> Result<u8, EmulationError>;
    fn write(&mut self, index: u16, byte: u8) -> Result<(), EmulationError>;

    fn read_word(&self, index: u16) -> Result<u16, EmulationError> {

        // little-endian!
        let nibble_low = self.read(index)?;
        let nibble_high = self.read(index+1)?;

        let word: u16 = ((nibble_high as u16) << 8) | nibble_low as u16;

        Ok(word)
    }
}

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
    fn read(&self, index: u16) -> Result<u8, EmulationError> {
        Ok(self.ram[index as usize])
    }

    fn write(&mut self, index: u16, byte: u8) -> Result<(), EmulationError> {
        self.ram[index as usize] = byte;
        Ok(())
    }
}

pub struct MemMap {
    rom: Rom,
    ram: Ram,
    pub apu: Apu,
    pub ppu: Vec<u8>, // dummy
    mapper: Box<dyn Mapper>,
}

impl Clone for MemMap {
    fn clone(&self) -> Self {
        MemMap {
            mapper: clone_box(&*self.mapper),
            ..Default::default()
        }
    }
}

impl Default for MemMap {
    fn default() -> MemMap {

        let def_mapper = mappers::default_mapper();

        MemMap {
            rom: Rom::default(),
            ram: Ram::default(),
            apu: Apu::default(),
            ppu: vec![0; 8],
            mapper: def_mapper,
        }
    }
}

impl MemMap {
    pub fn new(rom: Rom) -> MemMap {

        let mapper = mappers::load_mapper_for_rom(&rom).unwrap();

        let mem_map = MemMap {
            rom: rom,
            ram: Ram::new(),
            apu: Apu::new(),
            ppu: vec![0xFF; 8],
            mapper: mapper,
        };

        mem_map
    }
}

impl MemMapped for MemMap {
    #[inline]
    fn read(&self, index: u16) -> Result<u8, EmulationError> {

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

        match index {
            // RAM
            0..=0x1FFF => {
                let index = index % 0x800;
                self.ram.read(index)
            },
            // PPU
            0x2000..=0x3FFF => {
                //println!("Attempted read from dummy PPU register: 0x{:04X}", index);
                let index = index % 0x0008;
                Ok(self.ppu[index as usize])
            },
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
                self.mapper.read(index)
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
            },
            // PPU
            0x2000..=0x3FFF => {
                //println!("Attempted write to dummy PPU register: 0x{:X}", index);
                let index = index % 0x0008;
                self.ppu[index as usize] = byte;
                Ok(())
            },
            // APU
            0x4000..=0x4013 | 0x4015 => {
                self.apu.write(index, byte)

            }
            // OAM DMA register
            0x4014 => {
                println!("Attempted write to dummy APU register: 0x{:04X}", index);
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
                self.mapper.write(index, byte)
            }
            _ => unreachable!()
        }
    }
}



