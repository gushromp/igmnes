use std::fmt::{Debug, Formatter};
use std::fs;
use core::cpu::Cpu;
use core::instructions::Instruction;
use core::debugger::disassembler::disassemble;
use std::path::Path;
use core::memory::MemMapped;
use core::ppu::Ppu;

#[derive(Default)]
pub struct Trace {
    pub cpu_trace: Option<String>,
    pub ppu_trace: Option<String>,
    pub cycle_count: u64,
}

impl Debug for Trace {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        let mut trace_line = String::new();
        if let Some(ref cpu_trace) = self.cpu_trace {
            trace_line.push_str(&format!("{}", cpu_trace));
        }
        if let Some(ref ppu_trace) = self.ppu_trace {
            trace_line.push_str(&format!(" {}", ppu_trace));
        }
        if !trace_line.is_empty() {
            write!(fmt, "{}", trace_line).unwrap();
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct Tracer {
    is_enabled: bool,

    current_trace: Option<Trace>,
    traces: Vec<String>,

}

impl Tracer {
    pub fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    pub fn set_enabled(&mut self, is_enabled: bool) {
        self.is_enabled = is_enabled;
    }
    pub fn add_cpu_trace(&mut self, cpu_state: &Cpu, mem_map: &mut dyn MemMapped) {
        if let Some(ref mut current_trace) = self.current_trace {
            let instruction = Instruction::decode(mem_map, cpu_state.reg_pc);

            let trace_line = match instruction {
                Ok(mut instr) => {
                    disassemble(instr.address, &mut instr, cpu_state, mem_map).unwrap_or("INVALID".to_string())
                }
                Err(e) => e.to_string()
            };
            current_trace.cpu_trace = Some(trace_line);
        }
    }

    pub fn add_ppu_trace(&mut self, ppu: &Ppu) {
        if let Some(ref mut current_trace) = self.current_trace {
            let trace_line = format!("{}", ppu);
            current_trace.ppu_trace = Some(trace_line);
        }
    }

    pub fn set_cycle_count(&mut self, cycle_count: u64) {
        if let Some(ref mut current_trace) = self.current_trace {
            current_trace.cycle_count = cycle_count
        }
    }

    pub fn start_new_trace(&mut self) {
        if let Some(ref trace) = self.current_trace {
            self.traces.push(format!("{:#?}", trace));
        }
        let new_trace = Trace::default();
        self.current_trace = Some(new_trace);
    }

    pub fn write_to_file(&self, file_path: &Path) {
        fs::write(file_path, &self.traces.join("\n")).unwrap();
    }

    pub fn has_traces(&mut self) -> bool {
        !self.traces.is_empty()
    }
    pub fn clear_traces(&mut self) {
        self.traces.clear();
    }
}