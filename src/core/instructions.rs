pub enum AddressingMode {
    // indexed addressing modes
    //
    ZeroPageIndexedX,
    ZeroPageIndexedY,
    AbsoluteIndexedX,
    AbsoluteIndexedY,
    IndexedIndirectX,
    IndirectIndexedY,

    // other addressing modes
    //
    Implicit,
    Immediate,
    Accumulator,
    ZeroPage,
    Absolute,
    Relative,
    Indirect,

    Invalid,
}

pub enum InstructionToken {
    // instruction opcodes are byte-wide
    //
    // opcode cc bits == 01
    // ALU instructions
    //
    ORA,
    AND,
    EOR,
    ADC,
    CMP,
    SBC,
    //
    // opcode cc bits == 10
    // Read-Modify-Write (RMW) instructions
    //
    ASL,
    ROL,
    LSR,
    ROR,
    DEC,
    INC,
    //
    // Control and branch instructions
    //
    BIT,
    JMP,
    CPY,
    CPX,
    BPL,
    BMI,
    BVC,
    BVS,
    BCC,
    BCS,
    BNE,
    BEQ,
    //
    // load and store instructions
    //
    STA,
    LDA,
    STX,
    LDX,
    STY,
    LDY,
    // interrupt and subroutine instructions (single byte)
    //
    BRK,
    JSR,
    RTI,
    RTS,
    // rest of single byte instructions
    //
    PHP,
    PLP,
    PHA,
    PLA,
    DEY,
    TAY,
    INY,
    INX,
    CLC,
    SEC,
    CLI,
    SEI,
    TYA,
    CLV,
    CLD,
    SED,
    TXA,
    TXS,
    TAX,
    TSX,
    DEX,
    NOP,

    Invalid,
}


pub struct Instruction {
    pub token: InstructionToken,
    pub addressing_mode: AddressingMode,
    pub byte_count: u8,
    pub cycle_count: u8,
}

impl Instruction {
    pub fn new(token: InstructionToken, addressing_mode: AddressingMode,
               byte_count: u8, cycle_count: u8) -> Instruction {
        Instruction {
            token: token,
            addressing_mode: addressing_mode,
            byte_count: byte_count,
            cycle_count: cycle_count,
        }
    }
}

impl Instruction {
    pub fn decode(opcode: u8) -> Instruction {
        use self::InstructionToken::*;
        use self::AddressingMode::*;

        // Most instructions come with aaabbbcc bit form:
        //      aaa and cc bits are used to specify instruction type
        //      bbb bits are used to specify addressing mode
        // However, since a lot of instructions don't fit into this pattern,
        // we will match them one by one instead of looking at the individual bit groups.

        match opcode {
            //
            // Control, branch, and stack instructions
            //
            0x00 => Instruction::new(BRK, Implicit, 1, 7), // BReaK
            // Jump instructions
            0x20 => Instruction::new(JSR, Absolute, 3, 6), // Jump to SubRoutine
            0x4C => Instruction::new(JMP, Absolute, 3, 3), // JuMP
            0x6C => Instruction::new(JMP, Indirect, 3, 5), // JuMP
            // Branch instructions
            0x10 => Instruction::new(BPL, Implicit, 1, 2), // Branch on PLus
            0x30 => Instruction::new(BMI, Implicit, 1, 2), // Branch on MInus
            0x50 => Instruction::new(BVC, Implicit, 1, 2), // Branch on oVerflow Clear
            0x70 => Instruction::new(BVS, Implicit, 1, 2), // Branch on oVerflow Set
            0x90 => Instruction::new(BCC, Implicit, 1, 2), // Branch on Carry Clear
            0xB0 => Instruction::new(BCS, Implicit, 1, 2), // Branch on Carry Set
            0xD0 => Instruction::new(BNE, Implicit, 1, 2), // Branch on Not Equal
            0xF0 => Instruction::new(BEQ, Implicit, 1, 2), // Branch on EQual
            // Stack instructions
            0x08 => Instruction::new(PHP, Implicit, 1, 3), // Transfer X to Stack ptr
            0x28 => Instruction::new(PLP, Implicit, 1, 4), // Transfer Stack ptr to X
            0x48 => Instruction::new(PHA, Implicit, 1, 3), // PusH Accumulator
            0x68 => Instruction::new(PLA, Implicit, 1, 4), // PuLl Accumulator
            0x9A => Instruction::new(TXS, Implicit, 1, 2), // PusH Processor status
            0xBA => Instruction::new(TSX, Implicit, 1, 2), // PuLl Processor status
            // Flag instructions
            0x18 => Instruction::new(CLC, Implicit, 1, 2), // CLear Carry
            0x38 => Instruction::new(SEC, Implicit, 1, 2), // SEt Carry
            0x58 => Instruction::new(CLI, Implicit, 1, 2), // CLear Interrupt
            0x78 => Instruction::new(SEI, Implicit, 1, 2), // SEt Interrupt
            0xB8 => Instruction::new(CLV, Implicit, 1, 2), // CLear oVerflow
            0xD8 => Instruction::new(CLD, Implicit, 1, 2), // CLear Decimal
            0xF8 => Instruction::new(SED, Implicit, 1, 2), // SEt Decimal
            // Register instructions
            0xAA => Instruction::new(TAX, Implicit, 1, 2), // Transfer A to X
            0x8A => Instruction::new(TXA, Implicit, 1, 2), // Transfer X to A
            0xCA => Instruction::new(DEX, Implicit, 1, 2), // DEcrement X
            0xE8 => Instruction::new(INX, Implicit, 1, 2), // INcrement X
            0xA8 => Instruction::new(TAY, Implicit, 1, 2), // Transfer A to Y
            0x98 => Instruction::new(TYA, Implicit, 1, 2), // Transfer Y to A
            0x88 => Instruction::new(DEY, Implicit, 1, 2), // DEcrement Y
            0xC8 => Instruction::new(INY, Implicit, 1, 2), // INcrement Y
            //
            // ALU instructions
            //
            // ORA bitwise OR with Accumulator
            0x09 => Instruction::new(ORA, Immediate, 2, 2),
            0x05 => Instruction::new(ORA, ZeroPage, 2, 3),
            0x15 => Instruction::new(ORA, ZeroPageIndexedX, 2, 4),
            0x0D => Instruction::new(ORA, Absolute, 3, 4),
            0x1D => Instruction::new(ORA, AbsoluteIndexedX, 3, 4),
            0x19 => Instruction::new(ORA, AbsoluteIndexedY, 3, 4),
            0x01 => Instruction::new(ORA, IndexedIndirectX, 2, 6),
            0x11 => Instruction::new(ORA, IndirectIndexedY, 2, 5),
        }
    }
}