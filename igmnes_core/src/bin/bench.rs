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

    println!("Running {} cycles...", CYCLES_TO_RUN);
    let start_time = Instant::now();
    bench(&mut core, &mut tracer, CYCLES_TO_RUN);
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
}

fn bench(core: &mut Core, tracer: &mut Tracer, max_cycles: usize) {
    while core.cpu_cycles() < max_cycles as u64 {
        core.step(tracer);
    }
}
