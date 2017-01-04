mod rom;
mod mappers;
mod apu;
mod cpu;
mod memory;
mod instructions;

use std::path::Path;
use self::memory::*;
use self::apu::Apu;
use self::cpu::Cpu;
use self::rom::Rom;

// 2A03 (NTSC) and 2A07 (PAL) emulation
// contains CPU (nearly identical to MOS 6502) part and APU part
pub struct Core {
    cpu: Cpu,
}

impl Core {
    pub fn load_rom(file_path: &Path) -> Core {


        let rom = Rom::load_rom(file_path).unwrap();
        println!("{:#?}", rom.header);
        let mem_map = Box::new(MemMap::new(rom));

        let core = Core {
            cpu: Cpu::new(mem_map),

        };

        core
    }

}