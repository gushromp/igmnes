use crate::core::cpu::Cpu;
use crate::core::debugger::disassembler::disassemble;
use crate::core::instructions::Instruction;
use crate::core::memory::MemMapped;
use crate::core::ppu::Ppu;
use std::fmt::{Debug, Formatter};
use std::fs;
use std::path::Path;

#[derive(Default)]
pub struct Trace {
    pub cpu_trace: Option<String>,
    pub ppu_trace: Option<String>,
    pub tick_time_ns: Option<i64>,
}

impl Debug for Trace {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        let mut trace_line = String::new();
        if let Some(cpu_trace) = &self.cpu_trace {
            trace_line.push_str(&format!("{}", cpu_trace));
        }
        if let Some(ppu_trace) = &self.ppu_trace {
            trace_line.push_str(&format!(" {}", ppu_trace));
        }
        if let Some(tick_time_ns) = &self.tick_time_ns {
            trace_line.push_str(&format!(" tck:{}", tick_time_ns));
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
    pub fn add_cpu_trace(&mut self, cpu_state: &Cpu, mem_map: &mut impl MemMapped) {
        if let Some(ref mut current_trace) = self.current_trace {
            mem_map.set_is_mutating_read(false);
            let instruction = Instruction::decode(mem_map, cpu_state.reg_pc);

            let trace_line = match instruction {
                Ok(mut instr) => {
                    format!(
                        "{}\t{}",
                        disassemble(instr.address, &mut instr, cpu_state, mem_map)
                            .unwrap_or("INVALID".to_string()),
                        cpu_state
                    )
                }
                Err(e) => e.to_string(),
            };
            current_trace.cpu_trace = Some(trace_line);
            mem_map.set_is_mutating_read(true);
        }
    }

    pub fn add_ppu_trace(&mut self, ppu: &Ppu) {
        if let Some(ref mut current_trace) = self.current_trace {
            let trace_line = format!("{}", ppu);
            current_trace.ppu_trace = Some(trace_line);
        }
    }

    pub fn add_tick_time_ns(&mut self, tick_time_ns: Option<i64>) {
        if let Some(ref mut current_trace) = self.current_trace {
            current_trace.tick_time_ns = tick_time_ns;
        }
    }

    pub fn start_new_trace(&mut self) {
        if let Some(ref trace) = self.current_trace {
            if trace.cpu_trace.is_some() && trace.ppu_trace.is_some() {
                self.traces.push(format!("{:#?}", trace));
            }
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
