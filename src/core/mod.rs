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
    fn consume(self: Box<Self>) -> (Cpu, MemMap);
    fn debugger(&mut self) -> Option<&mut Debugger>;

    fn step(&mut self) -> Result<u8, String>;
}

struct DefaultCpuFacade {
    cpu: Cpu,
    mem_map: MemMap
}

impl DefaultCpuFacade {
    pub fn new(cpu: Cpu, mem_map: MemMap) -> DefaultCpuFacade {
        DefaultCpuFacade {
            cpu: cpu,
            mem_map: mem_map,
        }
    }
}

impl Default for DefaultCpuFacade {
    fn default() -> DefaultCpuFacade {
        let dummy_cpu: Cpu = Cpu::default();
        let dummy_mem_map: MemMap = MemMap::default();

        DefaultCpuFacade {
            cpu: dummy_cpu,
            mem_map: dummy_mem_map,
        }
    }
}

impl CpuFacade for DefaultCpuFacade {
    fn consume(self: Box<Self>) -> (Cpu, MemMap) {
        let this = *self;

        (this.cpu, this.mem_map)
    }

    // This is the real cpu, not a debugger
    fn debugger(&mut self) -> Option<&mut Debugger> {
        None
    }

    fn step(&mut self) -> Result<u8, String> {
        self.cpu.step(&mut self.mem_map)
    }
}

// 2A03 (NTSC) and 2A07 (PAL) emulation
// contains CPU (nearly identical to MOS 6502) part and APU part
pub struct Core {
    cpu_facade: Box<CpuFacade>,
    is_debugger_attached: bool,
}

impl Core {
    pub fn load_rom(file_path: &Path) -> Result<Core, Box<Error>> {
        let rom = Rom::load_rom(file_path)?;
        let mem_map = MemMap::new(rom);

        let cpu = Cpu::new(&mem_map);
        let cpu_facade = Box::new(DefaultCpuFacade::new(cpu, mem_map)) as Box<CpuFacade>;

        let mut core = Core {
            cpu_facade: cpu_facade,
            is_debugger_attached: false,
        };

        Ok(core)
    }

    pub fn step(&mut self) -> Result<u8, String> {
        self.cpu_facade.step()
    }

    pub fn attach_debugger(&mut self) {
        if !self.is_debugger_attached {
            let dummy_facade = self.get_dummy_facade();
            let (cpu, mem_map) = mem::replace(&mut self.cpu_facade, dummy_facade).consume();
            let new_facade = Box::new(TerminalDebugger::new(cpu, mem_map)) as Box<CpuFacade>;

            self.cpu_facade = new_facade;
            self.is_debugger_attached = true;
        }
    }

    pub fn detach_debugger(&mut self) {
        if self.is_debugger_attached {
            let dummy_facade = self.get_dummy_facade();
            let (cpu, mem_map) = mem::replace(&mut self.cpu_facade, dummy_facade).consume();
            let new_facade = Box::new(DefaultCpuFacade::new(cpu, mem_map)) as Box<CpuFacade>;

            self.cpu_facade = new_facade;
            self.is_debugger_attached = false;
        }
    }

    pub fn debugger(&mut self) -> Option<&mut Debugger> {
        self.cpu_facade.debugger()
    }

    fn get_dummy_facade(&mut self) -> Box<CpuFacade> {
        let dummy_device = DefaultCpuFacade::default();
        let dummy_facade = Box::new(dummy_device) as Box<CpuFacade>;

        dummy_facade
    }
}