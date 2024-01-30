use std::fmt::{Debug, Formatter};
use std::fs;
use core::cpu::Cpu;
use core::instructions::Instruction;
use core::debugger::disassembler::disassemble;
use std::path::Path;
use core::memory::MemMap;

pub struct CpuTrace {
    pub cpu_state: Cpu,
    pub instruction: Instruction,
    pub mem_map: MemMap,
}

#[derive(Default)]
pub struct Trace {
    pub cpu_trace: Option<CpuTrace>,
    pub cycle_count: u64,
}

impl Debug for Trace {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        if let Some(ref cpu_trace) = self.cpu_trace {
            let mut instruction = cpu_trace.instruction.clone();
            let disassembly = disassemble(instruction.address, &mut instruction, &cpu_trace.cpu_state, &cpu_trace.mem_map).unwrap_or("INVALID".to_owned());

            write!(fmt, "{}\t{}\tCYC: {}",
                   disassembly,
                   cpu_trace.cpu_state,
                   self.cycle_count
            ) .unwrap();
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct Tracer<'a> {
    current_trace: Option<Trace>,
    traces: Vec<String>,
    mem_map: Option<&'a MemMap>,
}

impl<'a> Tracer<'a> {
    pub fn set_mem_map(&mut self, mem_map: &'a MemMap) {
        self.mem_map = Some(mem_map);
    }

    pub fn set_cpu_trace(&mut self, cpu_trace: CpuTrace) {
        if let Some(ref mut current_trace) = self.current_trace {
            current_trace.cpu_trace = Some(cpu_trace);
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
        let mut new_trace = Trace::default();
        self.current_trace = Some(new_trace);
    }

    pub fn write_to_file(&self, file_path: &Path) {
        fs::write(file_path, &self.traces.join("\n")).unwrap();
    }

    pub fn clear_traces(&mut self) {
        self.traces.clear();
    }
}