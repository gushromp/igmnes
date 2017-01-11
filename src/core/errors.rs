use std::fmt;
use core::instructions::Instruction;
use core::debugger::disassembler;

pub enum CpuError {
    InstructionDecoding(u8),
    InstructionExecution(u16, Instruction),
    DebuggerBreakpoint(u16)
}

impl fmt::Display for CpuError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::CpuError::*;

        match *self {
            InstructionDecoding(op_code) => {
                write!(f, "Unknown op_code 0x{:02X}", op_code)
            }
            InstructionExecution(addr, ref instr) => {
                write!(f, "Error while executing instruction: {}", disassembler::disassemble(addr, instr))
            }
            DebuggerBreakpoint(addr) => {
                write!(f, "Hit breakpoint at addr: 0x{:04X}", addr)
            }
        }
    }
}