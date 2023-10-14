#[macro_use]
extern crate criterion;

use criterion::Criterion;
use rand::prelude::*;

fn copy_within(data: &mut [u8], source: usize, destination: usize, length: usize) {
    data.copy_within(source..source + length, destination);
}

fn criterion_benchmark(criterion: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(64);
    let length: u32 = rng.gen_range(40_000..50_000);

    let size = rng.gen_range(100_000..200_000);
    let mut data = Vec::with_capacity(size + 10_000);

    for _ in 0..size {
        let byte: u8 = rng.gen();
        data.push(byte);
    }

    let data = &mut data[..];
    criterion.bench_function("copy_within", |b| {
        b.iter_batched(
            || {
                let source: u32 = rng.gen_range(50_000..100_000);
                let destination: u32 = source + 280;

                (source, destination)
            },
            |(source, destination)| copy_within(data, source as usize, destination as usize, length as usize),
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
