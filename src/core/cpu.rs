// 6502

use core::instructions::*;
use core::memory::MemMapped;
use std::default::Default;
use std::fmt::{self, Display};

use core::errors::EmulationError;

use core::debug::{Tracer};

const RESET_PC_VEC: u16 = 0xFFFC;
const NMI_PC_VEC: u16 = 0xFFFA;
const BRK_PC_VEC: u16 = 0xFFFE;
const RESET_SP: u8 = 0xFD;

#[derive(Debug, Default, Copy, Clone)]
pub struct StatusReg {
    pub carry_flag: bool,
    pub zero_flag: bool,
    pub interrupt_disable: bool,
    pub decimal_mode: bool,
    pub break_executed: bool,
    // unused
    logical_1: bool,
    pub overflow_flag: bool,
    // 0 when result of operation is positive, 1 when negative
    pub sign_flag: bool,
}

impl StatusReg {
    fn byte(&self) -> u8 {
        let mut byte = 0u8;

        byte = byte | self.sign_flag as u8;
        byte = (byte << 1) | self.overflow_flag as u8;
        byte = (byte << 1) | 1;
        byte = (byte << 1) | self.break_executed as u8;
        byte = (byte << 1) | self.decimal_mode as u8;
        byte = (byte << 1) | self.interrupt_disable as u8;
        byte = (byte << 1) | self.zero_flag as u8;
        byte = (byte << 1) | self.carry_flag as u8;

        byte
    }

    fn irq(&self) -> u8 {
        let mut byte = 0u8;

        byte = byte | self.sign_flag as u8;
        byte = (byte << 1) | self.overflow_flag as u8;
        byte = (byte << 1) | 1;
        byte = (byte << 1) | 0;
        byte = (byte << 1) | self.decimal_mode as u8;
        byte = (byte << 1) | self.interrupt_disable as u8;
        byte = (byte << 1) | self.zero_flag as u8;
        byte = (byte << 1) | self.carry_flag as u8;

        byte
    }

    fn php(&self) -> u8 {
        let mut byte = 0u8;

        byte = byte | self.sign_flag as u8;
        byte = (byte << 1) | self.overflow_flag as u8;
        byte = (byte << 1) | 1;
        byte = (byte << 1) | 1;
        byte = (byte << 1) | self.decimal_mode as u8;
        byte = (byte << 1) | self.interrupt_disable as u8;
        byte = (byte << 1) | self.zero_flag as u8;
        byte = (byte << 1) | self.carry_flag as u8;

        byte
    }

    fn plp(&mut self, byte: u8) {
        self.carry_flag = byte & 0b_0000_0001 != 0;
        self.zero_flag = byte & 0b_0000_0010 != 0;
        self.interrupt_disable = byte & 0b_0000_0100 != 0;
        self.decimal_mode = byte & 0b_0000_1000 != 0;
        //self.break_executed = byte & 0b_0001_0000 != 0;
        self.logical_1 = true;
        self.overflow_flag = byte & 0b_0100_0000 != 0;
        self.sign_flag = byte & 0b_1000_0000 != 0;
    }

    pub fn toggle_zero_sign(&mut self, byte: u8) {
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

#[derive(Default, Copy, Clone)]
struct CpuInterrupt {
    is_hardware: bool,
    is_nmi: bool
}

#[derive(Default, Copy, Clone)]
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

    // Cycle count
    pub cycle_count: u64,

    unhandled_interrupt: Option<CpuInterrupt>,
    pending_interrupt: Option<CpuInterrupt>,
    instructions_since_last_interrupt: u64,

    is_halt_scheduled: bool,
    is_halted: bool,
}

impl Cpu {
    pub fn new(mem_map: &mut impl MemMapped) -> Cpu {
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

            cycle_count: 0,
            unhandled_interrupt: None,
            pending_interrupt: None,
            instructions_since_last_interrupt: 0,
            is_halt_scheduled: false,
            is_halted: false
        };
        cpu.hard_reset(mem_map);

        cpu
    }

    #[inline]
    pub fn hard_reset(&mut self, mem_map: &mut impl MemMapped) {
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
        self.reg_pc = mem_map.read_word(RESET_PC_VEC).unwrap();

        self.cycle_count = 7;
    }

    #[inline]
    pub fn soft_reset(&mut self) {}

    #[inline]
    pub fn irq(&mut self, mem_map: &mut impl MemMapped) -> Result<(), EmulationError> {
        let interrupt = CpuInterrupt { is_hardware: true, is_nmi: false};
        self.interrupt(mem_map, interrupt)
    }

    #[inline]
    pub fn nmi(&mut self, mem_map: &mut impl MemMapped) -> Result<(), EmulationError> {
        let interrupt = CpuInterrupt { is_hardware: true, is_nmi: true};
        self.interrupt(mem_map, interrupt)
    }

    #[inline]
    fn interrupt(&mut self, mem_map: &mut impl MemMapped, interrupt: CpuInterrupt) -> Result<(), EmulationError> {
        if !self.pending_interrupt.is_none() {
            return Ok(());
        }

        self.instructions_since_last_interrupt = 0;
        if interrupt.is_nmi {
            self.pending_interrupt = Some(interrupt);
            Ok(())
        }
        else {
            if !self.reg_status.interrupt_disable {
                self.perform_irq(mem_map, &interrupt)
            } else {
                self.unhandled_interrupt = Some(interrupt);
                Ok(())
            }
        }
    }

    pub fn dma(&mut self) {
        if !self.is_halted {
            self.is_halt_scheduled = true;
        }
    }

    #[inline]
    pub fn step(&mut self, mem_map: &mut impl MemMapped, tracer: &mut Tracer) -> Result<u8, EmulationError> {
        if self.is_halted {
            self.cycle_count += 2;
            self.is_halted = false;
            return Ok(2);
        }

        let result;

        if let Some(interrupt) = self.pending_interrupt {
            if (interrupt.is_nmi && self.instructions_since_last_interrupt > 0) || (self.instructions_since_last_interrupt > 1) {
                if tracer.is_enabled() {
                    tracer.add_cpu_trace(&self, mem_map);
                }
                self.perform_irq(mem_map, &interrupt)?;
                self.pending_interrupt = None;

                result = Ok(7);
            } else {
                result = self.execute_next_instruction(mem_map, tracer);
                self.instructions_since_last_interrupt += 1;
            }
        } else {
            result = self.execute_next_instruction(mem_map, tracer);
            self.instructions_since_last_interrupt += 1;
        }

        if self.is_halt_scheduled {
            self.is_halted = true;
            self.is_halt_scheduled = false;
        }

        self.unhandled_interrupt = None;

        result
    }

    fn execute_next_instruction(&mut self, mem_map: &mut impl MemMapped, tracer: &mut Tracer) -> Result<u8, EmulationError> {
        if tracer.is_enabled() {
            tracer.add_cpu_trace(&self, mem_map);
        }

        let instruction = Instruction::decode(mem_map, self.reg_pc);
        let result = match instruction {
            Ok(mut instr) => {
                match self.execute_instruction(&mut instr, mem_map) {
                    Ok(cycles) => {
                        self.cycle_count += cycles as u64;
                        Ok(cycles)
                    }
                    Err(e) => Err(e),
                }
            }
            Err(e) => {
                self.reg_pc = self.reg_pc.wrapping_add(2);
                Err(e)
            }
        };

        result
    }

    fn execute_instruction(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<u8, EmulationError> {
        use core::instructions::InstructionToken::*;

        let result = match instruction.token {
            NOP => self.instr_nop(instruction, mem_map),
            // Jump instructions
            JMP => self.instr_jmp(instruction, mem_map),
            JSR => self.instr_jsr(instruction, mem_map),
            // Break/Return instructions
            BRK => self.instr_brk(mem_map),
            RTI => self.instr_rti(mem_map),
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
            CLC => self.instr_clc(),
            SEC => self.instr_sec(),
            CLI => self.instr_cli(),
            SEI => self.instr_sei(),
            CLV => self.instr_clv(),
            CLD => self.instr_cld(),
            SED => self.instr_sed(),
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
            _ => {
                instruction.should_advance_pc = true;
                println!(
                    "0x{:04X}: Skipping unimplemented instruction: {}",
                    self.reg_pc, instruction.token
                );
                Ok(())
            }
        };

        match result {
            Ok(()) => {
                if instruction.should_advance_pc {
                    self.reg_pc =
                        self.reg_pc
                            .wrapping_add(instruction.addressing_mode.byte_count());
                }

                Ok(instruction.cycle_count)
            }
            Err(e) => {
                //self.reg_pc = self.reg_pc.wrapping_add(instruction.addressing_mode.byte_count());

                Err(e)
            }
        }
    }

    //
// NOP
//
    #[inline]
    fn instr_nop(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let _ = self.read_resolved(instruction, mem_map)?;
        Ok(())
    }

    //
// Jump instructions
//
    #[inline]
    fn instr_jmp(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        use core::instructions::AddressingMode::*;

        let addressing_mode = &instruction.addressing_mode;

        match *addressing_mode {
            Absolute(arg) => {
                self.reg_pc = arg;
            }
            Indirect(arg) => {
                // Indirect addressing wraps around a single 0x100-byte page
                // so for example JMP ($01FF) reads the low byte from $01FF
                // and the high byte from $0100

                // We could move this behavior to the read_word trait
                // but we keep it localized to indirect addressing
                // for performance reasons (because this effect can only
                // happen with indirect addressing)

                let addr_high = arg >> 8;
                let addr_low_1 = (arg & 0xFF) as u8;
                let addr_low_2 = addr_low_1.wrapping_add(1);

                let resolved_low = (addr_high << 8) | addr_low_1 as u16;
                let resolved_high = (addr_high << 8) | addr_low_2 as u16;

                let target_addr_low = mem_map.read(resolved_low)?;
                let target_addr_high = mem_map.read(resolved_high)?;

                let target_addr = ((target_addr_high as u16) << 8) | target_addr_low as u16;

                self.reg_pc = target_addr;
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    #[inline]
    fn instr_jsr(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        use core::instructions::AddressingMode::*;

        let addressing_mode = &instruction.addressing_mode;

        match *addressing_mode {
            Absolute(arg) => {
                let reg_pc = self.reg_pc;

                // note the -1
                let return_destination = reg_pc + addressing_mode.byte_count() - 1;

                self.stack_push_addr(mem_map, return_destination)?;
                self.reg_pc = arg;
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    //
// Break/Return instructions
//
    #[inline]
    fn instr_brk(&mut self, mem_map: &mut impl MemMapped) -> Result<(), EmulationError> {
        let interrupt = CpuInterrupt { is_hardware: false, is_nmi: false };
        self.perform_irq(mem_map, &interrupt)?;

        Ok(())
    }

    #[inline]
    fn instr_rti(&mut self, mem_map: &mut impl MemMapped) -> Result<(), EmulationError> {
        if self.pending_interrupt.is_some() {
            self.pending_interrupt = None;
        }

        let status_byte = self.stack_pull(mem_map)?;
        let new_pc = self.stack_pull_addr(mem_map)?;

        self.reg_status.plp(status_byte);
        self.reg_pc = new_pc;

        Ok(())
    }

    #[inline]
    fn instr_rts(&mut self, mem_map: &mut impl MemMapped) -> Result<(), EmulationError> {
        let mut addr = self.stack_pull_addr(mem_map)?;

        addr += 1;
        self.reg_pc = addr;

        Ok(())
    }

    //
// Branch instructions
//
    #[inline]
    fn instr_bpl(&mut self, instruction: &mut Instruction) -> Result<(), EmulationError> {
        if !self.reg_status.sign_flag {
            self.branch(instruction);
        }

        Ok(())
    }

    #[inline]
    fn instr_bmi(&mut self, instruction: &mut Instruction) -> Result<(), EmulationError> {
        if self.reg_status.sign_flag {
            self.branch(instruction);
        }

        Ok(())
    }

    #[inline]
    fn instr_bvc(&mut self, instruction: &mut Instruction) -> Result<(), EmulationError> {
        if !self.reg_status.overflow_flag {
            self.branch(instruction);
        }

        Ok(())
    }

    #[inline]
    fn instr_bvs(&mut self, instruction: &mut Instruction) -> Result<(), EmulationError> {
        if self.reg_status.overflow_flag {
            self.branch(instruction);
        }

        Ok(())
    }

    #[inline]
    fn instr_bcc(&mut self, instruction: &mut Instruction) -> Result<(), EmulationError> {
        if !self.reg_status.carry_flag {
            self.branch(instruction);
        }

        Ok(())
    }

    #[inline]
    fn instr_bcs(&mut self, instruction: &mut Instruction) -> Result<(), EmulationError> {
        if self.reg_status.carry_flag {
            self.branch(instruction);
        }

        Ok(())
    }

    #[inline]
    fn instr_bne(&mut self, instruction: &mut Instruction) -> Result<(), EmulationError> {
        if !self.reg_status.zero_flag {
            self.branch(instruction);
        }

        Ok(())
    }

    #[inline]
    fn instr_beq(&mut self, instruction: &mut Instruction) -> Result<(), EmulationError> {
        if self.reg_status.zero_flag {
            self.branch(instruction);
        }

        Ok(())
    }

    //
// Stack instructions
//
    #[inline]
    fn instr_txs(&mut self) -> Result<(), EmulationError> {
        self.reg_sp = self.reg_x;

        Ok(())
    }

    #[inline]
    fn instr_tsx(&mut self) -> Result<(), EmulationError> {
        self.reg_x = self.reg_sp;
        self.reg_status.toggle_zero_sign(self.reg_x);

        Ok(())
    }

    #[inline]
    fn instr_pha(&mut self, mem_map: &mut impl MemMapped) -> Result<(), EmulationError> {
        let reg_a = self.reg_a;
        self.stack_push(mem_map, reg_a)?;

        Ok(())
    }

    #[inline]
    fn instr_pla(&mut self, mem_map: &mut impl MemMapped) -> Result<(), EmulationError> {
        self.reg_a = self.stack_pull(mem_map)?;
        self.reg_status.toggle_zero_sign(self.reg_a);

        Ok(())
    }

    #[inline]
    fn instr_php(&mut self, mem_map: &mut impl MemMapped) -> Result<(), EmulationError> {
        let status_byte = self.reg_status.php();
        self.stack_push(mem_map, status_byte)?;

        Ok(())
    }

    #[inline]
    fn instr_plp(&mut self, mem_map: &mut impl MemMapped) -> Result<(), EmulationError> {
        let status_byte = self.stack_pull(mem_map)?;
        self.reg_status.plp(status_byte);

        if let Some(interrupt) = self.unhandled_interrupt {
            if !self.reg_status.interrupt_disable {
                self.instructions_since_last_interrupt = 0;
                self.pending_interrupt = Some(interrupt);
            }
        }

        Ok(())
    }

    //
// Flag instructions
//
    fn instr_clc(&mut self) -> Result<(), EmulationError> {
        self.reg_status.toggle_carry(false);

        Ok(())
    }

    #[inline]
    fn instr_sec(&mut self) -> Result<(), EmulationError> {
        self.reg_status.toggle_carry(true);

        Ok(())
    }

    #[inline]
    fn instr_cli(&mut self) -> Result<(), EmulationError> {
        if let Some(interrupt) = self.unhandled_interrupt {
            self.instructions_since_last_interrupt = 0;
            self.pending_interrupt = Some(interrupt);
        }

        self.reg_status.toggle_interrupt_disable(false);
        Ok(())
    }

    #[inline]
    fn instr_sei(&mut self) -> Result<(), EmulationError> {
        if let Some(interrupt) = self.unhandled_interrupt {
            if !self.reg_status.interrupt_disable {
                self.instructions_since_last_interrupt = 0;
                self.pending_interrupt = Some(interrupt);
            }
        }

        self.reg_status.toggle_interrupt_disable(true);
        Ok(())
    }

    #[inline]
    fn instr_clv(&mut self) -> Result<(), EmulationError> {
        self.reg_status.toggle_overflow(false);

        Ok(())
    }

    #[inline]
    fn instr_cld(&mut self) -> Result<(), EmulationError> {
        self.reg_status.toggle_decimal(false);

        Ok(())
    }

    #[inline]
    fn instr_sed(&mut self) -> Result<(), EmulationError> {
        self.reg_status.toggle_decimal(true);

        Ok(())
    }

    //
// Store/Load instructions
//
    #[inline]
    fn instr_lda(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        self.reg_a = self.read_resolved(instruction, mem_map)?;
        self.reg_status.toggle_zero_sign(self.reg_a);

        Ok(())
    }

    #[inline]
    fn instr_ldx(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        self.reg_x = self.read_resolved(instruction, mem_map)?;
        self.reg_status.toggle_zero_sign(self.reg_x);

        Ok(())
    }

    #[inline]
    fn instr_ldy(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        self.reg_y = self.read_resolved(instruction, mem_map)?;
        self.reg_status.toggle_zero_sign(self.reg_y);

        Ok(())
    }

    #[inline]
    fn instr_sta(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let reg_a = self.reg_a;
        self.write_resolved(instruction, mem_map, reg_a)?;

        Ok(())
    }

    #[inline]
    fn instr_stx(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let reg_x = self.reg_x;
        self.write_resolved(instruction, mem_map, reg_x)?;

        Ok(())
    }

    #[inline]
    fn instr_sty(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let reg_y = self.reg_y;
        self.write_resolved(instruction, mem_map, reg_y)?;

        Ok(())
    }

    //
// Register instructions
//
    #[inline]
    fn instr_tax(&mut self) -> Result<(), EmulationError> {
        self.reg_x = self.reg_a;
        self.reg_status.toggle_zero_sign(self.reg_x);

        Ok(())
    }

    #[inline]
    fn instr_txa(&mut self) -> Result<(), EmulationError> {
        self.reg_a = self.reg_x;
        self.reg_status.toggle_zero_sign(self.reg_a);

        Ok(())
    }

    #[inline]
    fn instr_dex(&mut self) -> Result<(), EmulationError> {
        self.reg_x = self.reg_x.wrapping_sub(1);
        self.reg_status.toggle_zero_sign(self.reg_x);

        Ok(())
    }

    #[inline]
    fn instr_inx(&mut self) -> Result<(), EmulationError> {
        self.reg_x = self.reg_x.wrapping_add(1);
        self.reg_status.toggle_zero_sign(self.reg_x);

        Ok(())
    }

    #[inline]
    fn instr_tay(&mut self) -> Result<(), EmulationError> {
        self.reg_y = self.reg_a;
        self.reg_status.toggle_zero_sign(self.reg_y);

        Ok(())
    }

    #[inline]
    fn instr_tya(&mut self) -> Result<(), EmulationError> {
        self.reg_a = self.reg_y;
        self.reg_status.toggle_zero_sign(self.reg_y);

        Ok(())
    }

    #[inline]
    fn instr_dey(&mut self) -> Result<(), EmulationError> {
        self.reg_y = self.reg_y.wrapping_sub(1);
        self.reg_status.toggle_zero_sign(self.reg_y);

        Ok(())
    }

    #[inline]
    fn instr_iny(&mut self) -> Result<(), EmulationError> {
        self.reg_y = self.reg_y.wrapping_add(1);
        self.reg_status.toggle_zero_sign(self.reg_y);

        Ok(())
    }

    //
// ALU instructions
//
    #[inline]
    fn instr_ora(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let byte = self.read_resolved(instruction, mem_map)?;

        self.reg_a |= byte;
        self.reg_status.toggle_zero_sign(self.reg_a);

        Ok(())
    }

    #[inline]
    fn instr_and(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let byte = self.read_resolved(instruction, mem_map)?;

        self.reg_a &= byte;
        self.reg_status.toggle_zero_sign(self.reg_a);

        Ok(())
    }

    #[inline]
    fn instr_eor(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let byte = self.read_resolved(instruction, mem_map)?;

        self.reg_a ^= byte;
        self.reg_status.toggle_zero_sign(self.reg_a);

        Ok(())
    }

    #[inline]
    fn instr_adc(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let byte = self.read_resolved(instruction, mem_map)?;
        self.perform_adc(byte);
        Ok(())
    }

    #[inline]
    fn instr_sbc(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let byte = self.read_resolved(instruction, mem_map)?;
        self.perform_adc(!byte);
        Ok(())
    }

    #[inline]
    fn instr_cmp(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let byte = self.read_resolved(instruction, mem_map)?;

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

        Ok(())
    }

    #[inline]
    fn instr_cpx(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let byte = self.read_resolved(instruction, mem_map)?;

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

        Ok(())
    }

    #[inline]
    fn instr_cpy(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let byte = self.read_resolved(instruction, mem_map)?;

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

        Ok(())
    }

    #[inline]
    fn instr_bit(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let byte = self.read_resolved(instruction, mem_map)?;

        let zero = byte & self.reg_a == 0;
        self.reg_status.toggle_zero(zero);

        let overflow = (byte >> 6) & 0b1 == 1;
        self.reg_status.toggle_overflow(overflow);

        let sign = (byte >> 7) & 0b1 == 1;
        self.reg_status.toggle_sign(sign);

        Ok(())
    }

    //
// Read/Modify/Write instructions
//
    #[inline]
    fn instr_asl(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let mut byte = self.read_resolved(instruction, mem_map)?;

        let carry = (byte >> 7) == 1;
        self.reg_status.toggle_carry(carry);

        byte = byte << 1;
        self.reg_status.toggle_zero_sign(byte);

        self.write_resolved(instruction, mem_map, byte)?;

        Ok(())
    }

    #[inline]
    fn instr_rol(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let mut byte = self.read_resolved(instruction, mem_map)?;

        let old_carry = self.reg_status.carry_flag as u8;
        let new_carry = (byte >> 7) == 1;

        self.reg_status.toggle_carry(new_carry);

        byte = byte << 1;
        byte |= old_carry;
        self.reg_status.toggle_zero_sign(byte);

        self.write_resolved(instruction, mem_map, byte)?;

        Ok(())
    }

    #[inline]
    fn instr_lsr(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let mut byte = self.read_resolved(instruction, mem_map)?;

        let carry = (byte & 1) == 1;
        self.reg_status.toggle_carry(carry);

        byte = byte >> 1;
        self.reg_status.toggle_zero_sign(byte);

        self.write_resolved(instruction, mem_map, byte)?;

        Ok(())
    }

    #[inline]
    fn instr_ror(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let mut byte = self.read_resolved(instruction, mem_map)?;

        let old_carry = self.reg_status.carry_flag as u8;
        let new_carry = (byte & 1) == 1;

        self.reg_status.toggle_carry(new_carry);

        byte = byte >> 1;
        byte |= old_carry << 7;
        self.reg_status.toggle_zero_sign(byte);

        self.write_resolved(instruction, mem_map, byte)?;

        Ok(())
    }

    #[inline]
    fn instr_dec(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let mut byte = self.read_resolved(instruction, mem_map)?;

        byte = byte.wrapping_sub(1);
        self.reg_status.toggle_zero_sign(byte);

        self.write_resolved(instruction, mem_map, byte)?;

        Ok(())
    }

    #[inline]
    fn instr_inc(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<(), EmulationError> {
        let mut byte = self.read_resolved(instruction, mem_map)?;

        byte = byte.wrapping_add(1);
        self.reg_status.toggle_zero_sign(byte);

        self.write_resolved(instruction, mem_map, byte)?;

        Ok(())
    }

//////////////
//
// Helpers
//
//////////////

    #[inline]
    fn perform_irq(
        &mut self,
        mem_map: &mut impl MemMapped,
        interrupt: &CpuInterrupt,
    ) -> Result<(), EmulationError> {
        let mut new_reg_pc = self.reg_pc;

        if !interrupt.is_hardware {
            new_reg_pc = new_reg_pc.wrapping_add(2);
            self.reg_status.toggle_break_executed(true);
        }

        let status_byte = if interrupt.is_hardware {
            self.reg_status.irq()
        } else {
            self.reg_status.php()
        };

        self.stack_push_addr(mem_map, new_reg_pc)?;
        self.stack_push(mem_map, status_byte)?;

        self.reg_pc = if interrupt.is_nmi {
            mem_map.read_word(NMI_PC_VEC)?
        } else {
            mem_map.read_word(BRK_PC_VEC)?
        };

        self.reg_status.interrupt_disable = true;
        self.cycle_count += 7;
        Ok(())
    }

    // Due to the complexity of the ADC/SBC instructions, they are
// performed here for both instr_adc and instr_sbc
    #[inline]
    fn perform_adc(&mut self, byte: u8) {
        let old_carry = self.reg_status.carry_flag as u16;

        let sum: u16 = self.reg_a as u16 + byte as u16 + old_carry;

        let carry = sum > 0xFF;
        self.reg_status.toggle_carry(carry);

        let overflow = !(((self.reg_a as u16 ^ byte as u16) & 0x80) != 0)
            && (((self.reg_a as u16 ^ sum) & 0x80) != 0);
        self.reg_status.toggle_overflow(overflow);

        self.reg_a = sum as u8;
        self.reg_status.toggle_zero_sign(self.reg_a);
    }

    #[inline]
    pub fn read_resolved(
        &self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
    ) -> Result<u8, EmulationError> {
        use core::instructions::AddressingMode::*;

        let addressing_mode = &instruction.addressing_mode;

        match *addressing_mode {
            ZeroPageIndexedX(arg) => mem_map.read(arg.wrapping_add(self.reg_x) as u16),
            ZeroPageIndexedY(arg) => mem_map.read(arg.wrapping_add(self.reg_y) as u16),
            AbsoluteIndexedX(arg) => {
                if (arg & 0xFF) + self.reg_x as u16 > 0xFF {
                    instruction.cycle_count += 1;
                }

                mem_map.read(arg.wrapping_add(self.reg_x as u16))
            }
            AbsoluteIndexedY(arg) => {
                if (arg & 0xFF) + self.reg_y as u16 > 0xFF {
                    instruction.cycle_count += 1;
                }

                mem_map.read(arg.wrapping_add(self.reg_y as u16))
            }
            IndexedIndirectX(arg) => {
                let arg_plus_x = arg.wrapping_add(self.reg_x) as u16;

                // When reading from addresses at page boundaries (0xFF)
                // we read the low byte from 0xFF and high byte from 0x00
                // relative to that page
                // For example:
                // X: 0; $00 = 05; $FF = 03;
                // LDA ($FF, X) [-> LDA ($FF)]
                // since the high byte would be at addr $100
                // it wraps around and instead takes the high byte
                // of the destination address from $00
                // resulting in the address $0503

                let addr_low = mem_map.read(arg_plus_x)?;
                let addr_high = mem_map.read(arg_plus_x.wrapping_add(1))?;

                let addr = ((addr_high as u16) << 8) | addr_low as u16;

                mem_map.read(addr)
            }
            IndirectIndexedY(arg) => {
                let addr_low = mem_map.read(arg as u16)?;
                let addr_high = mem_map.read(arg.wrapping_add(1) as u16)?;
                let arg_resolved = ((addr_high as u16) << 8) | addr_low as u16;

                let addr = arg_resolved.wrapping_add(self.reg_y as u16);

                if (arg_resolved & 0xFF) + self.reg_y as u16 > 0xFF {
                    instruction.cycle_count += 1;
                }

                mem_map.read(addr)
            }

            Immediate(arg) => Ok(arg),
            Accumulator => Ok(self.reg_a),
            ZeroPage(arg) => mem_map.read(arg as u16),
            Absolute(arg) => mem_map.read(arg),

            // Implicit, Relative and Indirect addressing modes are handled
            // by the instructions themselves
            _ => Ok(0),
        }
    }

    #[inline]
    fn write_resolved(
        &mut self,
        instruction: &mut Instruction,
        mem_map: &mut impl MemMapped,
        byte: u8,
    ) -> Result<(), EmulationError> {
        use core::instructions::AddressingMode::*;

        let addressing_mode = &instruction.addressing_mode;
        match *addressing_mode {
            ZeroPageIndexedX(arg) => mem_map.write(arg.wrapping_add(self.reg_x) as u16, byte),
            ZeroPageIndexedY(arg) => mem_map.write(arg.wrapping_add(self.reg_y) as u16, byte),
            AbsoluteIndexedX(arg) => mem_map.write(arg.wrapping_add(self.reg_x as u16), byte),
            AbsoluteIndexedY(arg) => mem_map.write(arg.wrapping_add(self.reg_y as u16), byte),
            IndexedIndirectX(arg) => {
                let arg_plus_x = arg.wrapping_add(self.reg_x);

                let addr_low = mem_map.read(arg_plus_x as u16)?;
                let addr_high = mem_map.read(arg_plus_x.wrapping_add(1) as u16)?;

                // See comment in the read_resolved function above
                let addr = ((addr_high as u16) << 8) | addr_low as u16;

                mem_map.write(addr, byte)
            }
            IndirectIndexedY(arg) => {
                let addr_low = mem_map.read(arg as u16)?;
                let addr_high = mem_map.read(arg.wrapping_add(1) as u16)?;
                let arg_resolved = ((addr_high as u16) << 8) | addr_low as u16;

                // See comment in the read_resolved function above
                let addr = arg_resolved.wrapping_add(self.reg_y as u16);

                mem_map.write(addr, byte)
            }

            ZeroPage(arg) => mem_map.write(arg as u16, byte),
            Absolute(arg) => mem_map.write(arg, byte),
            Accumulator => {
                self.reg_a = byte;

                Ok(())
            }
            // Above covers all addresing modes for writing memory
            _ => unreachable!(),
        }
    }

    fn stack_push(&mut self, mem_map: &mut impl MemMapped, byte: u8) -> Result<(), EmulationError> {
        //        if self.reg_sp == 0 {
        //            println!("Stack overflow detected! Wrapping...");
        //        }

        let addr = 0x100 + (self.reg_sp as u16);
        mem_map.write(addr, byte)?;

        self.reg_sp = self.reg_sp.wrapping_sub(1);

        Ok(())
    }

    fn stack_pull(&mut self, mem_map: &mut impl MemMapped) -> Result<u8, EmulationError> {
        //        if self.reg_sp == 0xFF {
        //            println!("Stack underflow detected! Wrapping...");
        //        }

        self.reg_sp = self.reg_sp.wrapping_add(1);

        let addr = 0x100 + self.reg_sp as u16;

        mem_map.read(addr)
    }

    fn stack_push_addr(
        &mut self,
        mem_map: &mut impl MemMapped,
        addr: u16,
    ) -> Result<(), EmulationError> {
        let addr_high = ((addr & 0xFF00) >> 8) as u8;
        let addr_low = (addr & 0xFF) as u8;

        self.stack_push(mem_map, addr_high)?;
        self.stack_push(mem_map, addr_low)?;

        Ok(())
    }

    fn stack_pull_addr(&mut self, mem_map: &mut impl MemMapped) -> Result<u16, EmulationError> {
        let addr_low = self.stack_pull(mem_map)?;
        let addr_high = self.stack_pull(mem_map)?;

        let addr = ((addr_high as u16) << 8) | addr_low as u16;

        Ok(addr)
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
                // The offset is added AFTER the PC has been incremented by 2
                // (which happens regardless of whether the branch is being taken or not)
                // So we need to check for page boundary crossing AFTER the PC has been incremented
                // by 2
                let old_reg_pc = self.reg_pc + 2;
                let reg_pc_i32 = self.reg_pc as i32;
                self.reg_pc = reg_pc_i32.wrapping_add(offset as i32) as u16;

                if old_reg_pc & 0xFF00 != self.reg_pc & 0xFF00 {
                    // moved to previous or next page, increase cycle count by 1
                    instruction.cycle_count += 1;
                }
            }
            _ => unreachable!(),
        }
    }
}

impl Display for Cpu {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let status_reg_byte: u8 = self.reg_status.byte();
        write!(
            f,
            "A:0x{:02X} X:0x{:02X} Y:0x{:02X} P:0x{:02X} SP:0x{:02X} N:{} I:{} UI:{} PI:{} CYC:{}",
            self.reg_a,
            self.reg_x,
            self.reg_y,
            status_reg_byte,
            self.reg_sp,
            self.reg_status.sign_flag as u8,
            self.reg_status.interrupt_disable as u8,
            self.unhandled_interrupt.is_some() as u8,
            self.pending_interrupt.is_some() as u8,
            self.cycle_count
        )
    }
}
