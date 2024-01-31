use std::ops::Range;
use core::instructions::Instruction;
use core::memory::MemMapped;
use core::cpu::Cpu;
use core::errors::EmulationError;

pub fn disassemble_range(addr: u16, range: &Range<u16>, cpu: &Cpu, mem_map: &mut dyn MemMapped)
                         -> Result<Vec<String>, EmulationError> {
    let mut result = Vec::new();
    let mut current_addr = addr;

    let range = range.clone();
    for _i in range {
        let index = current_addr as u16;
        let mut instruction = Instruction::decode(mem_map, index);

        match instruction {
            Ok(ref mut ins) => {
                result.push(disassemble(current_addr, ins, cpu, mem_map)?);
                current_addr += ins.addressing_mode.byte_count();
            },
            Err(e) => {
                result.push(format!("${:04X}: {}", current_addr, e));
                current_addr += 1;
            }
        };
    }

    Ok(result)
}

pub fn disassemble(addr: u16, instruction: &mut Instruction, cpu: &Cpu, mem_map: &mut dyn MemMapped)
                   -> Result<String, EmulationError> {
    use core::instructions::AddressingMode::*;

    let op_code = instruction.op_code;
    let token = instruction.token.to_string();

    let resolved = cpu.read_resolved(instruction, mem_map)?;
    let addressing_mode = &instruction.addressing_mode;

    let (args, detail) = match *addressing_mode {
        ZeroPageIndexedX(arg) => {
            (format!("${:02X}, X", arg),
             format!("[${:04X}: ${:02X}]", arg.wrapping_add(cpu.reg_x), resolved))
        },
        ZeroPageIndexedY(arg) => {
            (format!("${:02X}, Y", arg),
             format!("[${:04X}: ${:02X}]", arg.wrapping_add(cpu.reg_y), resolved))
        },
        AbsoluteIndexedX(arg) => {
            (format!("${:04X}, X", arg),
             format!("[${:04X}: ${:02X}]", arg.wrapping_add(cpu.reg_x as u16), resolved))
        },
        AbsoluteIndexedY(arg) => {
            (format!("${:04X}, Y", arg),
             format!("[${:04X}: ${:02X}]", arg.wrapping_add(cpu.reg_y as u16), resolved))
        },
        IndexedIndirectX(arg) => {
            let arg = arg.wrapping_add(cpu.reg_x);
            let addr_low = mem_map.read(arg as u16)?;
            let addr_high = mem_map.read(arg.wrapping_add(1) as u16)?;

            // See comment in the read_resolved function
            let addr = ((addr_high as u16) << 8) | addr_low as u16;

            (format!("(${:02X}, X)", arg),
             format!("[${:04X}: ${:02X}]", addr, resolved))
        },
        IndirectIndexedY(arg) => {
            let arg_resolved = mem_map.read_word(arg as u16)?;
            let addr = arg_resolved.wrapping_add(cpu.reg_y as u16);

            (format!("(${:02X}), Y", arg),
             format!("[${:04X}: ${:02X}]", addr, resolved))
        },

        Implicit => (format!(""), format!("")),
        Immediate(arg) => (format!("#${:02X}", arg), format!("")),
        Accumulator => (format!("A"), format!("[A: {:02X}]", cpu.reg_a)),
        ZeroPage(arg) => (format!("${:02X}", arg), format!("[${:02X}: ${:02X}]", arg, resolved)),
        Absolute(arg) => (format!("${:04X}", arg), format!("[${:X}]", resolved)),
        Relative(arg) => {
            (format!("${:02X}", arg),
             format!("[PC -> ${:04X}]", (cpu.reg_pc as i32 + arg as i32) + 2))
        }
        Indirect(arg) => {
            let addr_high = arg >> 8;
            let addr_low_1 = (arg & 0xFF) as u8;
            let addr_low_2 = addr_low_1.wrapping_add(1);

            let resolved_low = (addr_high << 8) | addr_low_1 as u16;
            let resolved_high = (addr_high << 8) | addr_low_2 as u16;

            let target_addr_low = mem_map.read(resolved_low)?;
            let target_addr_high = mem_map.read(resolved_high)?;

            let target_addr = ((target_addr_high as u16) << 8) | target_addr_low as u16;

            (format!("(${:04X})", arg),
             format!("[${:04X}]", target_addr))
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

    Ok(format!("${:04X}(${:02X}): {:<2} {:<10} {:<20}", addr, op_code, token, args, detail))
}