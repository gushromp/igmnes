use std::io::{self, Read, Write};
use std::collections::{HashSet, HashMap};
use std::collections::hash_map::Entry;
use core::CpuFacade;
use core::memory::MemMapped;
use core::debugger::Debugger;
use core::debugger::command::Command;
use core::cpu::Cpu;

struct MemMapShim<'a> {
    mem_map: &'a mut MemMapped
}

impl<'a> MemMapShim<'a> {
    pub fn new(mem_map: &'a mut MemMapped) -> MemMapShim {
        MemMapShim {
            mem_map: mem_map
        }
    }
}

pub struct TerminalDebugger {
    cpu: Box<Cpu>,
    breakpoint_set: HashSet<u16>,
    watchpoint_set: HashSet<u16>,
    label_map: HashMap<u16, String>,
}

impl TerminalDebugger {
    pub fn new(cpu: Box<Cpu>) -> TerminalDebugger {
        TerminalDebugger {
            cpu: cpu,
            breakpoint_set: HashSet::new(),
            watchpoint_set: HashSet::new(),
            label_map: HashMap::new(),
        }
    }

    fn execute_command(&mut self, command: Command) {
        use core::debugger::command::Command::*;

        match command {
            ShowUsage => TerminalDebugger::show_usage(),
            PrintState => self.print_state(),
            PrintBreakpoints => self.print_breakpoints(),
            PrintWatchpoints => self.print_watchpoints(),
            PrintLabels => self.print_labels(),
            SetBreakpoint(addr) => self.set_breakpoint(addr),
            RemoveBreakpoint(addr) => self.remove_breakpoint(addr),
            SetWatchpoint(addr) => self.set_watchpoint(addr),
            RemoveWatchpoint(addr) => self.remove_watchpoint(addr),
            SetLabel(label, addr) => self.set_label(addr, label),
            RemoveLabel(addr) => self.remove_label(addr),
            ClearBreakpoints => self.clear_breakpoints(),
            ClearWatchpoints => self.clear_watchpoints(),
            ClearLabels => self.clear_labels(),
            c @ _ => println!("{:?}", c)
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
        println!("{:#?}", self.cpu);
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

    fn set_label(&mut self, addr: u16, label: String) {
        self.label_map.insert(addr, label);

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
}

impl Debugger for TerminalDebugger {
    fn start_listening(&mut self) {
        let pc = self.cpu.reg_pc;

        let mut stdout = io::stdout();

        while true {
            print!("0x{:X} -> ", pc);
            stdout.flush().unwrap();

            let mut line = String::new();
            let stdin = io::stdin();
            stdin.read_line(&mut line);

            let command = Command::parse(&line);

            match command {
                Ok(command) => self.execute_command(command),
                Err(err) => println!("{:#?}", err),
            }
        }
    }

    fn stop_listening(&mut self) {}
}

impl CpuFacade for TerminalDebugger {
    fn cpu(self: Box<Self>) -> Box<Cpu> {
        self.cpu
    }

    fn debugger(&mut self) -> Option<&mut Debugger> {
        Some(self)
    }

    fn step(&mut self, mem_map: &mut MemMapped) -> u8 {
        let mut mem_map_shim = MemMapShim::new(mem_map);

        self.cpu.step(&mut mem_map_shim)
    }
}

impl<'a> MemMapped for MemMapShim<'a> {
    fn read(&self, index: u16) -> u8 {
        self.mem_map.read(index)
    }

    fn write(&mut self, index: u16, byte: u8) {
        self.mem_map.write(index, byte);
    }
}