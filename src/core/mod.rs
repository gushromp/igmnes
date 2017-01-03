mod apu;
mod cpu;
mod memory;
mod instructions;

use self::memory::*;
use self::apu::Apu;
use self::cpu::Cpu;

// 2A03 (NTSC) and 2A07 (PAL) emulation
// contains CPU (nearly identical to MOS 6502) part and APU part
#[derive(Default)]
pub struct Core {

    cpu: Cpu,

}

impl Core {
    pub fn new() -> Core {
        let core = Core {
            cpu: Cpu::new(),

        };

        core
    }
}