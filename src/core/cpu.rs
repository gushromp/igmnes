// 6502
use std::default::Default;
use core::CpuFacade;
use core::memory::{MemMap, MemMapped};
use core::instructions::*;
use core::debugger::Debugger;

const MASTER_CLOCK_NTSC: f32 = 21.477272_E6_f32; // 21.477272 MHz
const CLOCK_DIVISOR_NTSC: i32 = 12;

const MASTER_CLOCK_PAL: f32 = 26.601712_E6_f32; // 26.601712 MHz
const CLOCK_DIVISOR_PAL: i32 = 15;

const RESET_SP: u8 = 0xFD;
const RESET_PC_VEC: u16 = 0xFFFC;

#[derive(Debug, Default)]
struct StatusReg {
    pub carry_flag: bool,
    pub zero_flag: bool,
    pub interrupt_disable: bool,
    pub decimal_mode: bool, // unused
    pub break_executed: bool,
    logical_1: bool, // unused
    pub overflow_flag: bool,
    pub sign_flag: bool, // 0 when result of operation is positive, 1 when negative
}

#[derive(Debug, Default)]
struct State {
    // Registers
    pub reg_a: u8, // Accumulator
    pub reg_x: u8, // X index register
    pub reg_y: u8, // Y index register

    pub reg_status: StatusReg, // status register
    pub reg_sp: u8, // stack pointer register
    pub reg_pc: u16, // program counter register
}

#[derive(Default)]
pub struct Cpu {

    // CPU State
    state: State,

    // Memory map
    mem_map: Box<MemMap>,
}


impl Cpu {
    pub fn new(mem_map: Box<MemMap>) -> Cpu {

        let state = State {
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

        let mut cpu = Cpu {
            state: state,
            mem_map: mem_map,

        };
        cpu.hard_reset();

        cpu
    }

    pub fn hard_reset(&mut self) {

        let mut state = &mut self.state;

        state.reg_a = 0;
        state.reg_x = 0;
        state.reg_y = 0;

        state.reg_status = StatusReg {
            carry_flag: false,
            zero_flag: false,
            interrupt_disable: true,
            decimal_mode: false,
            break_executed: false,
            logical_1: true, // unused flag, always on
            overflow_flag: false,
            sign_flag: false,
        };

        state.reg_sp = RESET_SP;
        state.reg_pc = self.mem_map.read_word(RESET_PC_VEC);
    }

    pub fn soft_reset(&mut self) {

    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut State {
        &mut self.state
    }

    fn step(&mut self) -> u8 {
        let instruction = self.fetch_instruction().unwrap();

        println!("{:#?}", instruction);

        instruction.cycle_count
    }

    fn fetch_instruction(&self) -> Result<Instruction, String> {
        let opcode = self.mem_map.read(self.state.reg_pc);
        Instruction::decode(opcode)
    }


    fn execute_instruction(&mut self, instruction: Instruction) {

    }

}

impl CpuFacade for Cpu {
    fn cpu(self: Box<Self>) -> Box<Cpu> {
        self
    }

    // This is the real cpu, not a debugger
    fn debugger(&mut self) -> Option<&mut Debugger> {
        None
    }

    fn step(&mut self) -> u8 {
        self.step()
    }
}