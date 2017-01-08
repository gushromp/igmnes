mod debugger;
mod mappers;
mod rom;
mod apu;
mod cpu;
mod memory;
mod instructions;

use std::error::Error;
use std::path::Path;
use std::mem;
use self::memory::*;
use self::apu::Apu;
use self::cpu::Cpu;
use self::rom::Rom;
use self::debugger::Debugger;
use self::debugger::frontends::terminal::TerminalDebugger;

pub trait CpuFacade {
    fn cpu(self: Box<Self>) -> Box<Cpu>;
    fn debugger(&mut self) -> Option<&mut Debugger>;

    fn step(&mut self, mem_map: &mut MemMapped) -> u8;
}

// 2A03 (NTSC) and 2A07 (PAL) emulation
// contains CPU (nearly identical to MOS 6502) part and APU part
pub struct Core {
    mem_map: MemMap,
    cpu_facade: Box<CpuFacade>,
    is_debugger_attached: bool,
}

impl Core {
    pub fn load_rom(file_path: &Path) -> Result<Core, Box<Error>> {
        let rom = Rom::load_rom(file_path)?;
        let mem_map = MemMap::new(rom);

        let cpu = Box::new(Cpu::new(&mem_map)) as Box<CpuFacade>;

        let mut core = Core {
            mem_map: mem_map,
            cpu_facade: cpu,
            is_debugger_attached: false,
        };

        Ok(core)
    }

    pub fn step(&mut self) -> u8 {
        self.cpu_facade.step(&mut self.mem_map)
    }

    pub fn attach_debugger(&mut self) {
        if !self.is_debugger_attached {
            let dummy_facade = self.get_dummy_facade();
            let cpu = mem::replace(&mut self.cpu_facade, dummy_facade).cpu();
            let new_facade = Box::new(TerminalDebugger::new(cpu)) as Box<CpuFacade>;

            self.cpu_facade = new_facade;
            self.is_debugger_attached = true;
        }
    }

    pub fn detach_debugger(&mut self) {
        if self.is_debugger_attached {
            let dummy_facade = self.get_dummy_facade();
            let cpu = mem::replace(&mut self.cpu_facade, dummy_facade).cpu();
            let new_facade = cpu as Box<CpuFacade>;

            self.cpu_facade = new_facade;
            self.is_debugger_attached = false;
        }
    }

    pub fn debugger(&mut self) -> Option<&mut Debugger> {
        self.cpu_facade.debugger()
    }

    fn get_dummy_facade(&mut self) -> Box<CpuFacade> {
        let dummy_cpu: Cpu = Cpu::default();
        let dummy_facade = Box::new(dummy_cpu) as Box<CpuFacade>;

        dummy_facade
    }
}