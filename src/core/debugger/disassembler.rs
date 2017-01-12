use std::ops::Range;
use core::instructions::Instruction;
use core::memory::MemMapped;
use core::cpu::Cpu;

pub fn disassemble_range(addr: u16, range: &Range<u16>, cpu: &Cpu, mem_map: &MemMapped) -> Vec<String> {
    let mut result = Vec::new();
    let mut current_addr = addr;

    let range = range.clone();
    for i in range {
        let index = current_addr as u16;
        let mut instruction = Instruction::decode(mem_map, index);

        match instruction {
            Ok(ref mut ins) => {
                result.push(disassemble(current_addr, ins, cpu, mem_map));
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

pub fn disassemble(addr: u16, instruction: &mut Instruction, cpu: &Cpu, mem_map: &MemMapped) -> String {
    use core::instructions::AddressingMode::*;

    let op_code = instruction.op_code;
    let token = instruction.token.to_string();

    let resolved = cpu.read_resolved(instruction, mem_map);
    let addressing_mode = &instruction.addressing_mode;

    let (args, detail) = match *addressing_mode {
        ZeroPageIndexedX(arg) => {
            (format!("${:02X}, X", arg),
             format!("[${:04X}: ${:02X}]", arg + cpu.reg_x, resolved))
        },
        ZeroPageIndexedY(arg) => {
            (format!("${:02X}, Y", arg),
             format!("[${:04X}: ${:02X}]", arg + cpu.reg_y, resolved))
        },
        AbsoluteIndexedX(arg) => {
            (format!("${:04X}, X", arg),
             format!("[${:04X}: ${:02X}]", arg + cpu.reg_x as u16, resolved))
        },
        AbsoluteIndexedY(arg) => {
            (format!("${:04X}, Y", arg),
             format!("[${:04X}: ${:02X}]", arg + cpu.reg_y as u16, resolved))
        },
        IndexedIndirectX(arg) => {
            (format!("(${:02X}, X)", arg),
             format!("[${:04X}: ${:02X}]", mem_map.read_word((arg as u16).wrapping_add(cpu.reg_x as u16)), resolved))
        },
        IndirectIndexedY(arg) => {
            (format!("(${:02X}), Y", arg),
             format!("[${:04X}: ${:02X}]", mem_map.read_word(arg as u16).wrapping_add(cpu.reg_y as u16), resolved))
        },

        Implicit => (format!(""), format!("")),
        Immediate(arg) => (format!("#${:02X}", arg), format!("")),
        Accumulator => (format!("A"), format!("[A: {:02X}]", cpu.reg_a)),
        ZeroPage(arg) => (format!("${:02X}", arg), format!("[${:02X}: ${:02X}]", arg, resolved)),
        Absolute(arg) => (format!("${:04X}", arg), format!("")),
        Relative(arg) => {
            (format!("${:02X}", arg),
             format!("[PC -> ${:04X}]", (cpu.reg_pc as i32 + arg as i32) + 2))
        }
        Indirect(arg) => {
            (format!("(${:04X})", arg),
             format!("[${:04X}]", mem_map.read_word(arg)))
        },

        Invalid => (format!(""), format!(""))
    };

    let detail = {
        if !detail.is_empty() {
            format!("| {}", detail)
        } else {
            format!("")
        }
    };

    format!("${:04X}(${:02X}): {:<2} {:<10} {:<20}", addr, op_code, token, args, detail)
}