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
    // unused
    pub break_executed: bool,
    logical_1: bool,
    // unused
    pub overflow_flag: bool,
    pub sign_flag: bool,
    // 0 when result of operation is positive, 1 when negative
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

        println!("{:#?}", instruction);

        Ok(instruction.cycle_count)
    }

    fn execute_instruction(&mut self, instruction: Instruction) {}
}

