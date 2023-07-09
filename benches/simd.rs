#[macro_use]
extern crate criterion;
extern crate packed_simd;

use std::mem;
use criterion::Criterion;
use packed_simd::{u8x64, FromCast};


unsafe fn simd_u8x64_from_offset(slice: &[u8], offset: usize) -> u8x64 {
    u8x64::from_slice_unaligned_unchecked(slice.get_unchecked(offset..))
}

fn simd_sum_x64(u8s: &u8x64) -> usize {
    let mut store = [0; mem::size_of::<u8x64>()];
    u8s.write_to_slice_unaligned(&mut store);
    store.iter().map(|&e| e as usize).sum()
}

fn simd_chunk_count(haystack: &[u8], needle: u8) -> usize {
    unsafe {
        let needles_x64 = u8x64::splat(needle);
        let loops = haystack.len() / 64;

        let mut counts = u8x64::splat(0);
        let mut offset = 0;

        for _ in 0..loops {
            counts -= u8x64::from_cast(simd_u8x64_from_offset(haystack, offset).eq(needles_x64));
            offset += 64;
        }

        simd_sum_x64(&counts)
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let chunk =b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let mut haystack = Vec::with_capacity(16384);
    let needle = b'a';

    for _ in 0..255 {
        haystack.extend(chunk);
    }

    c.bench_function("simd", |b| b.iter(|| simd_chunk_count(haystack.as_ref(), needle)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
