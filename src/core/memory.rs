use std::default::Default;
use core::rom::Rom;
use core::apu::Apu;
use core::mappers::{self, Mapper};

const RAM_SIZE: usize = 0x800;

pub trait MemMapped {
    fn read(&self, index: u16) -> u8;
    fn write(&mut self, index: u16, byte: u8);

    fn read_word(&self, index: u16) -> u16 {

        // little-endian!
        let nibble_low = self.read(index);
        let nibble_high = self.read(index+1);

        let word: u16 = ((nibble_high as u16) << 8) | nibble_low as u16;

        word
    }
}

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
    fn read(&self, index: u16) -> u8 {
        self.ram[index as usize]
    }

    fn write(&mut self, index: u16, byte: u8) {
        self.ram[index as usize] = byte;
    }
}

pub struct MemMap {
    rom: Rom,
    ram: Ram,
    apu: Apu,
    mapper: Box<Mapper>,
}

impl Default for MemMap {
    fn default() -> MemMap {

        let def_mapper = mappers::default_mapper();

        MemMap {
            rom: Rom::default(),
            ram: Ram::default(),
            apu: Apu,
            mapper: def_mapper,
        }
    }
}

impl MemMap {
    pub fn new(rom: Rom) -> MemMap {

        let mapper = mappers::load_mapper_for_rom(&rom).unwrap();

        MemMap {
            rom: rom,
            ram: Ram::new(),
            apu: Apu,
            mapper: mapper,
        }
    }
}

impl MemMapped for MemMap {
    fn read(&self, index: u16) -> u8 {

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
            0...0x1FFF => {
                let index = index % 0x800;
                self.ram.read(index)
            },
            // PPU
            0x2000...0x3FFF => {
                let index = index % 0x0008;
                // self.ppu.read(index)
                panic!("Attempted unimplemented read from PPU register: 0x{:X}", index);
            },
            // APU
            0x4000...0x4015 => {
                let index = index % 0x4000;
                // self.apu.read(index)
                panic!("Attempted unimplemented read from APU register: 0x{:X}", index);
            }
            // I/O
            0x4016...0x4017 => {
                let index = index % 0x4016;
                // self.apu.read(index)
                panic!("Attempted unimplemented read from APU register: 0x{:X}", index);
            }
            0x4018...0x401f => {
                let index = index % 0x4018;
                panic!("Attempted unimplemented read from CPU Test Register: 0x{:X}", index);
            }
            0x4020...0xFFFF => {
                self.mapper.read(index)
            }
            _ => unreachable!()
        }
    }

    fn write(&mut self, index: u16, byte: u8) {
        match index {
            // RAM
            0...0x1FFF => {
                let index = index % 0x800;
                self.ram.write(index, byte);
            },
            // PPU
            0x2000...0x3FFF => {
                let index = index % 0x0008;
                // self.ppu.read(index)
                panic!("Attempted unimplemented write to PPU register: 0x{:X}", index);
            },
            // APU
            0x4000...0x4015 => {
                let index = index % 0x4000;
                // self.apu.read(index)
                panic!("Attempted unimplemented write to APU register: 0x{:X}", index);
            }
            // I/O
            0x4016...0x4017 => {
                let index = index % 0x4016;
                // self.apu.read(index)
                panic!("Attempted unimplemented write to APU register: 0x{:X}", index);
            }
            0x4018...0x401F => {
                let index = index % 0x4018;
                panic!("Attempted unimplemented write to CPU Test Register: 0x{:X}", index);
            }
            0x4020...0xFFFF => {
                self.mapper.write(index, byte);
            }
            _ => unreachable!()
        }
    }
}



