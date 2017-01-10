// 6502
use std::default::Default;
use core::CpuFacade;
use core::memory::MemMapped;
use core::instructions::*;
use core::debugger::Debugger;

const MASTER_CLOCK_NTSC: f32 = 21.477272_E6_f32; // 21.477272 MHz
const CLOCK_DIVISOR_NTSC: i32 = 12;

const MASTER_CLOCK_PAL: f32 = 26.601712_E6_f32; // 26.601712 MHz
const CLOCK_DIVISOR_PAL: i32 = 15;

const RESET_SP: u8 = 0xFD;
const RESET_PC_VEC: u16 = 0xFFFC;

#[derive(Debug, Default)]
pub struct StatusReg {
    pub carry_flag: bool,
    pub zero_flag: bool,
    pub interrupt_disable: bool,
    // should never be set to true by a NES rom
    pub decimal_mode: bool,
    pub break_executed: bool,
    // unused
    logical_1: bool,
    pub overflow_flag: bool,
    // 0 when result of operation is positive, 1 when negative
    pub sign_flag: bool,
}

impl StatusReg {
    pub fn to_byte(&self) -> u8 {
        let mut byte = 0u8;

        byte = byte | self.sign_flag as u8;
        byte = (byte << 1) | self.overflow_flag as u8;
        byte = (byte << 1) | self.logical_1 as u8;
        byte = (byte << 1) | self.break_executed as u8;
        byte = (byte << 1) | self.decimal_mode as u8;
        byte = (byte << 1) | self.interrupt_disable as u8;
        byte = (byte << 1) | self.zero_flag as u8;
        byte = (byte << 1) | self.carry_flag as u8;

        byte
    }

    pub fn from_byte(byte: u8) -> StatusReg {
        StatusReg {
            carry_flag: byte & 0b00000001 == 1,
            zero_flag: (byte >> 1) & 0b00000001 == 1,
            interrupt_disable: (byte >> 2) & 0b00000001 == 1,
            decimal_mode: (byte >> 3) & 0b00000001 == 1,
            break_executed: (byte >> 4) & 0b00000001 == 1,
            logical_1: true,
            overflow_flag: (byte >> 6) & 0b00000001 == 1,
            sign_flag: (byte >> 7) & 0b00000001 == 1,
        }
    }

    pub fn check(&mut self, byte: u8) {
        if byte >> 7 == 1 {
            self.set_sign();
        } else {
            self.clear_sign();
        }

        if byte == 0 {
            self.set_zero();
        } else {
            self.clear_zero();
        }
    }

    pub fn clear_carry(&mut self) {
        self.carry_flag = false;
    }

    pub fn set_carry(&mut self) {
        self.carry_flag = true;
    }

    pub fn clear_zero(&mut self) {
        self.zero_flag = false;
    }

    pub fn set_zero(&mut self) {
        self.zero_flag = true;
    }

    pub fn clear_interrupt_disable(&mut self) {
        self.interrupt_disable = false;
    }

    pub fn set_interrupt_disable(&mut self) {
        self.interrupt_disable = true;
    }

    pub fn clear_overflow(&mut self) {
        self.overflow_flag = false;
    }

    pub fn set_overflow(&mut self) {
        self.overflow_flag = true;
    }

    pub fn clear_decimal(&mut self) {
        self.decimal_mode = false;
    }

    pub fn set_decimal(&mut self) {
        self.decimal_mode = true;
    }

    pub fn set_sign(&mut self) {
        self.sign_flag = true;
    }

    pub fn clear_sign(&mut self) {
        self.sign_flag = false;
    }
}

#[derive(Debug, Default)]
pub struct Cpu {
    // Registers
    pub reg_a: u8,
    // Accumulator
    pub reg_x: u8,
    // X index register
    pub reg_y: u8,
    // Y index register

    pub reg_status: StatusReg,
    // status register
    pub reg_sp: u8,
    // stack pointer register
    pub reg_pc: u16,
    // program counter register
}

impl Cpu {
    pub fn new(mem_map: &MemMapped) -> Cpu {
        let mut cpu = Cpu {
            reg_a: 0,
            reg_x: 0,
            reg_y: 0,

            reg_status: StatusReg {
                carry_flag: false,
                zero_flag: false,
                interrupt_disable: true,
                decimal_mode: false,
                break_executed: false,
                logical_1: true, // unused flag, always on
                overflow_flag: false,
                sign_flag: false,
            },

            reg_sp: RESET_SP,
            reg_pc: 0,
        };
        cpu.hard_reset(mem_map);

        cpu
    }

    pub fn hard_reset(&mut self, mem_map: &MemMapped) {
        self.reg_a = 0;
        self.reg_x = 0;
        self.reg_y = 0;

        self.reg_status = StatusReg {
            carry_flag: false,
            zero_flag: false,
            interrupt_disable: true,
            decimal_mode: false,
            break_executed: false,
            logical_1: true, // unused flag, always on
            overflow_flag: false,
            sign_flag: false,
        };

        self.reg_sp = RESET_SP;
        self.reg_pc = mem_map.read_word(RESET_PC_VEC);
    }

    pub fn soft_reset(&mut self) {}

    pub fn step(&mut self, mem_map: &mut MemMapped) -> Result<u8, String> {
        let instruction = Instruction::decode(mem_map, self.reg_pc)?;

        self.execute_instruction(instruction, mem_map)
    }

    fn execute_instruction(&mut self, mut instruction: Instruction,
                           mem_map: &mut MemMapped) -> Result<u8, String> {
        use core::instructions::InstructionToken::*;

        let mut should_advance_pc = true;

        match instruction.token {
            // JMP / JSR
            JMP => {
                should_advance_pc = false;
                self.instr_jmp(&instruction.addressing_mode, mem_map)
            },
            // Stack instructions
            TXS => self.instr_txs(),
            TSX => self.instr_tsx(),
            PHA => self.instr_pha(mem_map),
            PLA => self.instr_pla(mem_map),
            PHP => self.instr_php(mem_map),
            PLP => self.instr_plp(mem_map),
            // Flag instructions
            CLC => self.reg_status.clear_carry(),
            SEC => self.reg_status.set_carry(),
            CLI => self.reg_status.clear_interrupt_disable(),
            SEI => self.reg_status.set_interrupt_disable(),
            CLV => self.reg_status.clear_overflow(),
            CLD => self.reg_status.clear_decimal(),
            SED => self.reg_status.set_decimal(),
            // Store/Load instructions
            LDA => self.instr_lda(&mut instruction, mem_map),
            LDX => self.instr_ldx(&mut instruction, mem_map),
            LDY => self.instr_ldy(&mut instruction, mem_map),
            STA => self.instr_sta(&mut instruction, mem_map),
            STX => self.instr_stx(&mut instruction, mem_map),
            STY => self.instr_sty(&mut instruction, mem_map),
            // Register instructions
            TAX => self.instr_tax(),
            TXA => self.instr_txa(),
            DEX => self.instr_dex(),
            INX => self.instr_inx(),
            TAY => self.instr_tay(),
            TYA => self.instr_tya(),
            DEY => self.instr_dey(),
            INY => self.instr_iny(),
            _ => println!("Skipping unimplemented instruction: {}", instruction.token),
        };

        if should_advance_pc {
            self.reg_pc += instruction.addressing_mode.byte_count()
        }

        Ok(instruction.cycle_count)
    }

    //
    // Jump instructions
    //
    fn instr_jmp(&mut self, addressing_mode: &AddressingMode, mem_map: &mut MemMapped) {
        use core::instructions::AddressingMode::*;

        match *addressing_mode {
            Absolute(arg) => {
                self.reg_pc = arg;
            }
            Indirect(arg) => {
                self.reg_pc = mem_map.read_word(arg);
            }
            _ => unreachable!()
        }
    }
    //
    // Stack instructions
    //
    fn instr_txs(&mut self) {
        self.reg_sp = self.reg_x;
        self.reg_status.check(self.reg_sp);
    }

    fn instr_tsx(&mut self) {
        self.reg_x = self.reg_sp;
        self.reg_status.check(self.reg_x);
    }

    fn instr_pha(&mut self, mem_map: &mut MemMapped) {
        let reg_a = self.reg_a;
        self.stack_push(mem_map, reg_a);
    }

    fn instr_pla(&mut self, mem_map: &mut MemMapped) {
        self.reg_a = self.stack_pull(mem_map);
        self.reg_status.check(self.reg_a);
    }

    fn instr_php(&mut self, mem_map: &mut MemMapped) {
        let reg_status = self.reg_status.to_byte();
        self.stack_push(mem_map, reg_status);
    }

    fn instr_plp(&mut self, mem_map: &mut MemMapped) {
        self.reg_status = StatusReg::from_byte(self.stack_pull(mem_map));
    }
    //
    // Store/Load instructions
    //
    fn instr_lda(&mut self, instruction: &mut Instruction, mem_map: &MemMapped) {
        self.reg_a = self.read_resolved_addr(instruction, mem_map);
        self.reg_status.check(self.reg_a);
    }

    fn instr_ldx(&mut self, instruction: &mut Instruction, mem_map: &MemMapped) {
        self.reg_x = self.read_resolved_addr(instruction, mem_map);
        self.reg_status.check(self.reg_x);
    }

    fn instr_ldy(&mut self, instruction: &mut Instruction, mem_map: &MemMapped) {
        self.reg_y = self.read_resolved_addr(instruction, mem_map);
        self.reg_status.check(self.reg_y);
    }

    fn instr_sta(&self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        self.write_resolved_addr(instruction, mem_map, self.reg_a);
    }

    fn instr_stx(&self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        self.write_resolved_addr(instruction, mem_map, self.reg_x);
    }

    fn instr_sty(&self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        self.write_resolved_addr(instruction, mem_map, self.reg_y);
    }
    //
    // Register instructions
    //
    fn instr_tax(&mut self) {
        self.reg_x = self.reg_a;
        self.reg_status.check(self.reg_x);
    }

    fn instr_txa(&mut self) {
        self.reg_a = self.reg_x;
        self.reg_status.check(self.reg_a);
    }

    fn instr_dex(&mut self) {
        self.reg_x = self.reg_x.wrapping_sub(1);
        self.reg_status.check(self.reg_x);
    }

    fn instr_inx(&mut self) {
        self.reg_x = self.reg_x.wrapping_add(1);
        self.reg_status.check(self.reg_x);
    }

    fn instr_tay(&mut self) {
        self.reg_y = self.reg_a;
        self.reg_status.check(self.reg_y);
    }

    fn instr_tya(&mut self) {
        self.reg_a = self.reg_y;
        self.reg_status.check(self.reg_y);
    }

    fn instr_dey(&mut self) {
        self.reg_y = self.reg_y.wrapping_sub(1);
        self.reg_status.check(self.reg_y);
    }

    fn instr_iny(&mut self) {
        self.reg_y = self.reg_y.wrapping_add(1);
        self.reg_status.check(self.reg_y);
    }

    //
    // Helpers
    //

    fn read_resolved_addr(&self, instruction: &mut Instruction, mem_map: &MemMapped) -> u8 {
        use core::instructions::AddressingMode::*;

        let addressing_mode = &instruction.addressing_mode;
        match *addressing_mode {
            ZeroPageIndexedX(arg) => mem_map.read((arg + self.reg_x) as u16 % 0x100),
            ZeroPageIndexedY(arg) => mem_map.read((arg + self.reg_y) as u16 % 0x100),
            AbsoluteIndexedX(arg) => {
                if (arg & 0xFF) + self.reg_x as u16 > 0xFF {
                    instruction.cycle_count += 1;
                }

                mem_map.read(arg + self.reg_x as u16)
            },
            AbsoluteIndexedY(arg) => {
                if (arg & 0xFF) + self.reg_y as u16 > 0xFF {
                    instruction.cycle_count += 1;
                }

                mem_map.read(arg + self.reg_y as u16)
            },
            IndexedIndirectX(arg) => mem_map.read(mem_map.read_word((arg + self.reg_x) as u16 % 0x100)),
            IndirectIndexedY(arg) => {
                let addr = mem_map.read_word(arg as u16) + self.reg_y as u16;
                if (addr & 0xFF) + self.reg_y as u16 > 0xFF {
                    instruction.cycle_count += 1;
                }
                mem_map.read(addr)
            }

            Immediate(arg) => arg,
            Accumulator => self.reg_a,
            ZeroPage(arg) => mem_map.read(arg as u16),
            Absolute(arg) => mem_map.read(arg),

            // Implicit, Relative and Indirect addressing modes are handled
            // by the instructions themselves
            _ => unreachable!()
        }
    }

    fn write_resolved_addr(&self, instruction: &mut Instruction, mem_map: &mut MemMapped, byte: u8) {
        use core::instructions::AddressingMode::*;

        let addressing_mode = &instruction.addressing_mode;
        match *addressing_mode {
            ZeroPageIndexedX(arg) => mem_map.write(((arg + self.reg_x) as u16 % 0x100), byte),
            ZeroPageIndexedY(arg) => mem_map.write(((arg + self.reg_y) as u16 % 0x200), byte),
            AbsoluteIndexedX(arg) => {
                instruction.cycle_count += 1;

                mem_map.write(arg + self.reg_x as u16, byte);
            },
            AbsoluteIndexedY(arg) => {
                instruction.cycle_count += 1;

                mem_map.write(arg + self.reg_y as u16, byte);
            },
            IndexedIndirectX(arg) => {
                let addr = mem_map.read_word((arg + self.reg_x) as u16 % 0x100);
                mem_map.write(addr, byte)
            },
            IndirectIndexedY(arg) => {
                let addr = mem_map.read_word(arg as u16) + self.reg_y as u16;

                instruction.cycle_count += 1;

                mem_map.write(addr, byte);
            }

            ZeroPage(arg) => mem_map.write(arg as u16, byte),
            Absolute(arg) => mem_map.write(arg, byte),

            // Above covers all addresing modes for writing memory
            _ => unreachable!()
        };
    }

    fn stack_push(&mut self, mem_map: &mut MemMapped, byte: u8) {
        if self.reg_sp == 0 {
            println!("Stack overflow detected! Wrapping...");
        }

        let addr = 0x100 + self.reg_sp as u16;
        mem_map.write(addr, byte);

        self.reg_sp = self.reg_sp.wrapping_sub(1);
    }

    fn stack_pull(&mut self, mem_map: &MemMapped) -> u8 {
        if self.reg_sp == 0xFF {
            println!("Stack underflow detected! Wrapping...");
        }

        self.reg_sp = self.reg_sp.wrapping_add(1);

        let addr = 0x100 + self.reg_sp as u16;
        mem_map.read(addr)
    }
}

