// 6502
use std;
use std::default::Default;
use std::fmt::{self, Display};
use core::CpuFacade;
use core::memory::MemMapped;
use core::instructions::*;
use core::debugger::Debugger;
use core::errors::CpuError;

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

impl<'a> Into<u8> for &'a StatusReg {
    fn into(self) -> u8 {
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
}

impl From<u8> for StatusReg {
    fn from(byte: u8) -> StatusReg {
        StatusReg {
            carry_flag: byte & 0b00000001 == 1,
            zero_flag: (byte >> 1) & 0b00000001 == 1,
            interrupt_disable: (byte >> 2) & 0b00000001 == 1,
            decimal_mode: (byte >> 3) & 0b00000001 == 1,
            break_executed: false,
            logical_1: true,
            overflow_flag: (byte >> 6) & 0b00000001 == 1,
            sign_flag: (byte >> 7) & 0b00000001 == 1,
        }
    }
}

impl StatusReg {
    pub fn check(&mut self, byte: u8) {
        let sign = byte >> 7 == 1;
        self.toggle_sign(sign);

        let zero = byte == 0;
        self.toggle_zero(zero);
    }

    pub fn toggle_carry(&mut self, value: bool) {
        self.carry_flag = value;
    }

    pub fn toggle_zero(&mut self, value: bool) {
        self.zero_flag = value;
    }

    pub fn toggle_interrupt_disable(&mut self, value: bool) {
        self.interrupt_disable = value;
    }

    pub fn toggle_break_executed(&mut self, value: bool) {
        self.break_executed = value;
    }

    pub fn toggle_decimal(&mut self, value: bool) {
        self.decimal_mode = value;
    }

    pub fn toggle_overflow(&mut self, value: bool) {
        self.overflow_flag = value;
    }

    pub fn toggle_sign(&mut self, value: bool) {
        self.sign_flag = value;
    }
}

#[derive(Debug, Default)]
pub struct Cpu {
    // Registers

    // Accumulator
    pub reg_a: u8,

    // X index register
    pub reg_x: u8,

    // Y index register
    pub reg_y: u8,

    // status register
    pub reg_status: StatusReg,

    // stack pointer register
    pub reg_sp: u8,

    // program counter register
    pub reg_pc: u16,
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

    pub fn step(&mut self, mem_map: &mut MemMapped) -> Result<u8, CpuError> {
        let instruction = Instruction::decode(mem_map, self.reg_pc)?;

        self.execute_instruction(instruction, mem_map)
    }

    fn execute_instruction(&mut self, mut instruction: Instruction,
                           mem_map: &mut MemMapped) -> Result<u8, CpuError> {
        use core::instructions::InstructionToken::*;

        let instruction = &mut instruction;

        match instruction.token {
            NOP => {},
            // Jump instructions
            JMP => self.instr_jmp(&instruction.addressing_mode, mem_map),
            JSR => self.instr_jsr(&instruction.addressing_mode, mem_map),
            // Return instructions
            RTS => self.instr_rts(mem_map),
            // Branch instructions
            BPL => self.instr_bpl(instruction),
            BMI => self.instr_bmi(instruction),
            BVC => self.instr_bvc(instruction),
            BVS => self.instr_bvs(instruction),
            BCC => self.instr_bcc(instruction),
            BCS => self.instr_bcs(instruction),
            BNE => self.instr_bne(instruction),
            BEQ => self.instr_beq(instruction),
            // Stack instructions
            TXS => self.instr_txs(),
            TSX => self.instr_tsx(),
            PHA => self.instr_pha(mem_map),
            PLA => self.instr_pla(mem_map),
            PHP => self.instr_php(mem_map),
            PLP => self.instr_plp(mem_map),
            // Flag instructions
            CLC => self.reg_status.toggle_carry(false),
            SEC => self.reg_status.toggle_carry(true),
            CLI => self.reg_status.toggle_interrupt_disable(false),
            SEI => self.reg_status.toggle_interrupt_disable(true),
            CLV => self.reg_status.toggle_overflow(false),
            CLD => self.reg_status.toggle_decimal(false),
            SED => self.reg_status.toggle_decimal(true),
            // Store/Load instructions
            LDA => self.instr_lda(instruction, mem_map),
            LDX => self.instr_ldx(instruction, mem_map),
            LDY => self.instr_ldy(instruction, mem_map),
            STA => self.instr_sta(instruction, mem_map),
            STX => self.instr_stx(instruction, mem_map),
            STY => self.instr_sty(instruction, mem_map),
            // Register instructions
            TAX => self.instr_tax(),
            TXA => self.instr_txa(),
            DEX => self.instr_dex(),
            INX => self.instr_inx(),
            TAY => self.instr_tay(),
            TYA => self.instr_tya(),
            DEY => self.instr_dey(),
            INY => self.instr_iny(),
            // ALU instructions
            ORA => self.instr_ora(instruction, mem_map),
            AND => self.instr_and(instruction, mem_map),
            EOR => self.instr_eor(instruction, mem_map),
            ADC => self.instr_adc(instruction, mem_map),
            SBC => self.instr_sbc(instruction, mem_map),
            CMP => self.instr_cmp(instruction, mem_map),
            CPX => self.instr_cpx(instruction, mem_map),
            CPY => self.instr_cpy(instruction, mem_map),
            BIT => self.instr_bit(instruction, mem_map),
            // Read/Modify/Write instructions
            ASL => self.instr_asl(instruction, mem_map),
            ROL => self.instr_rol(instruction, mem_map),
            LSR => self.instr_lsr(instruction, mem_map),
            ROR => self.instr_ror(instruction, mem_map),
            DEC => self.instr_dec(instruction, mem_map),
            INC => self.instr_inc(instruction, mem_map),
            _ => println!("0x{:04X}: Skipping unimplemented instruction: {}", self.reg_pc, instruction.token),
        };

        if instruction.should_advance_pc {
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
    fn instr_jsr(&mut self, addressing_mode: &AddressingMode, mem_map: &mut MemMapped) {
        use core::instructions::AddressingMode::*;

        match *addressing_mode {
            Absolute(arg) => {
                let reg_pc = self.reg_pc;
                self.stack_push_addr(mem_map, reg_pc + addressing_mode.byte_count() as u16);
                self.reg_pc = arg;
            },
            _ => unreachable!()
        }
    }
    //
    // Return instructions
    //
    fn instr_rts(&mut self, mem_map: &mut MemMapped) {
        let addr = self.stack_pull_addr(mem_map);

        self.reg_pc = addr;
    }
    //
    // Branch instructions
    //
    fn instr_bpl(&mut self, instruction: &mut Instruction) {
        if !self.reg_status.sign_flag {
            self.branch(instruction);
        }
    }

    fn instr_bmi(&mut self, instruction: &mut Instruction) {
        if self.reg_status.sign_flag {
            self.branch(instruction);
        }
    }

    fn instr_bvc(&mut self, instruction: &mut Instruction) {
        if !self.reg_status.overflow_flag {
            self.branch(instruction);
        }
    }

    fn instr_bvs(&mut self, instruction: &mut Instruction) {
        if self.reg_status.overflow_flag {
            self.branch(instruction);
        }
    }

    fn instr_bcc(&mut self, instruction: &mut Instruction) {
        if !self.reg_status.carry_flag {
            self.branch(instruction);
        }
    }

    fn instr_bcs(&mut self, instruction: &mut Instruction) {
        if self.reg_status.carry_flag {
            self.branch(instruction);
        }
    }

    fn instr_bne(&mut self, instruction: &mut Instruction) {
        if !self.reg_status.zero_flag {
            self.branch(instruction);
        }
    }

    fn instr_beq(&mut self, instruction: &mut Instruction) {
        if self.reg_status.zero_flag {
            self.branch(instruction);
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
        let reg_status_byte: u8 = {
            (&self.reg_status).into()
        };
        self.stack_push(mem_map, reg_status_byte);
    }

    fn instr_plp(&mut self, mem_map: &mut MemMapped) {
        self.reg_status = StatusReg::from(self.stack_pull(mem_map));
    }
    //
    // Store/Load instructions
    //
    fn instr_lda(&mut self, instruction: &mut Instruction, mem_map: &MemMapped) {
        self.reg_a = self.read_resolved(instruction, mem_map);
        self.reg_status.check(self.reg_a);
    }

    fn instr_ldx(&mut self, instruction: &mut Instruction, mem_map: &MemMapped) {
        self.reg_x = self.read_resolved(instruction, mem_map);
        self.reg_status.check(self.reg_x);
    }

    fn instr_ldy(&mut self, instruction: &mut Instruction, mem_map: &MemMapped) {
        self.reg_y = self.read_resolved(instruction, mem_map);
        self.reg_status.check(self.reg_y);
    }

    fn instr_sta(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let reg_a = self.reg_a;
        self.write_resolved(instruction, mem_map, reg_a);
    }

    fn instr_stx(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let reg_x = self.reg_x;
        self.write_resolved(instruction, mem_map, reg_x);
    }

    fn instr_sty(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let reg_y = self.reg_y;
        self.write_resolved(instruction, mem_map, reg_y);
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
    // ALU instructions
    //
    fn instr_ora(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let byte = self.read_resolved(instruction, mem_map);

        self.reg_a |= byte;
        self.reg_status.check(self.reg_a);
    }

    fn instr_and(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let byte = self.read_resolved(instruction, mem_map);

        self.reg_a &= byte;
        self.reg_status.check(self.reg_a);
    }

    fn instr_eor(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let byte = self.read_resolved(instruction, mem_map);

        self.reg_a ^= byte;
        self.reg_status.check(self.reg_a);
    }

    fn instr_adc(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let byte = self.read_resolved(instruction, mem_map);
        let old_carry = self.reg_status.carry_flag as u8;

        let addend = byte.wrapping_add(old_carry);
        let (result, carry) = self.reg_a.overflowing_add(addend);

        self.reg_status.toggle_carry(carry);

        // two's complement overflow check
        let overflow = (self.reg_a > 127u8 && addend > 127u8 && result <= 127u8)
            || (self.reg_a <= 127u8 && addend <= 127u8 && result > 127u8);

        self.reg_status.toggle_overflow(overflow);

        self.reg_a = result;
        self.reg_status.check(self.reg_a);
    }

    fn instr_sbc(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let byte = self.read_resolved(instruction, mem_map);
        let borrow = 1 - self.reg_status.carry_flag as u8;

        let addend = byte.wrapping_add(borrow);

        let (result, carry) = self.reg_a.overflowing_sub(addend);

        // when subtracting, 6502 sets carry if the result is NOT negative
        let reverse_carry = !(carry);
        self.reg_status.toggle_carry(reverse_carry);

        // two's complement overflow check
        let overflow = (self.reg_a > 127u8 && addend <= 127u8 && result >= 127u8)
            || (self.reg_a <= 127u8 && addend > 127u8 && result <= 127u8);

        self.reg_status.toggle_overflow(overflow);

        self.reg_a = result;
        self.reg_status.check(self.reg_a);
    }

    fn instr_cmp(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let byte = self.read_resolved(instruction, mem_map);

        if self.reg_a > byte {
            self.reg_status.toggle_carry(true);
            self.reg_status.toggle_zero(false);
        } else if self.reg_a == byte {
            self.reg_status.toggle_carry(true);
            self.reg_status.toggle_zero(true);
        } else {
            self.reg_status.toggle_carry(false);
            self.reg_status.toggle_zero(false);
        }

        let sub = self.reg_a.wrapping_sub(byte);
        let sign = sub >> 7 == 1;

        self.reg_status.toggle_sign(sign);
    }

    fn instr_cpx(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let byte = self.read_resolved(instruction, mem_map);

        if self.reg_x > byte {
            self.reg_status.toggle_carry(true);
            self.reg_status.toggle_zero(false);
        } else if self.reg_x == byte {
            self.reg_status.toggle_carry(true);
            self.reg_status.toggle_zero(true);
        } else {
            self.reg_status.toggle_carry(false);
            self.reg_status.toggle_zero(false);
        }

        let sub = self.reg_x.wrapping_sub(byte);
        let sign = sub >> 7 == 1;

        self.reg_status.toggle_sign(sign);
    }

    fn instr_cpy(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let byte = self.read_resolved(instruction, mem_map);

        if self.reg_y > byte {
            self.reg_status.toggle_carry(true);
            self.reg_status.toggle_zero(false);
        } else if self.reg_y == byte {
            self.reg_status.toggle_carry(true);
            self.reg_status.toggle_zero(true);
        } else {
            self.reg_status.toggle_carry(false);
            self.reg_status.toggle_zero(false);
        }

        let sub = self.reg_y.wrapping_sub(byte);
        let sign = sub >> 7 == 1;

        self.reg_status.toggle_sign(sign);
    }

    fn instr_bit(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let byte = self.read_resolved(instruction, mem_map);

        let zero = byte & self.reg_a == 0;
        self.reg_status.toggle_zero(zero);

        let overflow = (byte >> 6) & 0b1 == 1;
        self.reg_status.toggle_overflow(overflow);

        let sign = (byte >> 7) & 0b1 == 1;
        self.reg_status.toggle_sign(sign);
    }
    //
    // Read/Modify/Write instructions
    //
    fn instr_asl(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let mut byte = self.read_resolved(instruction, mem_map);

        let carry = (byte >> 7) == 1;
        self.reg_status.toggle_carry(carry);

        byte = byte << 1;
        self.reg_status.check(byte);

        self.write_resolved(instruction, mem_map, byte);
    }

    fn instr_rol(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let mut byte = self.read_resolved(instruction, mem_map);

        let old_carry = self.reg_status.carry_flag as i8;
        let new_carry = (byte >> 7) == 1;

        self.reg_status.toggle_carry(new_carry);

        byte = byte << 1;
        byte ^= ((-old_carry as u8) ^ byte) & 1;
        self.reg_status.check(byte);

        self.write_resolved(instruction, mem_map, byte);
    }

    fn instr_lsr(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let mut byte = self.read_resolved(instruction, mem_map);

        let carry = (byte & 1) == 1;
        self.reg_status.toggle_carry(carry);

        byte = byte >> 1;
        self.reg_status.check(byte);

        self.write_resolved(instruction, mem_map, byte);
    }

    fn instr_ror(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let mut byte = self.read_resolved(instruction, mem_map);

        let old_carry = self.reg_status.carry_flag as i8;
        let new_carry = (byte & 1) == 1;

        self.reg_status.toggle_carry(new_carry);

        byte = byte >> 1;
        byte ^= ((-old_carry as u8) ^ byte) & (1 << 7);
        self.reg_status.check(byte);

        self.write_resolved(instruction, mem_map, byte);
    }

    fn instr_dec(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let mut byte = self.read_resolved(instruction, mem_map);

        byte = byte.wrapping_sub(1);
        self.reg_status.check(byte);

        self.write_resolved(instruction, mem_map, byte);
    }

    fn instr_inc(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped) {
        let mut byte = self.read_resolved(instruction, mem_map);

        byte = byte.wrapping_add(1);
        self.reg_status.check(byte);

        self.write_resolved(instruction, mem_map, byte);
    }

    //////////////
    //
    // Helpers
    //
    //////////////
    pub fn read_resolved(&self, instruction: &mut Instruction, mem_map: &MemMapped) -> u8 {
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
            _ => 0
        }
    }

    fn write_resolved(&mut self, instruction: &mut Instruction, mem_map: &mut MemMapped, byte: u8) {
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
            Accumulator => self.reg_a = byte,
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

    fn stack_push_addr(&mut self, mem_map: &mut MemMapped, addr: u16) {
        let addr_high = ((addr & 0xFF00) >> 8) as u8;
        let addr_low = (addr & 0xFF) as u8;

        self.stack_push(mem_map, addr_low);
        self.stack_push(mem_map, addr_high);
    }

    fn stack_pull_addr(&mut self, mem_map: &mut MemMapped) -> u16 {
        let addr_high = self.stack_pull(mem_map);
        let addr_low = self.stack_pull(mem_map);

        let addr = ((addr_high as u16) << 8) | addr_low as u16;

        addr
    }

    // branch is taken
    fn branch(&mut self, instruction: &mut Instruction) {
        use core::instructions::AddressingMode::*;

        // increase cycle count by 1
        instruction.cycle_count += 1;

        // the PC will also be incremented by 2,
        // so the effective final pc address will be
        // reg_pc = reg_pc + offset + 2

        match instruction.addressing_mode {
            Relative(offset) => {
                let reg_pc_i32 = self.reg_pc as i32;
                let test = (reg_pc_i32 & 0xFF) + offset as i32;
                if test < 0 || test > 0xFF {
                    // moved to previous or next page, increase cycle count by 1
                    instruction.cycle_count += 1;
                }

                self.reg_pc = reg_pc_i32.wrapping_add(offset as i32) as u16;
            },
            _ => unreachable!()
        }
    }
}

impl Display for Cpu {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let status_reg_byte: u8 = (&self.reg_status).into();
        write!(f, "A:0x{:02X} X:0x{:02X} Y:0x{:02X} P:0x{:02X} SP:0x{:02X}",
               self.reg_a, self.reg_x, self.reg_y, status_reg_byte, self.reg_sp)
    }
}

