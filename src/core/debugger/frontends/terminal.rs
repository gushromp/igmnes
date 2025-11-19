use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::io;
use std::io::prelude::*;
use std::ops::Range;

use crate::core::apu::Apu;
use crate::core::controller::Controller;
use crate::core::cpu::Cpu;
use crate::core::debug::Tracer;
use crate::core::debugger::command::Command;
use crate::core::debugger::disassembler;
use crate::core::debugger::Debugger;
use crate::core::dma::Dma;
use crate::core::errors::EmulationError;
use crate::core::memory::{CpuMemMap, MemMapped};
use crate::core::ppu::Ppu;
use crate::core::BusOps;

struct MemMapShim<'a> {
    mem_map: &'a mut CpuMemMap,
    watchpoint_set: &'a HashSet<u16>,
}

impl<'a> Clone for MemMapShim<'a> {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl<'a> MemMapShim<'a> {
    pub fn new(mem_map: &'a mut CpuMemMap, watchpoint_set: &'a HashSet<u16>) -> MemMapShim<'a> {
        MemMapShim {
            mem_map,
            watchpoint_set,
        }
    }
}

pub struct TerminalDebugger {
    cpu: Cpu,
    mem_map: CpuMemMap,
    breakpoint_set: HashSet<u16>,
    breakpoint_cycles_set: HashSet<u64>,
    watchpoint_set: HashSet<u16>,
    label_map: HashMap<u16, String>,
    is_listening: bool,
    cur_breakpoint_addr: Option<u16>,
    cur_watchpoint_addr: Option<u16>,
    trace_active: bool,
}

impl TerminalDebugger {
    pub fn new(cpu: Cpu, mem_map: CpuMemMap) -> TerminalDebugger {
        TerminalDebugger {
            cpu,
            mem_map,
            breakpoint_set: HashSet::new(),
            breakpoint_cycles_set: HashSet::new(),
            watchpoint_set: HashSet::new(),
            label_map: HashMap::new(),
            is_listening: false,
            cur_breakpoint_addr: None,
            cur_watchpoint_addr: None,
            trace_active: false,
        }
    }

    fn execute_command(&mut self, command: &Command) {
        use crate::core::debugger::command::Command::*;

        match *command {
            ShowUsage => TerminalDebugger::show_usage(),
            PrintState => self.print_state(),
            PrintMemory(ref range) => self.print_memory(range),
            PrintBreakpoints => self.print_breakpoints(),
            PrintWatchpoints => self.print_watchpoints(),
            PrintLabels => self.print_labels(),
            SetBreakpoint(addr) => self.set_breakpoint(addr),
            RemoveBreakpoint(addr) => self.remove_breakpoint(addr),
            SetBreakpointCycles(cycles) => self.set_breakpoint_cycles(cycles),
            SetWatchpoint(addr) => self.set_watchpoint(addr),
            RemoveWatchpoint(addr) => self.remove_watchpoint(addr),
            SetLabel(ref label, addr) => self.set_label(addr, label),
            RemoveLabel(addr) => self.remove_label(addr),
            ClearBreakpoints => self.clear_breakpoints(),
            ClearWatchpoints => self.clear_watchpoints(),
            ClearLabels => self.clear_labels(),
            Goto(addr) => self.goto(addr),
            Disassemble(ref range) => self.disassemble(range),
            Reset => self.reset(),
            Trace => self.trace(),
            RepeatCommand(ref command, count) => self.repeat_command(command, count),
            _ => unreachable!(),
        };
    }

    fn show_usage() {
        println!();
        println!("Usage:");
        println!("---------------------------------------------------------");
        println!("Command Name                      Short       Description");
        println!("---------------------------------------------------------");
        println!("PrintMemory                       pm          prints current RAM state");
        println!("PrintState                        ps          prints current CPU state");
        println!("PrintBreakpoints                  pb          shows all set breakpoints");
        println!("PrintWatchpoints                  pw          shows all set watchpoints");
        println!("PrintLabels                       pl          shows all set labels");
        println!(
            "SetBreakpoint addr                sb          sets a CPU breakpoint at target address"
        );
        println!("RemoveBreakpoint addr             rb          removes a CPU breakpoint at target address");
        println!("ClearBreakpoints                  cb          clears all breakpoints");
        println!("SetWatchpoint addr                sw          sets a memory watchpoint at target address");
        println!("RemoveWatchpoint addr             rw          removes a memory watchpoint at target address");
        println!("ClearWatchpoints                  cw          clears all watchpoints");
        println!(
            "SetLabel addr                     sl          sets a text label at target address"
        );
        println!(
            "RemoveLabel addr                  rl          removes a text label at target address"
        );
        println!("ClearLabels                       cl          clears all text labels");
        println!("Disassemble [range]               d           disassembles CPU instructions for the given range \
            (optional, defaults to 5 instructions)");
        println!("Goto                              g           sets the CPU program counter to target address");
        println!("RepeatCommand (command) n         r           repeats the given debugger command n times");
        println!();
    }

    fn print_state(&self) {
        println!();
        println!("Cpu state:");
        println!("----------");
        println!("{}", self.cpu);
        println!();
    }

    fn print_memory(&mut self, range: &Range<u16>) {
        let (mut cursor, rows) = if range.start > 0 {
            (range.start, range.end - range.start)
        } else {
            (range.end, 8)
        };

        let columns = 16;

        println!();
        println!("Memory state (starting at 0x{:04X}):", cursor);
        println!();
        println!("         00  01  02  03  04  05  06  07  08  09  0A  0B  0C  0D  0E  0F");
        println!("       ----------------------------------------------------------------");
        for _i in 0..rows {
            print!("0x{:04X} | ", cursor);
            for j in 0..columns {
                let byte = self.mem_map.read(cursor);
                print!("{:02X}", byte);

                cursor += 1;
                if j < columns - 1 {
                    print!("  ");
                }
            }
            println!();
        }
        println!();
    }

    fn print_breakpoints(&self) {
        println!();
        println!("List of currently set breakpoints:");
        println!("----------------------------------");
        for addr in &self.breakpoint_set {
            println!(" | 0x{:04X} |", addr);
        }
        println!();
    }

    fn print_watchpoints(&self) {
        println!();
        println!("List of currently set watchpoints:");
        println!("----------------------------------");
        for addr in &self.watchpoint_set {
            println!(" | 0x{:04X} |", addr);
        }
        println!();
    }

    fn print_labels(&self) {
        println!();
        println!("List of currently set labels:");
        println!("-----------------------------");
        for (addr, ref label) in &self.label_map {
            println!(" | 0x{:04X} .{} |", addr, label);
        }
        println!();
    }

    fn set_breakpoint(&mut self, addr: u16) {
        self.breakpoint_set.insert(addr);

        println!();
        println!(
            "Successfully set breakpoint for program counter address: 0x{:X}",
            addr
        );
        println!();
    }

    fn remove_breakpoint(&mut self, addr: u16) {
        let result = self.breakpoint_set.remove(&addr);

        println!();
        if result {
            println!(
                "Successfully removed breakpoint for program counter address: 0x{:X}",
                addr
            );
        } else {
            println!(
                "No breakpoint present for program counter address: 0x{:X}",
                addr
            );
        }
        println!();
    }

    fn set_breakpoint_cycles(&mut self, cycles: u64) {
        self.breakpoint_cycles_set.insert(cycles);

        println!();
        println!(
            "Successfully set breakpoint for CPU cycles count: {}",
            cycles
        );
        println!();
    }

    fn clear_breakpoints(&mut self) {
        self.breakpoint_set.clear();

        println!();
        println!("Cleared all breakpoints");
        println!();
    }

    fn set_watchpoint(&mut self, addr: u16) {
        self.watchpoint_set.insert(addr);

        println!();
        println!(
            "Successfully set watchpoint for memory address: 0x{:X}",
            addr
        );
        println!();
    }

    fn remove_watchpoint(&mut self, addr: u16) {
        let result = self.watchpoint_set.remove(&addr);

        println!();
        if result {
            println!(
                "Successfully removed watchpoint for memory address: 0x{:X}",
                addr
            );
        } else {
            println!("No watchpoint present for memory address: 0x{:X}", addr);
        }
        println!();
    }

    fn clear_watchpoints(&mut self) {
        self.watchpoint_set.clear();

        println!();
        println!("Cleared all watchpoints");
        println!();
    }

    fn set_label(&mut self, addr: u16, label: &String) {
        self.label_map.insert(addr, label.clone());

        if let Entry::Occupied(e) = self.label_map.entry(addr) {
            let label = e.get();

            println!();
            println!(
                "Successfully set label \"{}\" for memory address: 0x{:X}",
                label, addr
            );
            println!();
        }
    }

    fn remove_label(&mut self, addr: u16) {
        let result = self.label_map.remove(&addr);

        println!();
        if let Some(_) = result {
            println!(
                "Successfully removed label for memory address: 0x{:X}",
                addr
            );
        } else {
            println!("No label present for memory address: 0x{:X}", addr);
        }
        println!();
    }

    fn clear_labels(&mut self) {
        self.label_map.clear();

        println!();
        println!("Cleared all labels");
        println!();
    }

    fn goto(&mut self, addr: u16) {
        self.cpu.reg_pc = addr;

        println!();
        println!("Changed program counter value to: 0x{:04X}", addr);
        println!();
    }

    fn disassemble(&mut self, range: &Range<u16>) {
        let addr = self.cpu.reg_pc;
        let disassembly =
            disassembler::disassemble_range(addr, range, &self.cpu, &mut self.mem_map).unwrap();

        println!();
        println!("Disassembly:");
        println!("------------");
        for (index, line) in disassembly.into_iter().enumerate() {
            if index == 0 {
                println!("{}\t{}\t{}", line, &self.cpu, &self.mem_map.ppu);
            } else {
                println!("{}", line);
            }
        }
        println!();
    }

    fn reset(&mut self) {
        self.cpu.hard_reset(&mut self.mem_map);

        println!();
        println!("CPU has been reset");
        println!();
    }

    fn trace(&mut self) {
        self.trace_active = !self.trace_active;

        println!();
        if self.trace_active {
            println!("Began tracing");
        } else {
            println!("Stopped tracing");
        }

        println!();
    }

    fn repeat_command(&mut self, command: &Box<Command>, count: u16) {
        for _i in 0..count {
            self.execute_command(command);
        }
    }
}

impl Debugger for TerminalDebugger {
    fn break_into(&mut self) {
        use crate::core::debugger::command::Command::*;

        let mut stdout = io::stdout();

        let range: Range<u16> = 0..5;
        self.disassemble(&range);

        'debug: loop {
            let pc = self.cpu.reg_pc;
            print!("0x{:04X} -> ", pc);
            stdout.flush().unwrap();

            let mut line = String::new();
            let stdin = io::stdin();
            stdin.read_line(&mut line).unwrap();

            let command = Command::parse(&line);

            match command {
                Ok(ref command) => {
                    match *command {
                        Step => {
                            break 'debug;
                        }
                        Continue => {
                            self.stop_listening();
                            break 'debug;
                        }
                        ref command @ _ => self.execute_command(command),
                    };
                }
                Err(err) => println!("{:#?}", err),
            }
        }
    }

    fn start_listening(&mut self) {
        self.is_listening = true;
    }
    fn stop_listening(&mut self) {
        self.is_listening = false;
    }

    fn is_listening(&self) -> bool {
        self.is_listening
    }
}
impl BusOps for TerminalDebugger {
    fn consume(self) -> (Cpu, CpuMemMap) {
        (self.cpu, self.mem_map)
    }

    fn cpu(&mut self) -> &mut Cpu {
        &mut self.cpu
    }

    fn ppu(&mut self) -> &mut Ppu {
        &mut self.mem_map.ppu
    }

    fn apu(&mut self) -> &mut Apu {
        &mut self.mem_map.apu
    }

    fn dma(&mut self) -> &mut Dma {
        &mut self.mem_map.dma
    }

    fn controllers(&mut self) -> &mut [Controller; 2] {
        &mut self.mem_map.controllers
    }

    fn step_cpu(&mut self, tracer: &mut Tracer) -> Result<u8, EmulationError> {
        let reg_pc = self.cpu.reg_pc;

        if self.breakpoint_set.contains(&reg_pc) {
            if let Some(_addr) = self.cur_breakpoint_addr {
                self.cur_breakpoint_addr = None;
            } else {
                println!("Address breakpoint hit");
                self.cur_breakpoint_addr = Some(reg_pc);
                return Err(EmulationError::DebuggerBreakpoint(self.cpu.reg_pc));
            }
        }

        for break_cycles in &self.breakpoint_cycles_set {
            if self.cpu.cycle_count >= *break_cycles {
                if let Some(_addr) = self.cur_breakpoint_addr {
                    self.cur_breakpoint_addr = None;
                } else {
                    println!("CPU cycles breakpoint hit");
                    self.cur_breakpoint_addr = Some(reg_pc);
                    let to_remove = *break_cycles;
                    self.breakpoint_cycles_set.remove(&to_remove);
                    return Err(EmulationError::DebuggerBreakpoint(self.cpu.reg_pc));
                }
            }
        }

        tracer.set_enabled(self.trace_active);

        let cpu_result = {
            let mut mem_map_shim = MemMapShim::new(&mut self.mem_map, &self.watchpoint_set);
            self.cpu.step(&mut mem_map_shim, tracer)
        };

        match cpu_result {
            Err(EmulationError::DebuggerWatchpoint(addr)) => {
                if let Some(_addr) = self.cur_watchpoint_addr {
                    // Already broken into debugger at this watchpoint,
                    // Execute this instruction
                    self.cur_watchpoint_addr = None;

                    self.cpu.step(&mut self.mem_map, tracer)
                } else {
                    println!("Watchpoint hit");
                    self.cur_watchpoint_addr = Some(addr);

                    Err(EmulationError::DebuggerWatchpoint(addr))
                }
            }
            res @ _ => res,
        }
    }

    fn step_ppu(&mut self, cpu_cycle_count: u64, tracer: &mut Tracer) -> bool {
        tracer.set_enabled(self.trace_active);
        self.mem_map.ppu.step(cpu_cycle_count, tracer)
    }

    fn step_apu(&mut self, cpu_cycles: u64) -> bool {
        self.mem_map.apu.step(cpu_cycles)
    }

    fn step_dma(&mut self) -> bool {
        // let dma = &mut self.dma();
        // let cpu_ram = &mut self.mem_map.ram;
        // let ppu_mem_map = &mut self.mem_map.ppu_mem_map;
        //
        // dma.step(cpu_ram, ppu_mem_map)
        true
    }

    fn nmi(&mut self) {
        self.cpu.nmi(&mut self.mem_map)
    }

    fn irq(&mut self) {
        self.cpu.irq(&mut self.mem_map);
    }
}

impl<'a> MemMapped for MemMapShim<'a> {
    fn read(&mut self, index: u16) -> u8 {
        match self.watchpoint_set.contains(&index) {
            true => todo!("Reimplement watchpoints after moving to infallible functions"), //Err(EmulationError::DebuggerWatchpoint(index)),
            false => self.mem_map.read(index),
        }
    }

    fn write(&mut self, index: u16, byte: u8) {
        match self.watchpoint_set.contains(&index) {
            true => todo!("Reimplement watchpoints after moving to infallible functions"), // Err(EmulationError::DebuggerWatchpoint(index)),
            false => self.mem_map.write(index, byte),
        }
    }
}
