#[macro_use]
extern crate criterion;

use criterion::Criterion;
use rand::prelude::*;

extern "C" {
    fn extract_bits(dst: *mut u8, src: *const u8, count: usize);
}

#[repr(align(32))]
struct AlignedArray32<const T: usize>([u8; T]);

fn criterion_benchmark(criterion: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(64);
    let mut src = Box::new(AlignedArray32([0; 131072]));
    let mut dst = Box::new(AlignedArray32([0; 1048576]));

    for i in 0..131072 {
        src.0[i] = rng.gen();
    }

    criterion.bench_function("simd", |b| {
        b.iter(|| unsafe {
            let src = src.0.as_ptr().add(1);
            let dst = dst.0.as_mut_ptr().add(8);
    
            extract_bits(dst, src, 131070);
        })
    });

    let mut sum: u64 = 0;
    for i in 0..1048576 {
        sum += if i % 7 == 0 { dst.0[i] } else { 0 } as u64;
    }

    println!("simd: {}", sum);
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
