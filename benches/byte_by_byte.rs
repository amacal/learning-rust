#[macro_use]
extern crate criterion;

use criterion::Criterion;
use rand::prelude::*;

#[inline(always)]
unsafe fn byte_by_byte(src: *const u8, dst: *mut u8, length: usize) {
    let mut i = 0;

    while i < length {
        *dst.add(i) = *src.add(i);
        i += 1;
    }
}

fn criterion_benchmark(criterion: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(64);
    let length: u32 = rng.gen_range(40_000..50_000);

    let source: u32 = rng.gen_range(50_000..100_000);
    let destination: u32 = source + 280;

    let size = rng.gen_range(100_000..200_000);
    let mut data = Vec::with_capacity(size + 10_000);

    for _ in 0..size {
        let byte: u8 = rng.gen();
        data.push(byte);
    }

    let data = &mut data[..];
    criterion.bench_function("byte_by_byte", |b| {
        b.iter(|| {
            unsafe {
                let source = data.as_ptr().add(source as usize);
                let destination = data.as_mut_ptr().add(destination as usize);
        
                byte_by_byte(source, destination, length as usize);
            }
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
