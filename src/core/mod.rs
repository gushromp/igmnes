extern crate sdl2;

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

use crate::core::debugger::DebuggerFrontend;
use self::apu::Apu;
use self::cpu::Cpu;
use self::debugger::frontends::terminal::TerminalDebugger;
use self::debugger::Debugger;
use self::errors::EmulationError;
use self::memory::*;
use self::ppu::Ppu;
use self::rom::Rom;
use crate::core::controller::Controller;
use crate::core::debug::Tracer;
use crate::core::dma::Dma;
use sdl2::audio::AudioSpecDesired;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{TextureCreator, WindowCanvas};
use sdl2::video::FullscreenType;
use std::error::Error;
use std::path::Path;
use std::time::{Duration, Instant};
use std::{mem, ptr};
use enum_dispatch::enum_dispatch;

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
const PIXELS_PER_SCANLINE: usize = 256_usize;
const BYTES_PER_SCANLINE: usize = PIXELS_PER_SCANLINE * 3;
const SCANLINES: usize = 240;
const SCANLINES_OFFSET: usize = 8;

const NANOS_PER_FRAME: u128 = 16_666_667;

#[enum_dispatch]
pub trait BusOps {
    fn consume(self) -> (Cpu, CpuMemMap);

    fn cpu(&mut self) -> &mut Cpu;
    fn ppu(&mut self) -> &mut Ppu;
    fn apu(&mut self) -> &mut Apu;
    fn dma(&mut self) -> &mut Dma;

    fn controllers(&mut self) -> &mut [Controller; 2];

    fn step_cpu(&mut self, tracer: &mut Tracer) -> Result<u8, EmulationError>;
    fn step_ppu(&mut self, cpu_cycles: u64, tracer: &mut Tracer) -> bool;
    fn step_apu(&mut self, cpu_cycles: u64) -> bool;
    fn step_dma(&mut self) -> bool;

    fn nmi(&mut self);
    fn irq(&mut self);

    fn mem_map(&self) -> &CpuMemMap;
}

#[enum_dispatch]
pub trait BusDebugger {
    fn debugger(&mut self) -> Option<&mut DebuggerFrontend>;
}


struct DefaultBus {
    cpu: Cpu,
    mem_map: CpuMemMap,
}

impl DefaultBus {
    pub fn new(cpu: Cpu, mem_map: CpuMemMap) -> DefaultBus {
        DefaultBus {
            cpu,
            mem_map,
        }
    }
}

impl Default for DefaultBus {
    fn default() -> DefaultBus {
        let dummy_cpu: Cpu = Cpu::default();
        let dummy_mem_map: CpuMemMap = CpuMemMap::default();

        DefaultBus {
            cpu: dummy_cpu,
            mem_map: dummy_mem_map,
        }
    }
}

impl BusOps for DefaultBus {
    fn consume(self) -> (Cpu, CpuMemMap) {
        (self.cpu, self.mem_map)
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

    fn nmi(&mut self) {
        self.cpu.nmi(&mut self.mem_map).unwrap()
    }

    fn irq(&mut self) {
        self.cpu.irq(&mut self.mem_map).unwrap();
    }

    fn mem_map(&self) -> &CpuMemMap { &self.mem_map }
}

impl BusDebugger for DefaultBus {
    fn debugger(&mut self) -> Option<&mut DebuggerFrontend> { None }
}

#[enum_dispatch(BusOps, BusDebugger)]
enum Bus {
    DefaultBus,
    DebuggerFrontend,
}

// 2A03 (NTSC) and 2A07 (PAL) emulation
// contains CPU (nearly identical to MOS 6502) part and APU part
pub struct Core {
    bus: Bus,
    is_debugger_attached: bool,
    is_running: bool,
}

impl Core {
    pub fn load_rom(file_path: &Path) -> Result<Core, Box<dyn Error>> {
        let rom = Rom::load_rom(file_path)?;
        let mut mem_map = CpuMemMap::new(rom);

        let cpu = Cpu::new(&mut mem_map);
        let bus = DefaultBus::new(cpu, mem_map);

        let core = Core {
            bus: Bus::from(bus),
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
            .resizable()
            .position_centered()
            .build()
            .unwrap();


        let mut renderer = window.into_canvas().build().unwrap();
        renderer.set_logical_size(256, 232).unwrap();

        let texture_creator = renderer.texture_creator();

        if attach_debugger {
            let debugger = self.attach_debugger();
            debugger.start_listening();
        }

        let mut tracer = Tracer::default();
        tracer.set_enabled(enable_tracing);

        if let Some(entry_point) = entry_point {
            self.bus.cpu().reg_pc = entry_point;
        }

        let start_time = Instant::now();

        'running: loop {
            if self.is_running {
                let frame_start = Instant::now();

                let mut did_change_fullscreen_state = false;
                // Events
                for event in events.poll_iter() {

                    match event {
                        Event::Quit { .. } |
                        Event::KeyDown { keycode: Some(Keycode::Escape), .. } => break 'running,
                        Event::KeyDown { keycode: Some(Keycode::F9), .. } => {
                            if did_change_fullscreen_state { break }
                            let new_state = if renderer.window().fullscreen_state() == FullscreenType::Desktop {
                                FullscreenType::Off
                            } else {
                                FullscreenType::Desktop
                            };
                            renderer.window_mut().set_fullscreen(new_state).unwrap();
                            did_change_fullscreen_state = true;
                        }
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
                while !self.bus.ppu().is_frame_ready() {
                    self.step(&mut tracer, &keys)
                }

                // Render frame
                self.render_frame(&mut renderer, &texture_creator);

                // Audio
                while !self.bus.apu().is_output_ready() {
                    // Keep running (if necessary) until we have audio enough samples for this frame
                    self.step(&mut tracer, &keys);
                }
                let samples = self.bus.apu().get_out_samples();
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
        println!("Cycles: {}", self.bus.cpu().cycle_count);
        println!("Seconds: {}", seconds);
        if seconds > 0.0 {
            println!("Cycles per second: {}", (self.bus.cpu().cycle_count as f64 / seconds).floor());
        }
    }

    pub fn unpause(&mut self) {
        self.is_running = true;
    }

    pub fn pause(&mut self) {
        self.is_running = false;
    }

    pub fn attach_debugger(&mut self) -> &mut DebuggerFrontend {
        if !self.is_debugger_attached {
            let dummy_facade = self.get_dummy_facade();
            let (cpu, mem_map) = mem::replace(&mut self.bus, dummy_facade).consume();
            let new_bus = DebuggerFrontend::from(TerminalDebugger::new(cpu, mem_map));

            self.bus = new_bus.into();
            self.is_debugger_attached = true;
        }

        self.bus.debugger().unwrap()
    }

    pub fn detach_debugger(&mut self) {
        if self.is_debugger_attached {
            let dummy_bus = self.get_dummy_facade();
            let (cpu, mem_map) = mem::replace(&mut self.bus, dummy_bus).consume();
            let new_bus = DefaultBus::new(cpu, mem_map);

            self.bus = new_bus.into();
            self.is_debugger_attached = false;
        }
    }

    fn get_dummy_facade(&mut self) -> Bus {
        let dummy_device = DefaultBus::default();
        dummy_device.into()
    }

    fn set_controllers_state<'a, I>(&mut self, state: I) where I: Iterator<Item=&'a Keycode> {
        use crate::core::controller::ControllerButton;
        let mut controller_1_state: Vec<ControllerButton> = vec![];

        for key_state in state {
            let button_state = match *key_state {
                Keycode::X => Some(ControllerButton::A),
                Keycode::Z => Some(ControllerButton::B),
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

        self.bus.controllers()[0].set_button_state(&controller_1_state);
    }

    fn step(&mut self, tracer: &mut Tracer, keys: &Vec<Keycode>) {
        tracer.start_new_trace();

        self.set_controllers_state(keys.iter());
        let current_cycle_count = self.bus.cpu().cycle_count;

        let nmi = self.bus.step_ppu(current_cycle_count, tracer);
        if nmi {
            self.bus.ppu().clear_nmi();
            self.bus.nmi();
        }

        let irq = self.bus.step_apu(current_cycle_count);
        if irq && !nmi {
            self.bus.irq();
        }

        let dma = self.bus.dma().is_dma_active();
        if dma {
            self.bus.step_dma();
            self.bus.cpu().dma();
        }

        if let Some(debugger) = self.bus.debugger() {
            if debugger.is_listening() {
                debugger.break_into();
            }
        }

        let result = self.bus.step_cpu(tracer);

        match result {
            Ok(_) => {
                if self.bus.ppu().should_suppress_nmi() {
                    self.bus.cpu().suppress_interrupt();
                } else if self.bus.ppu().nmi_pending {
                    // Needs PPU to track it's own cycles in order to be more accurate
                    self.bus.ppu().clear_nmi();
                    self.bus.nmi();
                }
            }
            Err(error) => match error {
                EmulationError::DebuggerBreakpoint(_addr) |
                EmulationError::DebuggerWatchpoint(_addr) => {
                    if self.is_debugger_attached {
                        self.bus.debugger().unwrap().start_listening();
                    }
                }
                e @ _ => println!("{}", e),
            }
        }
    }

    fn render_frame<T>(&mut self, renderer: &mut WindowCanvas, texture_creator: &TextureCreator<T>) {
        let frame = self.bus.ppu().get_frame();
        unsafe {


            let pointer = ptr::addr_of!(**frame);
            let pointer_arr = pointer as *mut [u8; BYTES_PER_SCANLINE * SCANLINES];
            let mut data = *pointer_arr;

            let offset = BYTES_PER_SCANLINE * SCANLINES_OFFSET;
            let data_slice = &mut data[offset..];
            let surface = sdl2::surface::Surface::from_data(data_slice, 256, 240 - (SCANLINES_OFFSET as u32 * 2), BYTES_PER_SCANLINE as u32, PixelFormatEnum::RGB24).unwrap();
            let tex = surface.as_texture(texture_creator).unwrap();
            renderer.copy(&tex, None, None).unwrap();
            renderer.present();
        }
    }
}

