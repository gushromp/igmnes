use std::error::Error;

#[derive(Debug)]
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

#[derive(Debug)]
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

    Unknown,
}

#[derive(Debug)]
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
    pub fn decode(opcode: u8) -> Result<Instruction, String> {
        use self::InstructionToken::*;
        use self::AddressingMode::*;

        // Most instructions come with aaabbbcc bit form:
        //      aaa and cc bits are used to specify instruction type
        //      bbb bits are used to specify addressing mode
        // However, since a lot of instructions don't fit into this pattern,
        // we will match the opcodes one by one instead of looking at the individual bit groups.

        let instr = match opcode {
            //
            // Control, branch, and stack instructions
            //
            0x00 => Instruction::new(BRK, Implicit, 1, 7), // BReaK
            0xEA => Instruction::new(NOP, Implicit, 1, 2), // NOP (No OPeration)
            // Jump instructions
            0x20 => Instruction::new(JSR, Absolute, 3, 6), // Jump to SubRoutine
            0x4C => Instruction::new(JMP, Absolute, 3, 3), // JuMP (absolute)
            0x6C => Instruction::new(JMP, Indirect, 3, 5), // JuMP (indirect)
            // Return instructions
            0x40 => Instruction::new(RTI, Implicit, 1, 6), // RTI (ReTurn from Interrupt)
            0x60 => Instruction::new(RTS, Implicit, 1, 6), // RTS (ReTurn from Subroutine)
            // Branch instructions
            0x10 => Instruction::new(BPL, Relative, 2, 2), // Branch on PLus
            0x30 => Instruction::new(BMI, Relative, 2, 2), // Branch on MInus
            0x50 => Instruction::new(BVC, Relative, 2, 2), // Branch on oVerflow Clear
            0x70 => Instruction::new(BVS, Relative, 2, 2), // Branch on oVerflow Set
            0x90 => Instruction::new(BCC, Relative, 2, 2), // Branch on Carry Clear
            0xB0 => Instruction::new(BCS, Relative, 2, 2), // Branch on Carry Set
            0xD0 => Instruction::new(BNE, Relative, 2, 2), // Branch on Not Equal
            0xF0 => Instruction::new(BEQ, Relative, 2, 2), // Branch on EQual
            // Stack instructions
            0x9A => Instruction::new(TXS, Implicit, 1, 2), // PusH Processor status
            0xBA => Instruction::new(TSX, Implicit, 1, 2), // PuLl Processor status
            0x48 => Instruction::new(PHA, Implicit, 1, 3), // PusH Accumulator
            0x68 => Instruction::new(PLA, Implicit, 1, 4), // PuLl Accumulator
            0x08 => Instruction::new(PHP, Implicit, 1, 3), // Transfer X to Stack ptr
            0x28 => Instruction::new(PLP, Implicit, 1, 4), // Transfer Stack ptr to X
            // Flag instructions
            0x18 => Instruction::new(CLC, Implicit, 1, 2), // CLear Carry
            0x38 => Instruction::new(SEC, Implicit, 1, 2), // SEt Carry
            0x58 => Instruction::new(CLI, Implicit, 1, 2), // CLear Interrupt
            0x78 => Instruction::new(SEI, Implicit, 1, 2), // SEt Interrupt
            0xB8 => Instruction::new(CLV, Implicit, 1, 2), // CLear oVerflow
            0xD8 => Instruction::new(CLD, Implicit, 1, 2), // CLear Decimal
            0xF8 => Instruction::new(SED, Implicit, 1, 2), // SEt Decimal
            //
            // ALU instructions
            //
            // ORA (bitwise OR with Accumulator)
            0x09 => Instruction::new(ORA, Immediate, 2, 2),
            0x05 => Instruction::new(ORA, ZeroPage, 2, 3),
            0x15 => Instruction::new(ORA, ZeroPageIndexedX, 2, 4),
            0x0D => Instruction::new(ORA, Absolute, 3, 4),
            0x1D => Instruction::new(ORA, AbsoluteIndexedX, 3, 4),
            0x19 => Instruction::new(ORA, AbsoluteIndexedY, 3, 4),
            0x01 => Instruction::new(ORA, IndexedIndirectX, 2, 6),
            0x11 => Instruction::new(ORA, IndirectIndexedY, 2, 5),
            // AND (bitwise AND with accumulator)
            0x29 => Instruction::new(AND, Immediate, 2, 2),
            0x25 => Instruction::new(AND, ZeroPage, 2, 3),
            0x35 => Instruction::new(AND, ZeroPageIndexedX, 2, 4),
            0x2D => Instruction::new(AND, Absolute, 3, 4),
            0x3D => Instruction::new(AND, AbsoluteIndexedX, 3, 4),
            0x39 => Instruction::new(AND, AbsoluteIndexedY, 3, 4),
            0x21 => Instruction::new(AND, IndexedIndirectX, 2, 6),
            0x31 => Instruction::new(AND, IndirectIndexedY, 2, 5),
            // EOR (bitwise Exclusive OR)
            0x49 => Instruction::new(EOR, Immediate, 2, 2),
            0x45 => Instruction::new(EOR, ZeroPage, 2, 3),
            0x55 => Instruction::new(EOR, ZeroPageIndexedX, 2, 4),
            0x4D => Instruction::new(EOR, Absolute, 3, 4),
            0x5D => Instruction::new(EOR, AbsoluteIndexedX, 3, 4),
            0x59 => Instruction::new(EOR, AbsoluteIndexedY, 3, 4),
            0x41 => Instruction::new(EOR, IndexedIndirectX, 2, 6),
            0x51 => Instruction::new(EOR, IndirectIndexedY, 2, 5),
            // ADC (ADd with Carry)
            0x69 => Instruction::new(ADC, Immediate, 2, 2),
            0x65 => Instruction::new(ADC, ZeroPage, 2, 3),
            0x75 => Instruction::new(ADC, ZeroPageIndexedX, 2, 4),
            0x6D => Instruction::new(ADC, Absolute, 3, 4),
            0x7D => Instruction::new(ADC, AbsoluteIndexedX, 3, 4),
            0x79 => Instruction::new(ADC, AbsoluteIndexedY, 3, 4),
            0x61 => Instruction::new(ADC, IndexedIndirectX, 2, 6),
            0x71 => Instruction::new(ADC, IndirectIndexedY, 2, 5),
            // CMP (CoMPare accumulator)
            0xC9 => Instruction::new(CMP, Immediate, 2, 2),
            0xC5 => Instruction::new(CMP, ZeroPage, 2, 3),
            0xD5 => Instruction::new(CMP, ZeroPageIndexedX, 2, 4),
            0xCD => Instruction::new(CMP, Absolute, 3, 4),
            0xDD => Instruction::new(CMP, AbsoluteIndexedX, 3, 4),
            0xD9 => Instruction::new(CMP, AbsoluteIndexedY, 3, 4),
            0xC1 => Instruction::new(CMP, IndexedIndirectX, 2, 6),
            0xD1 => Instruction::new(CMP, IndirectIndexedY, 2, 5),
            // SBC (SuBtract with Carry)
            0xE9 => Instruction::new(SBC, Immediate, 2, 2),
            0xE5 => Instruction::new(SBC, ZeroPage, 2, 3),
            0xF5 => Instruction::new(SBC, ZeroPageIndexedX, 2, 4),
            0xED => Instruction::new(SBC, Absolute, 3, 4),
            0xFD => Instruction::new(SBC, AbsoluteIndexedX, 3, 4),
            0xF9 => Instruction::new(SBC, AbsoluteIndexedY, 3, 4),
            0xE1 => Instruction::new(SBC, IndexedIndirectX, 2, 6),
            0xF1 => Instruction::new(SBC, IndirectIndexedY, 2, 5),
            //
            // Read/Modify/Write instructions
            //
            // ASL (Arithmetic Shift Left)
            0x0A => Instruction::new(ASL, Accumulator, 1, 2),
            0x06 => Instruction::new(ASL, ZeroPage, 2, 5),
            0x16 => Instruction::new(ASL, ZeroPageIndexedX, 2, 6),
            0x0E => Instruction::new(ASL, Absolute, 3, 6),
            0x1E => Instruction::new(ASL, AbsoluteIndexedX, 3, 7),
            // ROL (ROtate Left)
            0x2A => Instruction::new(ROL, Accumulator, 1, 2),
            0x26 => Instruction::new(ROL, ZeroPage, 2, 5),
            0x36 => Instruction::new(ROL, ZeroPageIndexedX, 2, 6),
            0x2E => Instruction::new(ROL, Absolute, 3, 6),
            0x3E => Instruction::new(ROL, AbsoluteIndexedX, 3, 7),
            // LSR (Logical Shift Right)
            0x4A => Instruction::new(ASL, Accumulator, 1, 2),
            0x46 => Instruction::new(ASL, ZeroPage, 2, 5),
            0x56 => Instruction::new(ASL, ZeroPageIndexedX, 2, 6),
            0x4E => Instruction::new(ASL, Absolute, 3, 6),
            0x5E => Instruction::new(ASL, AbsoluteIndexedX, 3, 7),
            // ROR (ROtate Right)
            0x6A => Instruction::new(ROL, Accumulator, 1, 2),
            0x66 => Instruction::new(ROL, ZeroPage, 2, 5),
            0x76 => Instruction::new(ROL, ZeroPageIndexedX, 2, 6),
            0x6E => Instruction::new(ROL, Absolute, 3, 6),
            0x7E => Instruction::new(ROL, AbsoluteIndexedX, 3, 7),
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
            // Store/Load instructions
            //
            // LDA (LoaD Accumulator)
            0xA9 => Instruction::new(LDA, Immediate, 2, 2),
            0xA5 => Instruction::new(LDA, ZeroPage, 2, 3),
            0xB5 => Instruction::new(LDA, ZeroPageIndexedX, 2, 4),
            0xAD => Instruction::new(LDA, Absolute, 3, 4),
            0xBD => Instruction::new(LDA, AbsoluteIndexedX, 3, 4),
            0xB9 => Instruction::new(LDA, AbsoluteIndexedY, 3, 4),
            0xA1 => Instruction::new(LDA, IndexedIndirectX, 2, 6),
            0xB1 => Instruction::new(LDA, IndirectIndexedY, 2, 5),
            // LDX (LoaD X register)
            0xA2 => Instruction::new(LDX, Immediate, 2, 2),
            0xA6 => Instruction::new(LDX, ZeroPage, 2, 3),
            0xB6 => Instruction::new(LDX, ZeroPageIndexedY, 2, 4),
            0xAE => Instruction::new(LDX, Absolute, 3, 4),
            0xBE => Instruction::new(LDX, AbsoluteIndexedY, 3, 4),
            // LDY (LoaD Y register)
            0xA0 => Instruction::new(LDY, Immediate, 2, 2),
            0xA4 => Instruction::new(LDY, ZeroPage, 2, 3),
            0xB4 => Instruction::new(LDY, ZeroPageIndexedX, 2, 4),
            0xAC => Instruction::new(LDY, Absolute, 3, 4),
            0xBC => Instruction::new(LDY, AbsoluteIndexedX, 3, 4),
            // STA (STore Accumulator)
            0x85 => Instruction::new(STA, ZeroPage, 2, 3),
            0x95 => Instruction::new(STA, ZeroPageIndexedX, 2, 4),
            0x8D => Instruction::new(STA, Absolute, 3, 4),
            0x9D => Instruction::new(STA, AbsoluteIndexedX, 3, 5),
            0x99 => Instruction::new(STA, AbsoluteIndexedY, 3, 5),
            0x81 => Instruction::new(STA, IndexedIndirectX, 2, 6),
            0x91 => Instruction::new(STA, IndirectIndexedY, 2, 6),
            // STX (STore X register)
            0x86 => Instruction::new(STX, ZeroPage, 2, 3),
            0x96 => Instruction::new(STX, ZeroPageIndexedY, 2, 4),
            0x8E => Instruction::new(STX, Absolute, 3, 4),
            // STY (STore Y register)
            0x84 => Instruction::new(STY, ZeroPage, 2, 3),
            0x94 => Instruction::new(STY, ZeroPageIndexedX, 2, 4),
            0x8C => Instruction::new(STY, Absolute, 3, 4),

            _ => Instruction::new(Unknown, Invalid, 0, 0)
        };

        if let Unknown = instr.token {
            Err(format!("Unknown opcode: 0x{:x}", opcode))
        }
        else {
            Ok(instr)
        }
    }
}