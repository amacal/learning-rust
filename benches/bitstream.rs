#[macro_use]
extern crate criterion;

use criterion::Criterion;
use rand::prelude::*;

use deflate::{BitStream, BitStreamBitwise, BitStreamBytewise};

fn bytewise_benchmark(criterion: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(64);
    let mut src = Box::new([0; 1048576]);

    for i in 0..src.len() {
        src[i] = rng.gen();
    }

    criterion.bench_function("bitstream-bytewise", |b| {
        b.iter(|| {
            let mut total: u64 = 0;
            let mut bitstream = BitStreamBytewise::<131072>::new();

            let size = src.len();
            let mut offset = 0;

            while offset < size {
                if let Some(available) = bitstream.appendable() {
                    let available = std::cmp::min(size - offset, available);

                    bitstream.append(&src[offset..offset + available]).unwrap();
                    offset += available;
                }

                while bitstream.available() >= 65536 * 4 + 1024 {
                    for _ in 0..65536 {
                        total += bitstream.next_bit_unchecked() as u64;
                        total += bitstream.next_bits_unchecked(3) as u64;
                    }
                }

                while bitstream.available() > 0 {
                    match bitstream.next_bit() {
                        None => break,
                        Some(bit) => total += bit as u64,
                    };

                    match bitstream.next_bits(3) {
                        None => break,
                        Some(bits) => total += bits as u64,
                    };
                }
            }

            assert!(total == 8387666);
        })
    });
}

fn bitwise_benchmark(criterion: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(64);
    let mut src = Box::new([0; 1048576]);

    for i in 0..src.len() {
        src[i] = rng.gen();
    }

    criterion.bench_function("bitstream-bitwise", |b| {
        b.iter(|| {
            let mut total: u64 = 0;
            let mut bitstream = BitStreamBitwise::<131072, 1048576>::new();

            let size = src.len();
            let mut offset = 0;

            while offset < size {
                if let Some(available) = bitstream.appendable() {
                    let available = std::cmp::min(size - offset, available);

                    bitstream.append(&src[offset..offset + available]).unwrap();
                    offset += available;
                }

                while bitstream.available() >= 65536 * 4 + 1024 {
                    for _ in 0..65536 {
                        total += bitstream.next_bit_unchecked() as u64;
                        total += bitstream.next_bits_unchecked(3) as u64;
                    }
                }

                while bitstream.available() > 0 {
                    match bitstream.next_bit() {
                        None => break,
                        Some(bit) => total += bit as u64,
                    };

                    match bitstream.next_bits(3) {
                        None => break,
                        Some(bits) => total += bits as u64,
                    };
                }
            }

            assert!(total == 8387666);
        })
    });
}

criterion_group!(benches, bytewise_benchmark, bitwise_benchmark);
criterion_main!(benches);
