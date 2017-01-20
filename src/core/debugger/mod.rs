mod command;
pub mod disassembler;
pub mod frontends;

use self::command::Command;
use core::CpuFacade;
use core::cpu::Cpu;

pub trait Debugger: CpuFacade {
    fn break_into(&mut self);

    fn start_listening(&mut self);
    fn stop_listening(&mut self);

    fn is_listening(&self) -> bool;
}
