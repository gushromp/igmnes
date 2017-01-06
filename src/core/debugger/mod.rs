mod command;
mod disassembler;
pub mod frontends;

use self::command::Command;
use core::CpuFacade;
use core::cpu::Cpu;

pub trait Debugger: CpuFacade {
    fn start_listening(&mut self);
    fn stop_listening(&mut self);
}
