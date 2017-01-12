use std::io::{self, Read, Write};
use std::collections::{HashSet, HashMap};
use std::collections::hash_map::Entry;
use std::ops::Range;
use std::mem;
use std::cell::RefCell;
use core::CpuFacade;
use core::cpu::Cpu;
use core::memory::{MemMap, MemMapped};
use core::debugger::Debugger;
use core::debugger::command::Command;
use core::debugger::disassembler;
use core::errors::CpuError;

struct MemMapShim<'a> {
    mem_map: &'a mut MemMapped,
    accessed_address: RefCell<Option<u16>>,
}

impl<'a> MemMapShim<'a> {
    pub fn new(mem_map: &'a mut MemMapped) -> MemMapShim {
        MemMapShim {
            mem_map: mem_map,
            accessed_address: RefCell::new(None),
        }
    }
}

pub struct TerminalDebugger {
    cpu: Cpu,
    mem_map: MemMap,
    breakpoint_set: HashSet<u16>,
    watchpoint_set: HashSet<u16>,
    label_map: HashMap<u16, String>,
    is_listening: bool,
    cur_breakpoint_addr: Option<u16>,
}

impl TerminalDebugger {
    pub fn new(cpu: Cpu, mem_map: MemMap) -> TerminalDebugger {
        TerminalDebugger {
            cpu: cpu,
            mem_map: mem_map,
            breakpoint_set: HashSet::new(),
            watchpoint_set: HashSet::new(),
            label_map: HashMap::new(),
            is_listening: false,
            cur_breakpoint_addr: None,
        }
    }

    fn execute_command(&mut self, command: &Command) {
        use core::debugger::command::Command::*;

        match *command {
            ShowUsage => TerminalDebugger::show_usage(),
            PrintState => self.print_state(),
            PrintMemory(ref range) => self.print_memory(range),
            PrintBreakpoints => self.print_breakpoints(),
            PrintWatchpoints => self.print_watchpoints(),
            PrintLabels => self.print_labels(),
            SetBreakpoint(addr) => self.set_breakpoint(addr),
            RemoveBreakpoint(addr) => self.remove_breakpoint(addr),
            SetWatchpoint(addr) => self.set_watchpoint(addr),
            RemoveWatchpoint(addr) => self.remove_watchpoint(addr),
            SetLabel(ref label, addr) => self.set_label(addr, label),
            RemoveLabel(addr) => self.remove_label(addr),
            ClearBreakpoints => self.clear_breakpoints(),
            ClearWatchpoints => self.clear_watchpoints(),
            ClearLabels => self.clear_labels(),
            Goto(addr) => self.goto(addr),
            Step => self.step_cpu(),
            Disassemble(ref range) => self.disassemble(range),
            Continue => self.stop_listening(),
            RepeatCommand(ref command, count) => self.repeat_command(command, count),
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
        println!("SetBreakpoint addr                sb          sets a CPU breakpoint at target address");
        println!("RemoveBreakpoint addr             rb          removes a CPU breakpoint at target address");
        println!("ClearBreakpoints                  cb          clears all breakpoints");
        println!("SetWatchpoint addr                sw          sets a memory watchpoint at target address");
        println!("RemoveWatchpoint addr             rw          removes a memory watchpoint at target address");
        println!("ClearWatchpoints                  cw          clears all watchpoints");
        println!("SetLabel addr                     sl          sets a text label at target address");
        println!("RemoveLabel addr                  rl          removes a text label at target address");
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

    fn print_memory(&self, range: &Range<u16>) {
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
        for i in 0..rows {
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
        println!("Successfully set breakpoint for program counter address: 0x{:X}", addr);
        println!();
    }

    fn remove_breakpoint(&mut self, addr: u16) {
        let result = self.breakpoint_set.remove(&addr);

        println!();
        if result {
            println!("Successfully removed breakpoint for program counter address: 0x{:X}", addr);
        } else {
            println!("No breakpoint present for program counter address: 0x{:X}", addr);
        }
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
        println!("Successfully set watchpoint for memory address: 0x{:X}", addr);
        println!();
    }

    fn remove_watchpoint(&mut self, addr: u16) {
        let result = self.watchpoint_set.remove(&addr);

        println!();
        if result {
            println!("Successfully removed watchpoint for memory address: 0x{:X}", addr);
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
            println!("Successfully set label \"{}\" for memory address: 0x{:X}", label, addr);
            println!();
        }
    }

    fn remove_label(&mut self, addr: u16) {
        let result = self.label_map.remove(&addr);

        println!();
        if let Some(_) = result {
            println!("Successfully removed label for memory address: 0x{:X}", addr);
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

    fn step_cpu(&mut self) {
        self.step();
        self.print_state();

        let range = 0..5;
        self.disassemble(&range);
    }

    fn disassemble(&self, range: &Range<u16>) {
        let addr = self.cpu.reg_pc;
        let disassembly = disassembler::disassemble_range(addr, range, &self.cpu, &self.mem_map);

        println!();
        println!("Disassembly:");
        println!("------------");
        for line in disassembly.into_iter() {
            println!("{}", line);
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
    fn start_listening(&mut self) {
        use core::debugger::command::Command::*;

        let mut stdout = io::stdout();
        self.is_listening = true;

        'debug: loop {
            let pc = self.cpu.reg_pc;
            print!("0x{:X} -> ", pc);
            stdout.flush().unwrap();

            let mut line = String::new();
            let stdin = io::stdin();
            stdin.read_line(&mut line);

            let command = Command::parse(&line);

            match command {
                Ok(ref command) => {
                    match *command {
                        Continue => {
                            self.stop_listening();
                            break 'debug;
                        }
                        ref command @ _ => self.execute_command(command),
                    };
                },
                Err(err) => println!("{:#?}", err),
            }
        }
    }
    fn stop_listening(&mut self) {
        self.is_listening = false;
    }

    fn is_listening(&self) -> bool {
        self.is_listening
    }
}

impl CpuFacade for TerminalDebugger {
    fn consume(self: Box<Self>) -> (Cpu, MemMap) {
        let this = *self;

        (this.cpu, this.mem_map)
    }

    fn debugger(&mut self) -> Option<&mut Debugger> {
        Some(self)
    }

    fn step(&mut self) -> Result<u8, CpuError> {
        let reg_pc = self.cpu.reg_pc;

        if self.breakpoint_set.contains(&reg_pc) {
            if let Some(addr) = self.cur_breakpoint_addr {
                self.cur_breakpoint_addr = None;
            } else {
                println!("Breakpoint hit");
                self.cur_breakpoint_addr = Some(reg_pc);
                return Err(CpuError::DebuggerBreakpoint(self.cpu.reg_pc));
            }
        }

        let mut mem_map_shim = MemMapShim::new(&mut self.mem_map);

        let cpu_result = self.cpu.step(&mut mem_map_shim);

        if let Some(addr) = *mem_map_shim.accessed_address.borrow() {
            if self.watchpoint_set.contains(&addr) {
                println!("Watchpoint hit");
                return Err(CpuError::DebuggerWatchpoint(addr));
            }
        }

        cpu_result
    }
}

impl<'a> MemMapped for MemMapShim<'a> {
    fn read(&self, index: u16) -> u8 {
        let mut accessed_address = self.accessed_address.borrow_mut();
        *accessed_address = Some(index);

        self.mem_map.read(index)
    }

    fn write(&mut self, index: u16, byte: u8) {
        let mut accessed_address = self.accessed_address.borrow_mut();
        *accessed_address = Some(index);

        self.mem_map.write(index, byte);
    }
}