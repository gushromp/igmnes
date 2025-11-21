#[macro_use]
extern crate nom;

mod apu;
mod controller;
mod cpu;
pub mod debug;
pub mod debugger;
mod dma;
mod errors;
mod instructions;
mod mappers;
mod memory;
mod ppu;
mod rom;

use self::apu::Apu;
use self::cpu::Cpu;
use self::errors::EmulationError;
use self::memory::*;
use self::ppu::Ppu;
use self::rom::Rom;

use crate::controller::Controller;
use crate::debug::Tracer;
use crate::debugger::frontends::terminal::TerminalDebugger;
use crate::debugger::{Debugger, DebuggerFrontend};
use crate::dma::Dma;
use crate::rom::RomError;
use enum_dispatch::enum_dispatch;

use std::path::Path;
use thiserror::Error;

pub use crate::controller::{ControllerButton, ControllerButtonState, ControllerIndex};
pub use crate::ppu::PpuFrame;

pub const MASTER_CLOCK_NTSC: f32 = 21.477272_E6_f32;
// 21.477272 MHz
pub const CPU_CLOCK_DIVISOR_NTSC: f32 = 12.0;

pub const CPU_CLOCK_RATIO_NTSC: f32 = MASTER_CLOCK_NTSC / CPU_CLOCK_DIVISOR_NTSC;
pub const PPU_CLOCK_DIVISOR_NTSC: f32 = 4.0;
pub const PPU_STEPS_PER_CPU_STEP_NTSC: usize =
    (CPU_CLOCK_DIVISOR_NTSC / PPU_CLOCK_DIVISOR_NTSC) as usize;

const MASTER_CLOCK_PAL: f32 = 26.601712_E6_f32;
// 26.601712 MHz
const CLOCK_DIVISOR_PAL: i32 = 15;

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
        DefaultBus { cpu, mem_map }
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

    fn cpu(&mut self) -> &mut Cpu {
        &mut self.cpu
    }

    fn ppu(&mut self) -> &mut Ppu {
        &mut self.mem_map.ppu
    }

    fn apu(&mut self) -> &mut Apu {
        &mut self.mem_map.apu
    }

    fn dma(&mut self) -> &mut Dma {
        &mut self.mem_map.dma
    }

    fn controllers(&mut self) -> &mut [Controller; 2] {
        &mut self.mem_map.controllers
    }

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
        dma.step(mem_map);
        let result = dma.is_dma_active();
        self.mem_map.dma = dma;
        result
    }

    #[inline]
    fn nmi(&mut self) {
        self.cpu.nmi(&mut self.mem_map)
    }

    #[inline]
    fn irq(&mut self) {
        self.cpu.irq(&mut self.mem_map);
    }
}

impl BusDebugger for DefaultBus {
    fn debugger(&mut self) -> Option<&mut DebuggerFrontend> {
        None
    }
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

    pub is_debugger_attached: bool,
    pub is_running: bool,
}

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Error loading ROM: {0:?}")]
    RomError(#[from] RomError),
}

impl Core {
    pub fn load_rom(file_path: &Path) -> Result<Core, CoreError> {
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

    #[inline]
    pub fn cpu_cycles(&mut self) -> u64 {
        self.bus.cpu().cycle_count
    }

    pub fn unpause(&mut self) {
        self.is_running = true;
    }

    pub fn pause(&mut self) {
        self.is_running = false;
    }

    pub fn set_entry_point(&mut self, entry_point_addr: u16) {
        self.bus.cpu().reg_pc = entry_point_addr;
    }

    pub fn is_ppu_frame_ready(&mut self) -> bool {
        self.bus.ppu().is_frame_ready()
    }

    pub fn ppu_frame(&mut self) -> PpuFrame<'_> {
        self.bus.ppu().get_frame()
    }

    pub fn is_apu_output_ready(&mut self) -> bool {
        self.bus.apu().is_output_ready()
    }

    pub fn apu_output_samples(&mut self) -> Vec<f32> {
        self.bus.apu().get_out_samples()
    }

    pub fn set_controller_button_state(
        &mut self,
        controller_index: ControllerIndex,
        controller_button_state: ControllerButtonState,
    ) {
        let controller_index: usize = controller_index as usize;
        self.bus.controllers()[controller_index].set_button_state(controller_button_state)
    }

    pub fn attach_debugger(&mut self) -> &mut DebuggerFrontend {
        if !self.is_debugger_attached {
            let dummy_facade = self.get_dummy_facade();
            let (cpu, mem_map) = std::mem::replace(&mut self.bus, dummy_facade).consume();
            let new_bus = DebuggerFrontend::from(TerminalDebugger::new(cpu, mem_map));

            self.bus = new_bus.into();
            self.is_debugger_attached = true;
        }

        self.bus.debugger().unwrap()
    }

    pub fn detach_debugger(&mut self) {
        if self.is_debugger_attached {
            let dummy_bus = self.get_dummy_facade();
            let (cpu, mem_map) = std::mem::replace(&mut self.bus, dummy_bus).consume();
            let new_bus = DefaultBus::new(cpu, mem_map);

            self.bus = new_bus.into();
            self.is_debugger_attached = false;
        }
    }

    pub fn step(&mut self, tracer: &mut Tracer) {
        tracer.start_new_trace();

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
                EmulationError::DebuggerBreakpoint(_addr)
                | EmulationError::DebuggerWatchpoint(_addr) => {
                    if self.is_debugger_attached {
                        self.bus.debugger().unwrap().start_listening();
                    }
                }
                e @ _ => println!("{}", e),
            },
        }
    }

    fn get_dummy_facade(&mut self) -> Bus {
        let dummy_device = DefaultBus::default();
        dummy_device.into()
    }
}
