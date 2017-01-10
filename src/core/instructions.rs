use std::error::Error;
use std::fmt;
use core::memory::MemMapped;

#[derive(Debug)]
pub enum AddressingMode {
    //
    // indexed addressing modes
    //
    ZeroPageIndexedX(u8),
    ZeroPageIndexedY(u8),
    AbsoluteIndexedX(u16),
    AbsoluteIndexedY(u16),
    IndexedIndirectX(u8),
    IndirectIndexedY(u8),
    //
    // other addressing modes
    //
    Implicit,
    Immediate(u8),
    Accumulator,
    ZeroPage(u8),
    Absolute(u16),
    Relative(u8),
    Indirect(u16),

    Invalid,
}

impl AddressingMode {
    pub fn byte_count(&self) -> u16 {
        use self::AddressingMode::*;

        match *self {
            ZeroPageIndexedX(_) => 2,
            ZeroPageIndexedY(_) => 2,
            AbsoluteIndexedX(_) => 3,
            AbsoluteIndexedY(_) => 3,
            IndexedIndirectX(_) => 2,
            IndirectIndexedY(_) => 2,

            Implicit => 1,
            Immediate(_) => 2,
            Accumulator => 1,
            ZeroPage(_) => 2,
            Absolute(_) => 3,
            Relative(_) => 2,
            Indirect(_) => 3,

            Invalid => 0,
        }
    }
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

impl fmt::Display for InstructionToken {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug)]
pub struct Instruction {
    pub token: InstructionToken,
    pub addressing_mode: AddressingMode,
    pub cycle_count: u8,
}

impl Instruction {
    pub fn new(token: InstructionToken, addressing_mode: AddressingMode,
               cycle_count: u8) -> Instruction {
        Instruction {
            token: token,
            addressing_mode: addressing_mode,
            cycle_count: cycle_count,
        }
    }
}

impl Instruction {
    pub fn decode(mem_map: &MemMapped, index: u16) -> Result<Instruction, String> {
        use self::InstructionToken::*;
        use self::AddressingMode::*;

        // Most instructions come with aaabbbcc bit form:
        //      aaa and cc bits are used to specify instruction type
        //      bbb bits are used to specify addressing mode
        // However, since a lot of instructions don't fit into this pattern,
        // we will match the opcodes one by one instead of looking at the individual bit groups.
        let op_code = mem_map.read(index);
        let arg_index = index + 1;

        let instr = match op_code {
            //
            // Control, branch, and stack instructions
            //
            0x00 => Instruction::new(BRK, Implicit, 7), // BReaK
            0xEA => Instruction::new(NOP, Implicit, 2), // NOP (No OPeration)
            // Jump instructions
            0x20 => Instruction::new(JSR, Absolute(mem_map.read_word(arg_index)), 6), // Jump to SubRoutine
            0x4C => Instruction::new(JMP, Absolute(mem_map.read_word(arg_index)), 3), // JuMP (absolute)
            0x6C => Instruction::new(JMP, Indirect(mem_map.read_word(arg_index)), 5), // JuMP (indirect)
            // Return instructions
            0x40 => Instruction::new(RTI, Implicit, 6), // RTI (ReTurn from Interrupt)
            0x60 => Instruction::new(RTS, Implicit, 6), // RTS (ReTurn from Subroutine)
            // Branch instructions
            0x10 => Instruction::new(BPL, Relative(mem_map.read(arg_index)), 2), // Branch on PLus
            0x30 => Instruction::new(BMI, Relative(mem_map.read(arg_index)), 2), // Branch on MInus
            0x50 => Instruction::new(BVC, Relative(mem_map.read(arg_index)), 2), // Branch on oVerflow Clear
            0x70 => Instruction::new(BVS, Relative(mem_map.read(arg_index)), 2), // Branch on oVerflow Set
            0x90 => Instruction::new(BCC, Relative(mem_map.read(arg_index)), 2), // Branch on Carry Clear
            0xB0 => Instruction::new(BCS, Relative(mem_map.read(arg_index)), 2), // Branch on Carry Set
            0xD0 => Instruction::new(BNE, Relative(mem_map.read(arg_index)), 2), // Branch on Not Equal
            0xF0 => Instruction::new(BEQ, Relative(mem_map.read(arg_index)), 2), // Branch on EQual
            // Stack instructions
            0x9A => Instruction::new(TXS, Implicit, 2), // PusH Processor status
            0xBA => Instruction::new(TSX, Implicit, 2), // PuLl Processor status
            0x48 => Instruction::new(PHA, Implicit, 3), // PusH Accumulator
            0x68 => Instruction::new(PLA, Implicit, 4), // PuLl Accumulator
            0x08 => Instruction::new(PHP, Implicit, 3), // Transfer X to Stack ptr
            0x28 => Instruction::new(PLP, Implicit, 4), // Transfer Stack ptr to X
            // Flag instructions
            0x18 => Instruction::new(CLC, Implicit, 2), // CLear Carry
            0x38 => Instruction::new(SEC, Implicit, 2), // SEt Carry
            0x58 => Instruction::new(CLI, Implicit, 2), // CLear Interrupt
            0x78 => Instruction::new(SEI, Implicit, 2), // SEt Interrupt
            0xB8 => Instruction::new(CLV, Implicit, 2), // CLear oVerflow
            0xD8 => Instruction::new(CLD, Implicit, 2), // CLear Decimal
            0xF8 => Instruction::new(SED, Implicit, 2), // SEt Decimal
            //
            // ALU instructions
            //
            // ORA (bitwise OR with Accumulator)
            0x09 => Instruction::new(ORA, Immediate(mem_map.read(arg_index)), 2),
            0x05 => Instruction::new(ORA, ZeroPage(mem_map.read(arg_index)), 3),
            0x15 => Instruction::new(ORA, ZeroPageIndexedX(mem_map.read(arg_index)), 4),
            0x0D => Instruction::new(ORA, Absolute(mem_map.read_word(arg_index)), 4),
            0x1D => Instruction::new(ORA, AbsoluteIndexedX(mem_map.read_word(arg_index)), 4),
            0x19 => Instruction::new(ORA, AbsoluteIndexedY(mem_map.read_word(arg_index)), 4),
            0x01 => Instruction::new(ORA, IndexedIndirectX(mem_map.read(arg_index)), 6),
            0x11 => Instruction::new(ORA, IndirectIndexedY(mem_map.read(arg_index)), 5),
            // AND (bitwise AND with accumulator)
            0x29 => Instruction::new(AND, Immediate(mem_map.read(arg_index)), 2),
            0x25 => Instruction::new(AND, ZeroPage(mem_map.read(arg_index)), 3),
            0x35 => Instruction::new(AND, ZeroPageIndexedX(mem_map.read(arg_index)), 4),
            0x2D => Instruction::new(AND, Absolute(mem_map.read_word(arg_index)), 4),
            0x3D => Instruction::new(AND, AbsoluteIndexedX(mem_map.read_word(arg_index)), 4),
            0x39 => Instruction::new(AND, AbsoluteIndexedY(mem_map.read_word(arg_index)), 4),
            0x21 => Instruction::new(AND, IndexedIndirectX(mem_map.read(arg_index)), 6),
            0x31 => Instruction::new(AND, IndirectIndexedY(mem_map.read(arg_index)), 5),
            // EOR (bitwise Exclusive OR)
            0x49 => Instruction::new(EOR, Immediate(mem_map.read(arg_index)), 2),
            0x45 => Instruction::new(EOR, ZeroPage(mem_map.read(arg_index)), 3),
            0x55 => Instruction::new(EOR, ZeroPageIndexedX(mem_map.read(arg_index)), 4),
            0x4D => Instruction::new(EOR, Absolute(mem_map.read_word(arg_index)), 4),
            0x5D => Instruction::new(EOR, AbsoluteIndexedX(mem_map.read_word(arg_index)), 4),
            0x59 => Instruction::new(EOR, AbsoluteIndexedY(mem_map.read_word(arg_index)), 4),
            0x41 => Instruction::new(EOR, IndexedIndirectX(mem_map.read(arg_index)), 6),
            0x51 => Instruction::new(EOR, IndirectIndexedY(mem_map.read(arg_index)), 5),
            // ADC (ADd with Carry)
            0x69 => Instruction::new(ADC, Immediate(mem_map.read(arg_index)), 2),
            0x65 => Instruction::new(ADC, ZeroPage(mem_map.read(arg_index)), 3),
            0x75 => Instruction::new(ADC, ZeroPageIndexedX(mem_map.read(arg_index)), 4),
            0x6D => Instruction::new(ADC, Absolute(mem_map.read_word(arg_index)), 4),
            0x7D => Instruction::new(ADC, AbsoluteIndexedX(mem_map.read_word(arg_index)), 4),
            0x79 => Instruction::new(ADC, AbsoluteIndexedY(mem_map.read_word(arg_index)), 4),
            0x61 => Instruction::new(ADC, IndexedIndirectX(mem_map.read(arg_index)), 6),
            0x71 => Instruction::new(ADC, IndirectIndexedY(mem_map.read(arg_index)), 5),
            // CMP (CoMPare accumulator)
            0xC9 => Instruction::new(CMP, Immediate(mem_map.read(arg_index)), 2),
            0xC5 => Instruction::new(CMP, ZeroPage(mem_map.read(arg_index)), 3),
            0xD5 => Instruction::new(CMP, ZeroPageIndexedX(mem_map.read(arg_index)), 4),
            0xCD => Instruction::new(CMP, Absolute(mem_map.read_word(arg_index)), 4),
            0xDD => Instruction::new(CMP, AbsoluteIndexedX(mem_map.read_word(arg_index)), 4),
            0xD9 => Instruction::new(CMP, AbsoluteIndexedY(mem_map.read_word(arg_index)), 4),
            0xC1 => Instruction::new(CMP, IndexedIndirectX(mem_map.read(arg_index)), 6),
            0xD1 => Instruction::new(CMP, IndirectIndexedY(mem_map.read(arg_index)), 5),
            // SBC (SuBtract with Carry)
            0xE9 => Instruction::new(SBC, Immediate(mem_map.read(arg_index)), 2),
            0xE5 => Instruction::new(SBC, ZeroPage(mem_map.read(arg_index)), 3),
            0xF5 => Instruction::new(SBC, ZeroPageIndexedX(mem_map.read(arg_index)), 4),
            0xED => Instruction::new(SBC, Absolute(mem_map.read_word(arg_index)), 4),
            0xFD => Instruction::new(SBC, AbsoluteIndexedX(mem_map.read_word(arg_index)), 4),
            0xF9 => Instruction::new(SBC, AbsoluteIndexedY(mem_map.read_word(arg_index)), 4),
            0xE1 => Instruction::new(SBC, IndexedIndirectX(mem_map.read(arg_index)), 6),
            0xF1 => Instruction::new(SBC, IndirectIndexedY(mem_map.read(arg_index)), 5),
            // CPX (ComPare X register)
            0xE0 => Instruction::new(CPX, Immediate(mem_map.read(arg_index)), 2),
            0xE4 => Instruction::new(CPX, ZeroPage(mem_map.read(arg_index)), 3),
            0xEC => Instruction::new(CPX, Absolute(mem_map.read_word(arg_index)), 4),
            // CPY (ComPare Y register)
            0xC0 => Instruction::new(CPY, Immediate(mem_map.read(arg_index)), 2),
            0xC4 => Instruction::new(CPY, ZeroPage(mem_map.read(arg_index)), 3),
            0xCC => Instruction::new(CPY, Absolute(mem_map.read_word(arg_index)), 4),
            // BIT (test BITs)
            0x24 => Instruction::new(BIT, ZeroPage(mem_map.read(arg_index)), 3),
            0x2C => Instruction::new(BIT, Absolute(mem_map.read_word(arg_index)), 4),
            //
            // Read/Modify/Write instructions
            //
            // ASL (Arithmetic Shift Left)
            0x0A => Instruction::new(ASL, Accumulator, 2),
            0x06 => Instruction::new(ASL, ZeroPage(mem_map.read(arg_index)), 5),
            0x16 => Instruction::new(ASL, ZeroPageIndexedX(mem_map.read(arg_index)), 6),
            0x0E => Instruction::new(ASL, Absolute(mem_map.read_word(arg_index)), 6),
            0x1E => Instruction::new(ASL, AbsoluteIndexedX(mem_map.read_word(arg_index)), 7),
            // ROL (ROtate Left)
            0x2A => Instruction::new(ROL, Accumulator, 2),
            0x26 => Instruction::new(ROL, ZeroPage(mem_map.read(arg_index)), 5),
            0x36 => Instruction::new(ROL, ZeroPageIndexedX(mem_map.read(arg_index)), 6),
            0x2E => Instruction::new(ROL, Absolute(mem_map.read_word(arg_index)), 6),
            0x3E => Instruction::new(ROL, AbsoluteIndexedX(mem_map.read_word(arg_index)), 7),
            // LSR (Logical Shift Right)
            0x4A => Instruction::new(LSR, Accumulator, 2),
            0x46 => Instruction::new(LSR, ZeroPage(mem_map.read(arg_index)), 5),
            0x56 => Instruction::new(LSR, ZeroPageIndexedX(mem_map.read(arg_index)), 6),
            0x4E => Instruction::new(LSR, Absolute(mem_map.read_word(arg_index)), 6),
            0x5E => Instruction::new(LSR, AbsoluteIndexedX(mem_map.read_word(arg_index)), 7),
            // ROR (ROtate Right)
            0x6A => Instruction::new(ROR, Accumulator, 2),
            0x66 => Instruction::new(ROR, ZeroPage(mem_map.read(arg_index)), 5),
            0x76 => Instruction::new(ROR, ZeroPageIndexedX(mem_map.read(arg_index)), 6),
            0x6E => Instruction::new(ROR, Absolute(mem_map.read_word(arg_index)), 6),
            0x7E => Instruction::new(ROR, AbsoluteIndexedX(mem_map.read_word(arg_index)), 7),
            // DEC (DECrement memory)
            0xC6 => Instruction::new(DEC, ZeroPage(mem_map.read(arg_index)), 5),
            0xD6 => Instruction::new(DEC, ZeroPageIndexedX(mem_map.read(arg_index)), 6),
            0xCE => Instruction::new(DEC, Absolute(mem_map.read_word(arg_index)), 6),
            0xDE => Instruction::new(DEC, AbsoluteIndexedX(mem_map.read_word(arg_index)), 7),
            // INC (INCrement memory)
            0xE6 => Instruction::new(INC, ZeroPage(mem_map.read(arg_index)), 5),
            0xF6 => Instruction::new(INC, ZeroPageIndexedX(mem_map.read(arg_index)), 6),
            0xEE => Instruction::new(INC, Absolute(mem_map.read_word(arg_index)), 6),
            0xFE => Instruction::new(INC, AbsoluteIndexedX(mem_map.read_word(arg_index)), 7),
            // Register instructions
            0xAA => Instruction::new(TAX, Implicit, 2), // Transfer A to X
            0x8A => Instruction::new(TXA, Implicit, 2), // Transfer X to A
            0xCA => Instruction::new(DEX, Implicit, 2), // DEcrement X
            0xE8 => Instruction::new(INX, Implicit, 2), // INcrement X
            0xA8 => Instruction::new(TAY, Implicit, 2), // Transfer A to Y
            0x98 => Instruction::new(TYA, Implicit, 2), // Transfer Y to A
            0x88 => Instruction::new(DEY, Implicit, 2), // DEcrement Y
            0xC8 => Instruction::new(INY, Implicit, 2), // INcrement Y
            //
            // Store/Load instructions
            //
            // LDA (LoaD Accumulator)
            0xA9 => Instruction::new(LDA, Immediate(mem_map.read(arg_index)), 2),
            0xA5 => Instruction::new(LDA, ZeroPage(mem_map.read(arg_index)), 3),
            0xB5 => Instruction::new(LDA, ZeroPageIndexedX(mem_map.read(arg_index)), 4),
            0xAD => Instruction::new(LDA, Absolute(mem_map.read_word(arg_index)), 4),
            0xBD => Instruction::new(LDA, AbsoluteIndexedX(mem_map.read_word(arg_index)), 4),
            0xB9 => Instruction::new(LDA, AbsoluteIndexedY(mem_map.read_word(arg_index)), 4),
            0xA1 => Instruction::new(LDA, IndexedIndirectX(mem_map.read(arg_index)), 6),
            0xB1 => Instruction::new(LDA, IndirectIndexedY(mem_map.read(arg_index)), 5),
            // LDX (LoaD X register)
            0xA2 => Instruction::new(LDX, Immediate(mem_map.read(arg_index)), 2),
            0xA6 => Instruction::new(LDX, ZeroPage(mem_map.read(arg_index)), 3),
            0xB6 => Instruction::new(LDX, ZeroPageIndexedY(mem_map.read(arg_index)), 4),
            0xAE => Instruction::new(LDX, Absolute(mem_map.read_word(arg_index)), 4),
            0xBE => Instruction::new(LDX, AbsoluteIndexedY(mem_map.read_word(arg_index)), 4),
            // LDY (LoaD Y register)
            0xA0 => Instruction::new(LDY, Immediate(mem_map.read(arg_index)), 2),
            0xA4 => Instruction::new(LDY, ZeroPage(mem_map.read(arg_index)), 3),
            0xB4 => Instruction::new(LDY, ZeroPageIndexedX(mem_map.read(arg_index)), 4),
            0xAC => Instruction::new(LDY, Absolute(mem_map.read_word(arg_index)), 4),
            0xBC => Instruction::new(LDY, AbsoluteIndexedX(mem_map.read_word(arg_index)),  4),
            // STA (STore Accumulator)
            0x85 => Instruction::new(STA, ZeroPage(mem_map.read(arg_index)), 3),
            0x95 => Instruction::new(STA, ZeroPageIndexedX(mem_map.read(arg_index)), 4),
            0x8D => Instruction::new(STA, Absolute(mem_map.read_word(arg_index)), 4),
            0x9D => Instruction::new(STA, AbsoluteIndexedX(mem_map.read_word(arg_index)), 5),
            0x99 => Instruction::new(STA, AbsoluteIndexedY(mem_map.read_word(arg_index)), 5),
            0x81 => Instruction::new(STA, IndexedIndirectX(mem_map.read(arg_index)), 6),
            0x91 => Instruction::new(STA, IndirectIndexedY(mem_map.read(arg_index)), 6),
            // STX (STore X register)
            0x86 => Instruction::new(STX, ZeroPage(mem_map.read(arg_index)), 3),
            0x96 => Instruction::new(STX, ZeroPageIndexedY(mem_map.read(arg_index)), 4),
            0x8E => Instruction::new(STX, Absolute(mem_map.read_word(arg_index)), 4),
            // STY (STore Y register)
            0x84 => Instruction::new(STY, ZeroPage(mem_map.read(arg_index)), 3),
            0x94 => Instruction::new(STY, ZeroPageIndexedX(mem_map.read(arg_index)), 4),
            0x8C => Instruction::new(STY, Absolute(mem_map.read_word(arg_index)), 4),

            _ => Instruction::new(Unknown, Invalid, 0)
        };

        if let Unknown = instr.token {
            Err(format!("Unknown opcode: 0x{:x}", op_code))
        } else {
            Ok(instr)
        }
    }
}