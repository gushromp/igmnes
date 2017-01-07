use core::CpuFacade;
use core::debugger::Debugger;
use core::debugger::command::Command;
use core::cpu::Cpu;
use std::io::{self, Read, Write};

pub struct TerminalDebugger {
    cpu: Box<Cpu>,
}

impl TerminalDebugger {
    pub fn new(cpu: Box<Cpu>) -> TerminalDebugger {
        TerminalDebugger {
            cpu: cpu
        }
    }

    fn execute_command(&mut self, command: Command) {}
}

impl Debugger for TerminalDebugger {
    fn start_listening(&mut self) {
        let pc = self.cpu.state().reg_pc;

        let mut stdout = io::stdout();

        while true {
            print!("0x{:X} -> ", pc);
            stdout.flush().unwrap();

            let mut line = String::new();
            let stdin = io::stdin();
            stdin.read_line(&mut line);

            let command = Command::parse(&line);

            match command {
                Ok(command) => println!("{:#?}", command),
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

    fn step(&mut self) -> u8 {
        self.cpu.step()
    }
}