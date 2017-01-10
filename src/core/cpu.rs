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
    pub decimal_mode: bool,
    // should never be set to true by a NES rom
    pub break_executed: bool,
    logical_1: bool,
    // unused
    pub overflow_flag: bool,
    pub sign_flag: bool,
    // 0 when result of operation is positive, 1 when negative
}

impl StatusReg {
    pub fn clear_carry(&mut self) {
        self.carry_flag = false;
    }

    pub fn set_carry(&mut self) {
        self.carry_flag = true;
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

        let should_advance_pc: bool = true;

        match instruction.token {
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
            _ => println!("Skipping unimplemented instruction: {}", instruction.token),
        };

        if should_advance_pc {
            self.reg_pc += instruction.addressing_mode.byte_count()
        }

        Ok(instruction.cycle_count)
    }


    fn instr_lda(&mut self, instruction: &mut Instruction, mem_map: &MemMapped) {
        self.reg_a = self.read_resolved_addr(instruction, mem_map);
    }

    fn instr_ldx(&mut self, instruction: &mut Instruction, mem_map: &MemMapped) {
        self.reg_x = self.read_resolved_addr(instruction, mem_map);
    }

    fn instr_ldy(&mut self, instruction: &mut Instruction, mem_map: &MemMapped) {
        self.reg_y = self.read_resolved_addr(instruction, mem_map);
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

    fn read_resolved_addr(&self, instruction: &mut Instruction, mem_map: &MemMapped) -> u8 {
        use core::instructions::AddressingMode::*;

        let addressing_mode = &instruction.addressing_mode;
        match *addressing_mode {
            ZeroPageIndexedX(arg) => mem_map.read(((arg + self.reg_x) % 0xFF) as u16),
            ZeroPageIndexedY(arg) => mem_map.read(((arg + self.reg_y) % 0xFF) as u16),
            AbsoluteIndexedX(arg) => {
                if ((arg & 0xFF) as u8) + self.reg_x > 0xFF {
                    instruction.cycle_count += 1;
                }

                mem_map.read(arg + self.reg_x as u16)
            },
            AbsoluteIndexedY(arg) => {
                if ((arg & 0xFF) as u8) + self.reg_y > 0xFF {
                    instruction.cycle_count += 1;
                }

                mem_map.read(arg + self.reg_y as u16)
            },
            IndexedIndirectX(arg) => mem_map.read(mem_map.read_word(((arg + self.reg_x) % 0xFF) as u16)),
            IndirectIndexedY(arg) => {
                let addr = mem_map.read_word(arg as u16) + self.reg_y as u16;
                if ((addr & 0xFF) as u8) + self.reg_y > 0xFF {
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
            ZeroPageIndexedX(arg) => mem_map.write(((arg + self.reg_x) % 0xFF) as u16, byte),
            ZeroPageIndexedY(arg) => mem_map.write(((arg + self.reg_y) % 0xFF) as u16, byte),
            AbsoluteIndexedX(arg) => {
                instruction.cycle_count += 1;

                mem_map.write(arg + self.reg_x as u16, byte);
            },
            AbsoluteIndexedY(arg) => {
                instruction.cycle_count += 1;

                mem_map.write(arg + self.reg_y as u16, byte);
            },
            IndexedIndirectX(arg) => {
                let addr = mem_map.read_word(((arg + self.reg_x) % 0xFF) as u16);
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
}

