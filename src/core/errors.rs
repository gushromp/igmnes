use std::fmt;
use core::instructions::Instruction;
use core::debugger::disassembler;

pub enum CpuError {
    InstructionDecoding(u16, u8),
    InstructionExecution(u16, Instruction),
    DebuggerBreakpoint(u16),
    DebuggerWatchpoint(u16),
}

impl fmt::Display for CpuError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::CpuError::*;

        match *self {
            InstructionDecoding(addr, op_code) => {
                write!(f, "${:04X}: Unknown op_code 0x{:02X}", addr, op_code)
            }
            InstructionExecution(addr, ref instr) => {
                write!(f, "Error while executing instruction: 0x{:4X}: {}", addr, instr.token)
            }
            DebuggerBreakpoint(addr) => {
                write!(f, "Hit breakpoint at addr: 0x{:04X}", addr)
            }
            DebuggerWatchpoint(addr) => {
                write!(f, "Hit watchpoint at addr: 0x{:04X}", addr)
            }
        }
    }
}