use std::path::Path;

use igmnes_core::debug::Tracer;
use igmnes_core::Core;

fn main() {
    divan::main();
}

#[divan::bench(args = [1_000_000, 5_000_000, 10_000_000], sample_count = 10)]
fn stretch(max_cycles: usize) {
    let rom_path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../test_roms/stretch.nes"
    ))
    .canonicalize()
    .unwrap();

    let mut tracer = Tracer::default();
    let mut core = Core::load_rom(&rom_path).unwrap();

    while core.cpu_cycles() < max_cycles as u64 {
        core.step(&mut tracer);
    }
}
