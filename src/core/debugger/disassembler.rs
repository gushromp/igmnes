use std::ops::Range;
use core::instructions::Instruction;
use core::memory::MemMapped;

pub fn disassemble_range(addr: u16, range: &Range<i16>, mem_map: &MemMapped) -> Vec<String> {
    let mut result = Vec::new();
    let mut current_addr = addr;

    let range = range.clone();
    for i in range {
        let index = current_addr as u16;
        let instruction = Instruction::decode(mem_map, index);

        match instruction {
            Ok(ref ins) => {
                result.push(disassemble(current_addr, ins));
                current_addr += ins.addressing_mode.byte_count();
            },
            Err(e) => {
                result.push(format!("${:04X}: {}", current_addr, e));
                current_addr += 1;
            }
        };
    }

    result
}

pub fn disassemble(addr: u16, instruction: &Instruction) -> String {
    use core::instructions::AddressingMode::*;

    let op_code = instruction.op_code;
    let token = instruction.token.to_string();
    let addressing_mode = &instruction.addressing_mode;

    let args: String = match *addressing_mode {
        ZeroPageIndexedX(arg) => format!("${:02X}, X", arg),
        ZeroPageIndexedY(arg) => format!("${:02X}, Y", arg),
        AbsoluteIndexedX(arg) => format!("${:04X}, X", arg),
        AbsoluteIndexedY(arg) => format!("${:04X}, Y", arg),
        IndexedIndirectX(arg) => format!("(${:02X}, X)", arg),
        IndirectIndexedY(arg) => format!("(${:02X}), Y", arg),

        Implicit => format!(""),
        Immediate(arg) => format!("#${:02X}", arg),
        Accumulator => format!("A"),
        ZeroPage(arg) => format!("${:02X}", arg),
        Absolute(arg) => format!("${:04X}", arg),
        Relative(arg) => format!("${:02X}", arg),
        Indirect(arg) => format!("(${:04X}", arg),

        Invalid => format!(""),
    };

    format!("${:04X}(${:02X}): {} {}", addr, op_code, token, args)
}