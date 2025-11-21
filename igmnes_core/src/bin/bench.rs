use igmnes_core::BusOps;
use igmnes_core::{debug::Tracer, Core};
use std::{path::Path, time::Instant};

const CYCLES_TO_RUN: usize = 20_000_000;

fn main() {
    let rom_path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../test_roms/stretch.nes"
    ))
    .canonicalize()
    .unwrap();

    let mut tracer = Tracer::default();
    let mut core = Core::load_rom(&rom_path).unwrap();

    for (enable_ppu, enable_apu) in [(false, false), (false, true), (true, false), (true, true)] {
        core.hard_reset();
        println!("Running {} cycles...", CYCLES_TO_RUN);
        println!("PPU: {}   |   APU: {}", enable_ppu, enable_apu);
        let start_time = Instant::now();
        bench(
            &mut core,
            &mut tracer,
            CYCLES_TO_RUN,
            enable_ppu,
            enable_apu,
        );
        let cur_time = Instant::now();

        let seconds = cur_time.duration_since(start_time).as_millis() as f64 / 1000.0;
        println!("Cycles: {}", core.cpu_cycles());
        println!("Seconds: {}", seconds);
        if seconds > 0.0 {
            println!(
                "Cycles per second: {}",
                (core.cpu_cycles() as f64 / seconds).floor()
            );
        }
        println!();
    }
}

fn bench(
    core: &mut Core,
    tracer: &mut Tracer,
    max_cycles: usize,
    enable_ppu: bool,
    enable_apu: bool,
) {
    while core.cpu_cycles() < max_cycles as u64 {
        step(core, tracer, enable_ppu, enable_apu);
    }
}

fn step(core: &mut Core, tracer: &mut Tracer, enable_ppu: bool, enable_apu: bool) {
    tracer.start_new_trace();

    let current_cycle_count = core.bus.cpu().cycle_count;

    let mut nmi = false;
    if enable_ppu {
        nmi = core.bus.step_ppu(current_cycle_count, tracer);
        if nmi {
            core.bus.ppu().clear_nmi();
            core.bus.nmi();
        }
    }

    if enable_apu {
        let irq = core.bus.step_apu(current_cycle_count);
        if irq && !nmi {
            core.bus.irq();
        }
    }

    let dma = core.bus.dma().is_dma_active();
    if dma {
        core.bus.step_dma();
        core.bus.cpu().dma();
    }

    core.bus.step_cpu(tracer).unwrap();

    if enable_ppu {
        if core.bus.ppu().should_suppress_nmi() {
            core.bus.cpu().suppress_interrupt();
        } else if core.bus.ppu().nmi_pending {
            // Needs PPU to track it's own cycles in order to be more accurate
            core.bus.ppu().clear_nmi();
            core.bus.nmi();
        }
    }
}
