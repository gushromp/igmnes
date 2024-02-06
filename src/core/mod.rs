extern crate sdl2;
extern crate time;

mod debugger;
mod mappers;
mod rom;
mod apu;
mod cpu;
mod memory;
mod instructions;
mod errors;
mod debug;
mod ppu;
mod dma;

use std::error::Error;
use std::path::Path;
use std::mem;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::audio::AudioSpecDesired;
use time::Duration;
use core::debug::Tracer;
use core::dma::Dma;
use self::time::PreciseTime;
use self::memory::*;
use self::cpu::Cpu;
use self::ppu::Ppu;
use self::apu::Apu;
use self::rom::Rom;
use self::debugger::Debugger;
use self::debugger::frontends::terminal::TerminalDebugger;
use self::errors::EmulationError;

pub const MASTER_CLOCK_NTSC: f32 = 21.477272_E6_f32; // 21.477272 MHz
pub const CPU_CLOCK_DIVISOR_NTSC: f32 = 12.0;

pub const CPU_CLOCK_RATIO_NTSC: f32 = MASTER_CLOCK_NTSC / CPU_CLOCK_DIVISOR_NTSC;
pub const PPU_CLOCK_DIVISOR_NTSC: f32 = 4.0;
pub const PPU_STEPS_PER_CPU_STEP_NTSC: usize = (CPU_CLOCK_DIVISOR_NTSC / PPU_CLOCK_DIVISOR_NTSC) as usize;

const MASTER_CLOCK_PAL: f32 = 26.601712_E6_f32; // 26.601712 MHz
const CLOCK_DIVISOR_PAL: i32 = 15;


pub trait CpuFacade {
    fn consume(self: Box<Self>) -> (Cpu, CpuMemMap);
    fn debugger(&mut self) -> Option<&mut dyn Debugger>;

    fn cpu(&mut self) -> &mut Cpu;
    fn ppu(&mut self) -> &mut Ppu;
    fn apu(&mut self) -> &mut Apu;
    fn dma(&mut self) -> &mut Dma;

    fn step_cpu(&mut self, tracer: &mut Tracer) -> Result<u8, EmulationError>;
    fn step_ppu(&mut self, cpu_cycles: u64, tracer: &mut Tracer) -> bool;
    fn step_apu(&mut self, cpu_cycles: u64) -> bool;
    fn step_dma(&mut self) -> bool;



    fn nmi(&mut self);
    fn irq(&mut self);

    fn mem_map(&self) -> &CpuMemMap;

}

struct DefaultCpuFacade {
    cpu: Cpu,
    mem_map: CpuMemMap
}

impl DefaultCpuFacade {
    pub fn new(cpu: Cpu, mem_map: CpuMemMap) -> DefaultCpuFacade {
        DefaultCpuFacade {
            cpu,
            mem_map,
        }
    }
}

impl Default for DefaultCpuFacade {
    fn default() -> DefaultCpuFacade {
        let dummy_cpu: Cpu = Cpu::default();
        let dummy_mem_map: CpuMemMap = CpuMemMap::default();

        DefaultCpuFacade {
            cpu: dummy_cpu,
            mem_map: dummy_mem_map,
        }
    }
}

impl CpuFacade for DefaultCpuFacade {
    fn consume(self: Box<Self>) -> (Cpu, CpuMemMap) {
        let this = *self;

        (this.cpu, this.mem_map)
    }

    // This is the real cpu, not a debugger
    fn debugger(&mut self) -> Option<&mut dyn Debugger> {
        None
    }

    fn cpu(&mut self) -> &mut Cpu { &mut self.cpu }

    fn ppu(&mut self) -> &mut Ppu { &mut self.mem_map.ppu }

    fn apu(&mut self) -> &mut Apu { &mut self.mem_map.apu }

    fn dma(&mut self) -> &mut Dma { &mut self.mem_map.dma }

    fn step_cpu(&mut self, tracer: &mut Tracer) -> Result<u8, EmulationError> {
        self.cpu.step(&mut self.mem_map, tracer)
    }

    fn step_ppu(&mut self, cpu_cycle_count: u64, tracer: &mut Tracer) -> bool {
        let ppu_mem_map = &mut self.mem_map.ppu_mem_map;
        self.mem_map.ppu.step(ppu_mem_map, cpu_cycle_count, tracer)
    }

    fn step_apu(&mut self, cpu_cycles: u64) -> bool {
        self.mem_map.apu.step(cpu_cycles)
    }

    fn step_dma(&mut self) -> bool {
        let mut dma = std::mem::take(&mut self.mem_map.dma);
        let mem_map = &mut self.mem_map;
        if let Err(e) = dma.step(mem_map) {
            println!("DMA error: {}", e.to_string());
        }
        let result = dma.is_dma_active();
        self.mem_map.dma = dma;
        result
    }

    fn nmi(&mut self) {
        self.cpu.nmi(&mut self.mem_map).unwrap()
    }

    fn irq(&mut self) {
        self.cpu.irq(&mut self.mem_map).unwrap();
    }

    fn mem_map(&self) -> &CpuMemMap { &self.mem_map }
}

// 2A03 (NTSC) and 2A07 (PAL) emulation
// contains CPU (nearly identical to MOS 6502) part and APU part
pub struct Core {
    // cpu_facade: DefaultCpuFacade, //Box<dyn CpuFacade>,
    cpu_facade: Box<dyn CpuFacade>,
    is_debugger_attached: bool,
    is_running: bool,
}

impl Core {
    pub fn load_rom(file_path: &Path) -> Result<Core, Box<dyn Error>> {
        let rom = Rom::load_rom(file_path)?;
        let mut mem_map = CpuMemMap::new(rom);

        let cpu = Cpu::new(&mut mem_map);
        let cpu_facade = Box::new(DefaultCpuFacade::new(cpu, mem_map)) as Box<dyn CpuFacade>;

        let core = Core {
            cpu_facade,
            is_debugger_attached: false,
            is_running: false,
        };

        Ok(core)
    }

    pub fn start(&mut self, attach_debugger: bool, enable_tracing: bool, entry_point: Option<u16>) {
        self.is_running = true;

        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();
        let audio_subsystem = sdl_context.audio().unwrap();
        //
        let audio_spec_desired = AudioSpecDesired {
            freq: Some(41_000),
            channels: Some(1),
            samples: None,
        };

        let audio_queue = audio_subsystem.open_queue::<f32, _>(None, &audio_spec_desired).unwrap();
        audio_queue.resume();

        let mut events = sdl_context.event_pump().unwrap();

        let window = video_subsystem.window("IGMNes", 256, 240)
            .position_centered()
            .opengl()
            .build()
            .unwrap();

        let mut renderer = window.into_canvas().build().unwrap();

        renderer.set_draw_color(Color::RGB(0, 0, 0));
        renderer.clear();
        renderer.present();

        if attach_debugger {
            let debugger = self.attach_debugger();
            debugger.start_listening();
        }

        let mut tracer = Tracer::default();
        tracer.set_enabled(enable_tracing);

        if let Some(entry_point) = entry_point {
            self.cpu_facade.cpu().reg_pc = entry_point;
        }

        let start_time = PreciseTime::now();
        let mut previous_cycle_count = self.cpu_facade.cpu().cycle_count;


        'running: loop {
            tracer.start_new_trace();

            if self.is_running {
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

                let current_cycle_count = self.cpu_facade.cpu().cycle_count;

                let nmi = self.cpu_facade.step_ppu(current_cycle_count, &mut tracer);
                if nmi {
                    self.cpu_facade.ppu().clear_nmi();
                    self.cpu_facade.nmi();
                }

                let irq = self.cpu_facade.step_apu(current_cycle_count);
                if irq && !nmi {
                    self.cpu_facade.irq();
                }

                let dma = self.cpu_facade.dma().is_dma_active();
                if dma {
                    self.cpu_facade.step_dma();
                    self.cpu_facade.cpu().dma();
                }

                if let Some(debugger) = self.cpu_facade.debugger() {
                    if debugger.is_listening() {
                        debugger.break_into();
                    }
                }

                let result = self.cpu_facade.step_cpu(&mut tracer);

                match result {
                    Ok(_) => {},
                    Err(error) => match error {
                        EmulationError::DebuggerBreakpoint(_addr) |
                        EmulationError::DebuggerWatchpoint(_addr) => {
                            if self.is_debugger_attached {
                                self.debugger().unwrap().start_listening();
                            }
                        }
                        e @ _ => println!("{}", e),
                    }
                }
            }

            let apu = self.cpu_facade.apu();
            if let Some(out_samples) = apu.get_out_samples(44100) {
                audio_queue.queue_audio(out_samples.as_ref()).unwrap();
            }
            // let remaining_samples = audio_queue.spec().freq - audio_queue.size() as i32;
            // if remaining_samples > 0 {
            //     if let Some(out_samples) = apu.get_out_samples(remaining_samples as usize + 1) {
            //         audio_queue.queue_audio(out_samples.as_ref()).unwrap();
            //     }
            // }
        }

        if tracer.has_traces() {
            tracer.write_to_file(Path::new("./trace.log"));
        }

        let cur_time = PreciseTime::now();
        let seconds = start_time.to(cur_time).num_milliseconds() as f64 / 1000.0;
        println!("Cycles: {}", self.cpu_facade.cpu().cycle_count);
        println!("Seconds: {}", seconds);
        if seconds > 0.0 {
            println!("Cycles per second: {}", (self.cpu_facade.cpu().cycle_count as f64 / seconds).floor());
        }
    }

    pub fn unpause(&mut self) {
        self.is_running = true;
    }

    pub fn pause(&mut self) {
        self.is_running = false;
    }

    pub fn attach_debugger(&mut self) -> &mut dyn Debugger {
        if !self.is_debugger_attached {
            let dummy_facade = self.get_dummy_facade();
            let (cpu, mem_map) = mem::replace(&mut self.cpu_facade, dummy_facade).consume();
            let new_facade = Box::new(TerminalDebugger::new(cpu, mem_map)) as Box<dyn CpuFacade>;

            self.cpu_facade = new_facade;
            self.is_debugger_attached = true;
        }

        self.debugger().unwrap()
    }

    pub fn detach_debugger(&mut self) {
        if self.is_debugger_attached {
            let dummy_facade = self.get_dummy_facade();
            let (cpu, mem_map) = mem::replace(&mut self.cpu_facade, dummy_facade).consume();
            let new_facade = Box::new(DefaultCpuFacade::new(cpu, mem_map)) as Box<dyn CpuFacade>;

            self.cpu_facade = new_facade;
            self.is_debugger_attached = false;
        }
    }

    pub fn debugger(&mut self) -> Option<&mut dyn Debugger> {
        self.cpu_facade.debugger()
    }

    fn get_dummy_facade(&mut self) -> Box<dyn CpuFacade> {
        let dummy_device = DefaultCpuFacade::default();
        let dummy_facade = Box::new(dummy_device) as Box<dyn CpuFacade>;

        dummy_facade
    }
}