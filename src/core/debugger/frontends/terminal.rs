use core::CpuFacade;
use core::memory::MemMapped;
use core::debugger::Debugger;
use core::debugger::command::Command;
use core::cpu::Cpu;
use std::io::{self, Read, Write};

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
}

impl TerminalDebugger {
    pub fn new(cpu: Box<Cpu>) -> TerminalDebugger {
        TerminalDebugger {
            cpu: cpu
        }
    }

    fn execute_command(&mut self, command: Command) {
        use core::debugger::command::Command::*;

        match command {
            ShowUsage => TerminalDebugger::show_usage(),
            PrintState => self.print_state(),
            c @ _ => println!("{:?}", c)
        }
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
        println!("BreakpointSet addr                bs          sets a CPU breakpoint at target address");
        println!("BreakpointRemove addr             br          removes a CPU breakpoint at target address");
        println!("WatchpointSet addr                ws          sets a memory watchpoint at target address");
        println!("WatchpointRemove addr             wr          removes a memory watchpoint at target address");
        println!("LabelSet addr                     ls          sets a text label at target address");
        println!("LabelRemove addr                  lr          removes a text label at target address");
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