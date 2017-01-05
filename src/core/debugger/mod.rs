mod command;
mod disassembler;

use core::cpu::Cpu;

pub struct Debugger<'a> {
    cpu: &'a mut Cpu,
}

impl<'a> Debugger<'a> {
    pub fn attach(cpu: &'a mut Cpu) -> Debugger {
        Debugger {
            cpu: cpu
        }
    }
}
