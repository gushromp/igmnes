use std::time::Duration;
use std::{hint::black_box, path::Path};

use criterion::{criterion_group, criterion_main, Criterion};
use igmnes_core::debug::Tracer;
use igmnes_core::Core;

fn stretch(core: &mut Core, tracer: &mut Tracer, max_cycles: usize) -> u64 {
    let mut res = 0;
    while core.cpu_cycles() < max_cycles as u64 {
        core.step(tracer);
        res = core.cpu_cycles();
    }
    res
}

fn benchmark(c: &mut Criterion) {
    let rom_path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../test_roms/stretch.nes"
    ))
    .canonicalize()
    .unwrap();

    let mut core = Core::load_rom(&rom_path).unwrap();

    let mut tracer = Tracer::default();

    let mut group = c.benchmark_group("bench");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(1));

    group.bench_function("stretch 10M cycles", |b| {
        b.iter(|| black_box(stretch(&mut core, &mut tracer, 10_000_000)))
    });
    group.bench_function("stretch 20M cycles", |b| {
        b.iter(|| black_box(stretch(&mut core, &mut tracer, 20_000_000)))
    });
    group.bench_function("stretch 40M cycles", |b| {
        let mut max = 0;
        b.iter(|| max += black_box(stretch(&mut core, &mut tracer, 40_000_000)));
        println!("{}", max);
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
