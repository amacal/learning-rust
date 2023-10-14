#[macro_use]
extern crate criterion;

use criterion::Criterion;
use rand::prelude::*;
use std::arch::x86_64;

unsafe fn with_simd(src: *const u8, dst: *mut u8, length: usize) {
    let mut i = 0;
    let size = 16;

    while i + size <= length {
        let source = src.add(i) as *const x86_64::__m128i;
        let destination = dst.add(i) as *mut x86_64::__m128i;

        let data = x86_64::_mm_loadu_si128(source);
        x86_64::_mm_storeu_si128(destination, data);

        i += size;
    }

    while i < length && length - i >= 4 {
        let src = src.add(i) as *const u32;
        let dst = dst.add(i) as *mut u32;

        *dst = *src;
        i += 4;
    }

    while i < length && length - i < 16 {
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

    unsafe {
        let data = &mut data[..];
        let source = data.as_ptr().add(source as usize);
        let destination = data.as_mut_ptr().add(destination as usize);

        criterion.bench_function("with_simd", |b| {
            b.iter(|| {
                with_simd(source, destination, length as usize);
            })
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
