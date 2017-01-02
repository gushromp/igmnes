mod apu;
mod cpu;
mod memmap;
mod instructions;

use self::apu::Apu;
use self::cpu::Cpu;

// 2A03 (NTSC) and 2A07 (PAL) emulation
// contains CPU (nearly identical to MOS 6502) part and APU part
#[derive(Debug, Default)]
pub struct Core {
    cpu: Cpu,
    apu: Apu,
}

impl Core {
    pub fn new() -> Core {
        let core = Core {
            cpu: Cpu::new(),
            apu: Apu,
        };

        core
    }
}