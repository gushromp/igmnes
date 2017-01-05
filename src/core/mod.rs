mod debugger;
mod mappers;
mod rom;
mod apu;
mod cpu;
mod memory;
mod instructions;

use std::path::Path;
use self::memory::*;
use self::apu::Apu;
use self::cpu::Cpu;
use self::rom::Rom;
use self::debugger::Debugger;

// 2A03 (NTSC) and 2A07 (PAL) emulation
// contains CPU (nearly identical to MOS 6502) part and APU part
pub struct Core<'a> {
    cpu: Cpu,
    debugger: Option<Debugger<'a>>,
}

impl<'a> Core<'a> {
    pub fn load_rom(file_path: &Path) -> Core {
        let rom = Rom::load_rom(file_path).unwrap();
        let mem_map = Box::new(MemMap::new(rom));

        let cpu = Cpu::new(mem_map);
        let debugger = None;

        let mut core = Core {
            cpu: cpu,
            debugger: debugger,
        };

        core
    }

    pub fn print_cpu_state(&self) {
        println!("{:#?}", self.cpu.state());
    }

    pub fn step(&mut self) {
        self.cpu.step();
    }

    pub fn attach_debugger(&'a mut self) {
        let debugger = Some(Debugger::attach(&mut self.cpu));

        self.debugger = debugger;
    }

    pub fn detach_debugger(&mut self) {
        self.debugger = None
    }
}