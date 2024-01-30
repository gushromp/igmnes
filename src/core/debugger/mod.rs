mod command;
pub mod disassembler;
pub mod frontends;


use core::CpuFacade;


pub trait Debugger: CpuFacade {
    fn break_into(&mut self);

    fn start_listening(&mut self);
    fn stop_listening(&mut self);

    fn is_listening(&self) -> bool;
}
