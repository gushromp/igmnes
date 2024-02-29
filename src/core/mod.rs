extern crate sdl2;
extern crate shuteye;

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
mod controller;

use std::error::Error;
use std::path::Path;
use std::{mem, ptr, slice};
use std::cmp::max;
use std::collections::HashMap;
use std::ops::Deref;
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Scancode};
use sdl2::pixels::PixelFormatEnum;
use sdl2::audio::AudioSpecDesired;
use core::controller::Controller;
use core::debug::Tracer;
use core::dma::Dma;
use std::time::{Duration, Instant};
use self::memory::*;
use self::cpu::Cpu;
use self::ppu::Ppu;
use self::apu::Apu;
use self::rom::Rom;
use self::debugger::Debugger;
use self::debugger::frontends::terminal::TerminalDebugger;
use self::errors::EmulationError;

pub const MASTER_CLOCK_NTSC: f32 = 21.477272_E6_f32;
// 21.477272 MHz
pub const CPU_CLOCK_DIVISOR_NTSC: f32 = 12.0;

pub const CPU_CLOCK_RATIO_NTSC: f32 = MASTER_CLOCK_NTSC / CPU_CLOCK_DIVISOR_NTSC;
pub const PPU_CLOCK_DIVISOR_NTSC: f32 = 4.0;
pub const PPU_STEPS_PER_CPU_STEP_NTSC: usize = (CPU_CLOCK_DIVISOR_NTSC / PPU_CLOCK_DIVISOR_NTSC) as usize;

const MASTER_CLOCK_PAL: f32 = 26.601712_E6_f32;
// 26.601712 MHz
const CLOCK_DIVISOR_PAL: i32 = 15;

const WINDOW_SCALING: u32 = 3;

const NANOS_PER_FRAME: u128 = 16_666_667;
// const NANOS_PER_FRAME: u32 = 16_466_666;
// const NANOS_PER_FRAME: u32 = 16_465_700;
// const NANOS_PER_FRAME: u32 = 16_333_334;

pub trait CpuFacade {
    fn consume(self: Box<Self>) -> (Cpu, CpuMemMap);
    fn debugger(&mut self) -> Option<&mut dyn Debugger>;

    fn cpu(&mut self) -> &mut Cpu;
    fn ppu(&mut self) -> &mut Ppu;
    fn apu(&mut self) -> &mut Apu;
    fn dma(&mut self) -> &mut Dma;

    fn controllers(&mut self) -> &mut [Controller; 2];

    fn step_cpu(&mut self, tracer: &mut Tracer) -> Result<u8, EmulationError>;
    fn step_ppu(&mut self, cpu_cycles: u64, tracer: &mut Tracer) -> bool;
    fn step_apu(&mut self, cpu_cycles: u64) -> bool;
    fn step_dma(&mut self) -> bool;

    fn nmi(&mut self, is_immediate: bool);
    fn irq(&mut self);

    fn mem_map(&self) -> &CpuMemMap;
}

struct DefaultCpuFacade {
    cpu: Cpu,
    mem_map: CpuMemMap,
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

    fn controllers(&mut self) -> &mut [Controller; 2] { &mut self.mem_map.controllers }

    fn step_cpu(&mut self, tracer: &mut Tracer) -> Result<u8, EmulationError> {
        self.cpu.step(&mut self.mem_map, tracer)
    }

    fn step_ppu(&mut self, cpu_cycle_count: u64, tracer: &mut Tracer) -> bool {
        self.mem_map.ppu.step(cpu_cycle_count, tracer)
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

    fn nmi(&mut self, is_immediate: bool) {
        self.cpu.nmi(&mut self.mem_map, is_immediate).unwrap()
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
            freq: Some(44_100),
            channels: Some(1),
            samples: Some(1),
        };


        let audio_queue = audio_subsystem.open_queue::<f32, _>(None, &audio_spec_desired).unwrap();
        audio_queue.resume();

        let mut events = sdl_context.event_pump().unwrap();

        let window = video_subsystem.window("IGMNes", 256 * WINDOW_SCALING, 240 * WINDOW_SCALING)
            .position_centered()
            .build()
            .unwrap();


        let mut renderer = window.into_canvas().build().unwrap();

        let texture_creator = renderer.texture_creator();

        if attach_debugger {
            let debugger = self.attach_debugger();
            debugger.start_listening();
        }

        let mut tracer = Tracer::default();
        tracer.set_enabled(enable_tracing);

        if let Some(entry_point) = entry_point {
            self.cpu_facade.cpu().reg_pc = entry_point;
        }

        let start_time = Instant::now();

        'running: loop {
            if self.is_running {
                let frame_start = Instant::now();

                // Events
                for event in events.poll_iter() {
                    match event {
                        Event::Quit { .. } |
                        Event::KeyDown { keycode: Some(Keycode::Escape), .. } => break 'running,
                        Event::KeyDown { keycode: Some(Keycode::F12), .. } => {
                            let debugger = self.attach_debugger();

                            if !debugger.is_listening() {
                                debugger.start_listening();
                            }
                        }
                        _ => {}
                    }
                }

                // Input
                let keyboard_state = events.keyboard_state();
                let pressed_scancodes = keyboard_state.pressed_scancodes();
                let keys: Vec<Keycode> = pressed_scancodes
                    .filter_map(Keycode::from_scancode).collect();

                // Run emulation until PPU frame ready
                while !self.cpu_facade.ppu().is_frame_ready() {
                    self.step(&mut tracer, &keys)
                }

                // Render frame
                let frame = self.cpu_facade.ppu().get_frame();
                unsafe {
                    let pointer = ptr::addr_of!(**frame);
                    let pointer_arr = pointer as *mut [u8; 256 * 240 * 3];
                    let mut data = *pointer_arr;

                    let surface = sdl2::surface::Surface::from_data(&mut data, 256, 240, 256 * 3, PixelFormatEnum::RGB24).unwrap();
                    let tex = surface.as_texture(&texture_creator).unwrap();
                    renderer.copy(&tex, None, None).unwrap();
                    renderer.present();
                }


                // Audio
                while !self.cpu_facade.apu().is_output_ready() {
                    // Keep running (if necessary) until we have audio enough samples for this frame
                    self.step(&mut tracer, &keys);
                }
                let samples = self.cpu_facade.apu().get_out_samples();
                audio_queue.queue_audio(&samples).unwrap();


                // Sleep
                let frame_duration = Instant::now().duration_since(frame_start);
                let frame_duration_nanos = frame_duration.as_nanos();
                if frame_duration_nanos < NANOS_PER_FRAME {
                    // Sleep for a certain amount to alleviate CPU usage, then use busy loop for rest for accurate timing
                    let frame_duration_millis = frame_duration.as_millis();
                    let ms_to_sleep = 16 - frame_duration_millis as u64 - 1;

                    let duration_to_sleep = Duration::from_millis(ms_to_sleep);
                    std::thread::sleep(duration_to_sleep);

                    while Instant::now().duration_since(frame_start).as_nanos() < NANOS_PER_FRAME { }
                }
            }

        }

        if tracer.has_traces() {
            tracer.write_to_file(Path::new("./trace.log"));
        }

        let cur_time = Instant::now();
        let seconds = cur_time.duration_since(start_time).as_millis() as f64 / 1000.0;
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

    fn set_controllers_state<'a, I>(&mut self, state: I) where I: Iterator<Item=&'a Keycode> {
        use core::controller::ControllerButton;
        let mut controller_1_state: Vec<ControllerButton> = vec![];

        for key_state in state {
            let button_state = match key_state {
                Keycode::Z => Some(ControllerButton::A),
                Keycode::X => Some(ControllerButton::B),
                Keycode::RShift => Some(ControllerButton::SELECT),
                Keycode::Return => Some(ControllerButton::START),
                Keycode::Up => Some(ControllerButton::UP),
                Keycode::Down => Some(ControllerButton::DOWN),
                Keycode::Left => Some(ControllerButton::LEFT),
                Keycode::Right => Some(ControllerButton::RIGHT),
                _ => None
            };

            if let Some(button_state) = button_state {
                controller_1_state.push(button_state);
            }
        }

        self.cpu_facade.controllers()[0].set_button_state(&controller_1_state);
    }

    fn step(&mut self, tracer: &mut Tracer, keys: &Vec<Keycode>) {
        tracer.start_new_trace();

        self.set_controllers_state(keys.iter());
        let current_cycle_count = self.cpu_facade.cpu().cycle_count;

        let nmi = self.cpu_facade.step_ppu(current_cycle_count, tracer);
        if nmi {
            self.cpu_facade.ppu().clear_nmi();
            self.cpu_facade.nmi(false);
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

        let result = self.cpu_facade.step_cpu(tracer);

        match result {
            Ok(_) => {
                if self.cpu_facade.ppu().should_suppress_nmi() {
                    self.cpu_facade.cpu().suppress_interrupt();
                } else if self.cpu_facade.ppu().nmi_pending {
                    // Needs PPU to track it's own cycles in order to be more accurate
                    self.cpu_facade.ppu().clear_nmi();
                    self.cpu_facade.nmi(true);
                }
            }
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
}