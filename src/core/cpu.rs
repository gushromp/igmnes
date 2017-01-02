// 2A03

use std::default::Default;

const MASTER_CLOCK_NTSC: f32 = 21.477272_E6_f32; // 21.477272 MHz
const CLOCK_DIVISOR_NTSC: i32 = 12;

const MASTER_CLOCK_PAL: f32 = 26.601712_E6_f32; // 26.601712 MHz
const CLOCK_DIVISOR_PAL: i32 = 15;

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

#[derive(Debug, Default)]
pub struct Cpu {
    // Registers
    reg_a: u8, // Accumulator
    reg_x: u8, // X index register
    reg_y: u8, // Y index register

    status: StatusReg, // status register
    sp: u8, // stack pointer register
    pc: u16, // program counter register
}

impl Cpu {
    pub fn new() -> Cpu {
        let mut cpu = Cpu::default();
        cpu.reset();

        cpu
    }

    pub fn reset(&mut self) {
        self.reg_a = 0;
        self.reg_x = 0;
        self.reg_y = 0;

        self.status = StatusReg {
            carry_flag: false,
            zero_flag: false,
            interrupt_disable: false,
            decimal_mode: false,
            break_executed: false,
            logical_1: true,
            overflow_flag: false,
            sign_flag: false,
        };

        self.sp = 0;
        self.pc = 0;
    }
}