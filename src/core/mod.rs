extern crate sdl2;

mod debugger;
mod mappers;
mod rom;
mod apu;
mod cpu;
mod memory;
mod instructions;
mod errors;

use std::error::Error;
use std::path::Path;
use std::mem;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use self::memory::*;
use self::apu::Apu;
use self::cpu::Cpu;
use self::rom::Rom;
use self::debugger::Debugger;
use self::debugger::frontends::terminal::TerminalDebugger;
use self::errors::CpuError;

pub trait CpuFacade {
    fn consume(self: Box<Self>) -> (Cpu, MemMap);
    fn debugger(&mut self) -> Option<&mut Debugger>;

    fn step(&mut self) -> Result<u8, CpuError>;
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

    fn step(&mut self) -> Result<u8, CpuError> {
        self.cpu.step(&mut self.mem_map)
    }
}

// 2A03 (NTSC) and 2A07 (PAL) emulation
// contains CPU (nearly identical to MOS 6502) part and APU part
pub struct Core {
    cpu_facade: Box<CpuFacade>,

    is_debugger_attached: bool,
    is_running: bool,
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
            is_running: false,
        };

        Ok(core)
    }

    pub fn start(&mut self) {
        self.is_running = true;

        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();
        let mut events = sdl_context.event_pump().unwrap();

        let window = video_subsystem.window("rust-sdl2 demo: Video", 256, 240)
            .position_centered()
            .opengl()
            .build()
            .unwrap();

        let mut renderer = window.renderer().build().unwrap();

        renderer.set_draw_color(Color::RGB(0, 0, 0));
        renderer.clear();
        renderer.present();

        let mut cycle_count: u64 = 0;

        'running: loop {
            for event in events.poll_iter() {
                match event {
                    Event::Quit { .. } => break 'running,
                    Event::KeyDown { keycode: Some(Keycode::F12), .. } => {
                        let debugger = self.attach_debugger();

                        if !debugger.is_listening() {
                            debugger.start_listening();
                        }
                    }
                    _ => {},
                }
            }

            if self.is_running {
                let result = self.step();

                match result {
                    Ok(cycles) => cycle_count += cycles as u64,
                    Err(error) => match error {
                        CpuError::DebuggerBreakpoint(addr) => {
                            if self.is_debugger_attached {
                                self.debugger().unwrap().start_listening();
                            }
                        }
                        e @ _ => println!("{}", e),
                    }
                }
            }
        }
    }

    pub fn unpause(&mut self) {
        self.is_running = true;
    }

    pub fn pause(&mut self) {
        self.is_running = false;
    }

    pub fn step(&mut self) -> Result<u8, CpuError> {
        self.cpu_facade.step()
    }

    pub fn attach_debugger(&mut self) -> &mut Debugger {
        if !self.is_debugger_attached {
            let dummy_facade = self.get_dummy_facade();
            let (cpu, mem_map) = mem::replace(&mut self.cpu_facade, dummy_facade).consume();
            let new_facade = Box::new(TerminalDebugger::new(cpu, mem_map)) as Box<CpuFacade>;

            self.cpu_facade = new_facade;
            self.is_debugger_attached = true;
        }

        self.debugger().unwrap()
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