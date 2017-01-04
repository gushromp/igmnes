// 6502
use std::default::Default;
use core::memory::{MemMap, MemMapped};
use core::instructions::*;

const MASTER_CLOCK_NTSC: f32 = 21.477272_E6_f32; // 21.477272 MHz
const CLOCK_DIVISOR_NTSC: i32 = 12;

const MASTER_CLOCK_PAL: f32 = 26.601712_E6_f32; // 26.601712 MHz
const CLOCK_DIVISOR_PAL: i32 = 15;

const RESET_SP: u8 = 0xFD;

#[derive(Debug, Default)]
struct StatusReg {
    carry_flag: bool,
    zero_flag: bool,
    interrupt_disable: bool,
    decimal_mode: bool, // unused
    break_executed: bool,
    logical_1: bool, // unused
    overflow_flag: bool,
    sign_flag: bool, // 0 when result of operation is positive, 1 when negative
}

pub struct Cpu {
    // Memory map
    mem_map: MemMap,

    // Registers
    reg_a: u8, // Accumulator
    reg_x: u8, // X index register
    reg_y: u8, // Y index register

    reg_status: StatusReg, // status register
    reg_sp: u8, // stack pointer register
    reg_pc: u16, // program counter register
}


impl Cpu {
    pub fn new(mem_map: MemMap) -> Cpu {
        let mut cpu = Cpu {
            mem_map: mem_map,

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
        cpu.hard_reset();

        cpu
    }

    pub fn hard_reset(&mut self) {
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
        self.reg_pc = 0;
    }

    pub fn soft_reset(&mut self) {

    }

    pub fn step() -> u8 {
        0u8
    }

    fn fetch_instruction() -> Result<Instruction, String> {
        unimplemented!()
    }
}