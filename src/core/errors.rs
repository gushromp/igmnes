use std::fmt;
use core::instructions::Instruction;

#[derive(Debug)]
pub enum EmulationError {
    InstructionDecoding(u16, u8),
    InstructionExecution(u16, Instruction),
    MemoryAccess(String),
    DebuggerBreakpoint(u16),
    DebuggerWatchpoint(u16),
}

impl fmt::Display for EmulationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::EmulationError::*;

        match *self {
            InstructionDecoding(addr, op_code) => {
                write!(f, "${:04X}: Unknown op_code 0x{:02X}", addr, op_code)
            }
            InstructionExecution(addr, ref instr) => {
                write!(f, "Error while executing instruction: 0x{:4X}: {}", addr, instr.token)
            }
            MemoryAccess(ref msg) => {
                write!(f, "Memory access error: {}", msg)
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