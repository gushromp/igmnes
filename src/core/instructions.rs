
use std::fmt;
use core::memory::MemMapped;
use core::errors::EmulationError;

#[derive(Debug, Clone)]
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
    Relative(i8),
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

            Invalid => 1,
        }
    }
}

#[derive(Debug, Copy, Clone)]
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

    // Unofficial opcodes
    // Read 2 bytes and IGNore them (useful for side effects)
    IGN,
    // Combined ALU/RMW
    LAX,
    SAX,
    ALR,
    ANC,
    ARR,
    AXS,
    DCP,
    ISC,
    RLA,
    RRA,
    SLO,
    SRE,

    Unknown,
}

impl fmt::Display for InstructionToken {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub op_code: u8,
    pub address: u16,
    pub token: InstructionToken,
    pub addressing_mode: AddressingMode,
    pub cycle_count: u8,
    pub should_advance_pc: bool
}

impl Instruction {
    pub fn new(token: InstructionToken, addressing_mode: AddressingMode,
               cycle_count: u8, should_advance_pc: bool) -> Instruction {
        Instruction {
            op_code: 0,
            address: 0,
            token: token,
            addressing_mode: addressing_mode,
            cycle_count: cycle_count,
            should_advance_pc: should_advance_pc
        }
    }
}

impl Instruction {
    pub fn decode(mem_map: &mut dyn MemMapped, addr: u16) -> Result<Instruction, EmulationError> {
        use self::InstructionToken::*;
        use self::AddressingMode::*;

        // Most instructions come with aaabbbcc bit form:
        //      aaa and cc bits are used to specify instruction type
        //      bbb bits are used to specify addressing mode
        // However, since a lot of instructions don't fit into this pattern,
        // we will match the opcodes one by one instead of looking at the individual bit groups.
        let op_code = mem_map.read(addr)?;
        let arg_index = addr.wrapping_add(1);

        let result: Result<Instruction, EmulationError> = match op_code {
            //
            // Control, branch, and stack instructions
            //
            0x00 => Ok(Instruction::new(BRK, Implicit, 7, false)), // BReaK
            0xEA => Ok(Instruction::new(NOP, Implicit, 2, true)), // NOP (No OPeration)
            // Jump instructions
            0x20 => Ok(Instruction::new(JSR, Absolute(mem_map.read_word(arg_index)?), 6, false)), // Jump to SubRoutine
            0x4C => Ok(Instruction::new(JMP, Absolute(mem_map.read_word(arg_index)?), 3, false)), // JuMP (absolute)
            0x6C => Ok(Instruction::new(JMP, Indirect(mem_map.read_word(arg_index)?), 5, false)), // JuMP (indirect)
            // Return instructions
            0x40 => Ok(Instruction::new(RTI, Implicit, 6, false)), // RTI (ReTurn from Interrupt)
            0x60 => Ok(Instruction::new(RTS, Implicit, 6, false)), // RTS (ReTurn from Subroutine)
            // Branch instructions
            0x10 => Ok(Instruction::new(BPL, Relative(mem_map.read(arg_index)? as i8), 2, true)), // Branch on PLus
            0x30 => Ok(Instruction::new(BMI, Relative(mem_map.read(arg_index)? as i8), 2, true)), // Branch on MInus
            0x50 => Ok(Instruction::new(BVC, Relative(mem_map.read(arg_index)? as i8), 2, true)), // Branch on oVerflow Clear
            0x70 => Ok(Instruction::new(BVS, Relative(mem_map.read(arg_index)? as i8), 2, true)), // Branch on oVerflow Set
            0x90 => Ok(Instruction::new(BCC, Relative(mem_map.read(arg_index)? as i8), 2, true)), // Branch on Carry Clear
            0xB0 => Ok(Instruction::new(BCS, Relative(mem_map.read(arg_index)? as i8), 2, true)), // Branch on Carry Set
            0xD0 => Ok(Instruction::new(BNE, Relative(mem_map.read(arg_index)? as i8), 2, true)), // Branch on Not Equal
            0xF0 => Ok(Instruction::new(BEQ, Relative(mem_map.read(arg_index)? as i8), 2, true)), // Branch on EQual
            // Stack instructions
            0x9A => Ok(Instruction::new(TXS, Implicit, 2, true)), // PusH Processor status
            0xBA => Ok(Instruction::new(TSX, Implicit, 2, true)), // PuLl Processor status
            0x48 => Ok(Instruction::new(PHA, Implicit, 3, true)), // PusH Accumulator
            0x68 => Ok(Instruction::new(PLA, Implicit, 4, true)), // PuLl Accumulator
            0x08 => Ok(Instruction::new(PHP, Implicit, 3, true)), // Transfer X to Stack ptr
            0x28 => Ok(Instruction::new(PLP, Implicit, 4, true)), // Transfer Stack ptr to X
            // Flag instructions
            0x18 => Ok(Instruction::new(CLC, Implicit, 2, true)), // CLear Carry
            0x38 => Ok(Instruction::new(SEC, Implicit, 2, true)), // SEt Carry
            0x58 => Ok(Instruction::new(CLI, Implicit, 2, true)), // CLear Interrupt
            0x78 => Ok(Instruction::new(SEI, Implicit, 2, true)), // SEt Interrupt
            0xB8 => Ok(Instruction::new(CLV, Implicit, 2, true)), // CLear oVerflow
            0xD8 => Ok(Instruction::new(CLD, Implicit, 2, true)), // CLear Decimal
            0xF8 => Ok(Instruction::new(SED, Implicit, 2, true)), // SEt Decimal
            //
            // ALU instructions
            //
            // ORA (bitwise OR with Accumulator)
            0x09 => Ok(Instruction::new(ORA, Immediate(mem_map.read(arg_index)?), 2, true)),
            0x05 => Ok(Instruction::new(ORA, ZeroPage(mem_map.read(arg_index)?), 3, true)),
            0x15 => Ok(Instruction::new(ORA, ZeroPageIndexedX(mem_map.read(arg_index)?), 4, true)),
            0x0D => Ok(Instruction::new(ORA, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            0x1D => Ok(Instruction::new(ORA, AbsoluteIndexedX(mem_map.read_word(arg_index)?), 4, true)),
            0x19 => Ok(Instruction::new(ORA, AbsoluteIndexedY(mem_map.read_word(arg_index)?), 4, true)),
            0x01 => Ok(Instruction::new(ORA, IndexedIndirectX(mem_map.read(arg_index)?), 6, true)),
            0x11 => Ok(Instruction::new(ORA, IndirectIndexedY(mem_map.read(arg_index)?), 5, true)),
            // AND (bitwise AND with accumulator)
            0x29 => Ok(Instruction::new(AND, Immediate(mem_map.read(arg_index)?), 2, true)),
            0x25 => Ok(Instruction::new(AND, ZeroPage(mem_map.read(arg_index)?), 3, true)),
            0x35 => Ok(Instruction::new(AND, ZeroPageIndexedX(mem_map.read(arg_index)?), 4, true)),
            0x2D => Ok(Instruction::new(AND, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            0x3D => Ok(Instruction::new(AND, AbsoluteIndexedX(mem_map.read_word(arg_index)?), 4, true)),
            0x39 => Ok(Instruction::new(AND, AbsoluteIndexedY(mem_map.read_word(arg_index)?), 4, true)),
            0x21 => Ok(Instruction::new(AND, IndexedIndirectX(mem_map.read(arg_index)?), 6, true)),
            0x31 => Ok(Instruction::new(AND, IndirectIndexedY(mem_map.read(arg_index)?), 5, true)),
            // EOR (bitwise Exclusive OR)
            0x49 => Ok(Instruction::new(EOR, Immediate(mem_map.read(arg_index)?), 2, true)),
            0x45 => Ok(Instruction::new(EOR, ZeroPage(mem_map.read(arg_index)?), 3, true)),
            0x55 => Ok(Instruction::new(EOR, ZeroPageIndexedX(mem_map.read(arg_index)?), 4, true)),
            0x4D => Ok(Instruction::new(EOR, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            0x5D => Ok(Instruction::new(EOR, AbsoluteIndexedX(mem_map.read_word(arg_index)?), 4, true)),
            0x59 => Ok(Instruction::new(EOR, AbsoluteIndexedY(mem_map.read_word(arg_index)?), 4, true)),
            0x41 => Ok(Instruction::new(EOR, IndexedIndirectX(mem_map.read(arg_index)?), 6, true)),
            0x51 => Ok(Instruction::new(EOR, IndirectIndexedY(mem_map.read(arg_index)?), 5, true)),
            // ADC (ADd with Carry)
            0x69 => Ok(Instruction::new(ADC, Immediate(mem_map.read(arg_index)?), 2, true)),
            0x65 => Ok(Instruction::new(ADC, ZeroPage(mem_map.read(arg_index)?), 3, true)),
            0x75 => Ok(Instruction::new(ADC, ZeroPageIndexedX(mem_map.read(arg_index)?), 4, true)),
            0x6D => Ok(Instruction::new(ADC, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            0x7D => Ok(Instruction::new(ADC, AbsoluteIndexedX(mem_map.read_word(arg_index)?), 4, true)),
            0x79 => Ok(Instruction::new(ADC, AbsoluteIndexedY(mem_map.read_word(arg_index)?), 4, true)),
            0x61 => Ok(Instruction::new(ADC, IndexedIndirectX(mem_map.read(arg_index)?), 6, true)),
            0x71 => Ok(Instruction::new(ADC, IndirectIndexedY(mem_map.read(arg_index)?), 5, true)),
            // CMP (CoMPare accumulator)
            0xC9 => Ok(Instruction::new(CMP, Immediate(mem_map.read(arg_index)?), 2, true)),
            0xC5 => Ok(Instruction::new(CMP, ZeroPage(mem_map.read(arg_index)?), 3, true)),
            0xD5 => Ok(Instruction::new(CMP, ZeroPageIndexedX(mem_map.read(arg_index)?), 4, true)),
            0xCD => Ok(Instruction::new(CMP, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            0xDD => Ok(Instruction::new(CMP, AbsoluteIndexedX(mem_map.read_word(arg_index)?), 4, true)),
            0xD9 => Ok(Instruction::new(CMP, AbsoluteIndexedY(mem_map.read_word(arg_index)?), 4, true)),
            0xC1 => Ok(Instruction::new(CMP, IndexedIndirectX(mem_map.read(arg_index)?), 6, true)),
            0xD1 => Ok(Instruction::new(CMP, IndirectIndexedY(mem_map.read(arg_index)?), 5, true)),
            // SBC (SuBtract with Carry)
            0xE9 => Ok(Instruction::new(SBC, Immediate(mem_map.read(arg_index)?), 2, true)),
            0xE5 => Ok(Instruction::new(SBC, ZeroPage(mem_map.read(arg_index)?), 3, true)),
            0xF5 => Ok(Instruction::new(SBC, ZeroPageIndexedX(mem_map.read(arg_index)?), 4, true)),
            0xED => Ok(Instruction::new(SBC, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            0xFD => Ok(Instruction::new(SBC, AbsoluteIndexedX(mem_map.read_word(arg_index)?), 4, true)),
            0xF9 => Ok(Instruction::new(SBC, AbsoluteIndexedY(mem_map.read_word(arg_index)?), 4, true)),
            0xE1 => Ok(Instruction::new(SBC, IndexedIndirectX(mem_map.read(arg_index)?), 6, true)),
            0xF1 => Ok(Instruction::new(SBC, IndirectIndexedY(mem_map.read(arg_index)?), 5, true)),
            // CPX (ComPare X register)
            0xE0 => Ok(Instruction::new(CPX, Immediate(mem_map.read(arg_index)?), 2, true)),
            0xE4 => Ok(Instruction::new(CPX, ZeroPage(mem_map.read(arg_index)?), 3, true)),
            0xEC => Ok(Instruction::new(CPX, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            // CPY (ComPare Y register)
            0xC0 => Ok(Instruction::new(CPY, Immediate(mem_map.read(arg_index)?), 2, true)),
            0xC4 => Ok(Instruction::new(CPY, ZeroPage(mem_map.read(arg_index)?), 3, true)),
            0xCC => Ok(Instruction::new(CPY, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            // BIT (test BITs)
            0x24 => Ok(Instruction::new(BIT, ZeroPage(mem_map.read(arg_index)?), 3, true)),
            0x2C => Ok(Instruction::new(BIT, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            //
            // Read/Modify/Write instructions
            //
            // ASL (Arithmetic Shift Left)
            0x0A => Ok(Instruction::new(ASL, Accumulator, 2, true)),
            0x06 => Ok(Instruction::new(ASL, ZeroPage(mem_map.read(arg_index)?), 5, true)),
            0x16 => Ok(Instruction::new(ASL, ZeroPageIndexedX(mem_map.read(arg_index)?), 6, true)),
            0x0E => Ok(Instruction::new(ASL, Absolute(mem_map.read_word(arg_index)?), 6, true)),
            0x1E => Ok(Instruction::new(ASL, AbsoluteIndexedX(mem_map.read_word(arg_index)?), 7, true)),
            // ROL (ROtate Left)
            0x2A => Ok(Instruction::new(ROL, Accumulator, 2, true)),
            0x26 => Ok(Instruction::new(ROL, ZeroPage(mem_map.read(arg_index)?), 5, true)),
            0x36 => Ok(Instruction::new(ROL, ZeroPageIndexedX(mem_map.read(arg_index)?), 6, true)),
            0x2E => Ok(Instruction::new(ROL, Absolute(mem_map.read_word(arg_index)?), 6, true)),
            0x3E => Ok(Instruction::new(ROL, AbsoluteIndexedX(mem_map.read_word(arg_index)?), 7, true)),
            // LSR (Logical Shift Right)
            0x4A => Ok(Instruction::new(LSR, Accumulator, 2, true)),
            0x46 => Ok(Instruction::new(LSR, ZeroPage(mem_map.read(arg_index)?), 5, true)),
            0x56 => Ok(Instruction::new(LSR, ZeroPageIndexedX(mem_map.read(arg_index)?), 6, true)),
            0x4E => Ok(Instruction::new(LSR, Absolute(mem_map.read_word(arg_index)?), 6, true)),
            0x5E => Ok(Instruction::new(LSR, AbsoluteIndexedX(mem_map.read_word(arg_index)?), 7, true)),
            // ROR (ROtate Right)
            0x6A => Ok(Instruction::new(ROR, Accumulator, 2, true)),
            0x66 => Ok(Instruction::new(ROR, ZeroPage(mem_map.read(arg_index)?), 5, true)),
            0x76 => Ok(Instruction::new(ROR, ZeroPageIndexedX(mem_map.read(arg_index)?), 6, true)),
            0x6E => Ok(Instruction::new(ROR, Absolute(mem_map.read_word(arg_index)?), 6, true)),
            0x7E => Ok(Instruction::new(ROR, AbsoluteIndexedX(mem_map.read_word(arg_index)?), 7, true)),
            // DEC (DECrement memory)
            0xC6 => Ok(Instruction::new(DEC, ZeroPage(mem_map.read(arg_index)?), 5, true)),
            0xD6 => Ok(Instruction::new(DEC, ZeroPageIndexedX(mem_map.read(arg_index)?), 6, true)),
            0xCE => Ok(Instruction::new(DEC, Absolute(mem_map.read_word(arg_index)?), 6, true)),
            0xDE => Ok(Instruction::new(DEC, AbsoluteIndexedX(mem_map.read_word(arg_index)?), 7, true)),
            // INC (INCrement memory)
            0xE6 => Ok(Instruction::new(INC, ZeroPage(mem_map.read(arg_index)?), 5, true)),
            0xF6 => Ok(Instruction::new(INC, ZeroPageIndexedX(mem_map.read(arg_index)?), 6, true)),
            0xEE => Ok(Instruction::new(INC, Absolute(mem_map.read_word(arg_index)?), 6, true)),
            0xFE => Ok(Instruction::new(INC, AbsoluteIndexedX(mem_map.read_word(arg_index)?), 7, true)),
            // Register instructions
            0xAA => Ok(Instruction::new(TAX, Implicit, 2, true)), // Transfer A to X
            0x8A => Ok(Instruction::new(TXA, Implicit, 2, true)), // Transfer X to A
            0xCA => Ok(Instruction::new(DEX, Implicit, 2, true)), // DEcrement X
            0xE8 => Ok(Instruction::new(INX, Implicit, 2, true)), // INcrement X
            0xA8 => Ok(Instruction::new(TAY, Implicit, 2, true)), // Transfer A to Y
            0x98 => Ok(Instruction::new(TYA, Implicit, 2, true)), // Transfer Y to A
            0x88 => Ok(Instruction::new(DEY, Implicit, 2, true)), // DEcrement Y
            0xC8 => Ok(Instruction::new(INY, Implicit, 2, true)), // INcrement Y
            //
            // Store/Load instructions
            //
            // LDA (LoaD Accumulator)
            0xA9 => Ok(Instruction::new(LDA, Immediate(mem_map.read(arg_index)?), 2, true)),
            0xA5 => Ok(Instruction::new(LDA, ZeroPage(mem_map.read(arg_index)?), 3, true)),
            0xB5 => Ok(Instruction::new(LDA, ZeroPageIndexedX(mem_map.read(arg_index)?), 4, true)),
            0xAD => Ok(Instruction::new(LDA, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            0xBD => Ok(Instruction::new(LDA, AbsoluteIndexedX(mem_map.read_word(arg_index)?), 4, true)),
            0xB9 => Ok(Instruction::new(LDA, AbsoluteIndexedY(mem_map.read_word(arg_index)?), 4, true)),
            0xA1 => Ok(Instruction::new(LDA, IndexedIndirectX(mem_map.read(arg_index)?), 6, true)),
            0xB1 => Ok(Instruction::new(LDA, IndirectIndexedY(mem_map.read(arg_index)?), 5, true)),
            // LDX (LoaD X register)
            0xA2 => Ok(Instruction::new(LDX, Immediate(mem_map.read(arg_index)?), 2, true)),
            0xA6 => Ok(Instruction::new(LDX, ZeroPage(mem_map.read(arg_index)?), 3, true)),
            0xB6 => Ok(Instruction::new(LDX, ZeroPageIndexedY(mem_map.read(arg_index)?), 4, true)),
            0xAE => Ok(Instruction::new(LDX, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            0xBE => Ok(Instruction::new(LDX, AbsoluteIndexedY(mem_map.read_word(arg_index)?), 4, true)),
            // LDY (LoaD Y register)
            0xA0 => Ok(Instruction::new(LDY, Immediate(mem_map.read(arg_index)?), 2, true)),
            0xA4 => Ok(Instruction::new(LDY, ZeroPage(mem_map.read(arg_index)?), 3, true)),
            0xB4 => Ok(Instruction::new(LDY, ZeroPageIndexedX(mem_map.read(arg_index)?), 4, true)),
            0xAC => Ok(Instruction::new(LDY, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            0xBC => Ok(Instruction::new(LDY, AbsoluteIndexedX(mem_map.read_word(arg_index)?), 4, true)),
            // STA (STore Accumulator)
            0x85 => Ok(Instruction::new(STA, ZeroPage(mem_map.read(arg_index)?), 3, true)),
            0x95 => Ok(Instruction::new(STA, ZeroPageIndexedX(mem_map.read(arg_index)?), 4, true)),
            0x8D => Ok(Instruction::new(STA, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            0x9D => Ok(Instruction::new(STA, AbsoluteIndexedX(mem_map.read_word(arg_index)?), 5, true)),
            0x99 => Ok(Instruction::new(STA, AbsoluteIndexedY(mem_map.read_word(arg_index)?), 5, true)),
            0x81 => Ok(Instruction::new(STA, IndexedIndirectX(mem_map.read(arg_index)?), 6, true)),
            0x91 => Ok(Instruction::new(STA, IndirectIndexedY(mem_map.read(arg_index)?), 6, true)),
            // STX (STore X register)
            0x86 => Ok(Instruction::new(STX, ZeroPage(mem_map.read(arg_index)?), 3, true)),
            0x96 => Ok(Instruction::new(STX, ZeroPageIndexedY(mem_map.read(arg_index)?), 4, true)),
            0x8E => Ok(Instruction::new(STX, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            // STY (STore Y register)
            0x84 => Ok(Instruction::new(STY, ZeroPage(mem_map.read(arg_index)?), 3, true)),
            0x94 => Ok(Instruction::new(STY, ZeroPageIndexedX(mem_map.read(arg_index)?), 4, true)),
            0x8C => Ok(Instruction::new(STY, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            //
            // Unofficial opcodes
            //
            // 1-byte NOPs
            0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xFA => Ok(Instruction::new(NOP, Implicit, 2, true)),
            // 2-byte NOPs
            0x04 | 0x14 | 0x34 | 0x44 | 0x54 | 0x64 | 0x74 | 0x80 | 0x82 | 0x89 | 0xC2 | 0xE2 | 0xEB | 0xD4 | 0xF4 => Ok(Instruction::new(NOP, Immediate(mem_map.read(arg_index)?), 2, true)),
            // 3-byte NOPs
            0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC => Ok(Instruction::new(NOP, Absolute(mem_map.read_word(arg_index)?), 2, true)),
            // IGNore
            0x0C => Ok(Instruction::new(IGN, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            // ALU/RMW combination instructions
            0xAF => Ok(Instruction::new(LAX, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            0x8F => Ok(Instruction::new(SAX, Absolute(mem_map.read_word(arg_index)?), 4, true)),
            0x4B => Ok(Instruction::new(ALR, Immediate(mem_map.read(arg_index)?), 2, true)),
            0x0B | 0x2B => Ok(Instruction::new(ANC, Immediate(mem_map.read(arg_index)?), 2, true)),
            0x6B => Ok(Instruction::new(ARR, Immediate(mem_map.read(arg_index)?), 2, true)),
            0xCB => Ok(Instruction::new(AXS, Immediate(mem_map.read(arg_index)?), 2, true)),
            0xCF => Ok(Instruction::new(DCP, Absolute(mem_map.read_word(arg_index)?), 6, true)),
            0xEF => Ok(Instruction::new(ISC, Absolute(mem_map.read_word(arg_index)?), 6, true)),
            0x2F => Ok(Instruction::new(RLA, Absolute(mem_map.read_word(arg_index)?), 6, true)),
            0x6F => Ok(Instruction::new(RRA, Absolute(mem_map.read_word(arg_index)?), 6, true)),
            0x0F => Ok(Instruction::new(SLO, Absolute(mem_map.read_word(arg_index)?), 6, true)),
            0x4F => Ok(Instruction::new(SRE, Absolute(mem_map.read_word(arg_index)?), 6, true)),
            _ => Ok(Instruction::new(Unknown, Invalid, 0, true))
        };

        match result {
            Ok(mut instr) => {
                instr.op_code = op_code;
                instr.address = addr;
                if let Unknown = instr.token {
                    Err(EmulationError::InstructionDecoding(addr, op_code))
                } else {
                    Ok(instr)
                }
            }
            Err(e) => Err(e)
        }
    }
}