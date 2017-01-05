use core::instructions::Instruction;

pub fn disassemble(instruction: Instruction) -> String {
    let token = instruction.token.to_string();

    token
}