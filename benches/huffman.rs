#[macro_use]
extern crate criterion;

use criterion::Criterion;
use rand::prelude::*;

use deflate::{BitReader, BitStream, BitStreamBitwise, BitStreamExt};
use deflate::{HuffmanDecoder, HuffmanTableIterative, HuffmanTableLookup};

fn iterative_benchmark(criterion: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(64);
    let mut src = Box::new([0; 131072]);

    for i in 0..131072 {
        src[i] = rng.gen();
    }

    criterion.bench_function("huffman-iterative", |b| {
        b.iter(|| {
            let mut total: u64 = 0;
            let mut bitstream = BitStreamBitwise::<131072, 1048576>::new();

            let size = 131072;
            let mut offset = 0;

            while offset < size {
                if let Some(size) = bitstream.appendable() {
                    let available = std::cmp::min(size - offset, size);

                    bitstream.append(&src[offset..offset + available]).unwrap();
                    offset += available;
                }

                let table: HuffmanTableIterative<4, 6> = HuffmanTableIterative::new([0, 1, 0, 2, 3, 2]).unwrap();
                let mut reader = bitstream.as_unchecked();

                while reader.available() >= 65536 * 4 + 4 {
                    for _ in 0..65536 {
                        total += table.decode(&mut reader).unwrap() as u64;
                    }
                }

                while reader.available() >= 1024 * 4 + 4 {
                    for _ in 0..1024 {
                        total += table.decode(&mut reader).unwrap() as u64;
                    }
                }

                while reader.available() > 4 {
                    total += table.decode(&mut reader).unwrap() as u64;
                }
            }

            assert_eq!(total, 1747026);
        })
    });
}

fn lookup_benchmark(criterion: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(64);
    let mut src = Box::new([0; 131072]);

    for i in 0..131072 {
        src[i] = rng.gen();
    }

    criterion.bench_function("huffman-lookup", |b| {
        b.iter(|| {
            let mut total: u64 = 0;
            let mut bitstream = BitStreamBitwise::<131072, 1048576>::new();

            let size = 131072;
            let mut offset = 0;

            while offset < size {
                if let Some(size) = bitstream.appendable() {
                    let available = std::cmp::min(size - offset, size);

                    bitstream.append(&src[offset..offset + available]).unwrap();
                    offset += available;
                }

                let table: HuffmanTableLookup<4, 6> = HuffmanTableLookup::new([0, 1, 0, 2, 3, 2]).unwrap();
                let mut reader = bitstream.as_unchecked();

                while reader.available() >= 65536 * 4 + 4 {
                    for _ in 0..65536 {
                        total += table.decode(&mut reader).unwrap() as u64;
                    }
                }

                while reader.available() >= 1024 * 4 + 4 {
                    for _ in 0..1024 {
                        total += table.decode(&mut reader).unwrap() as u64;
                    }
                }

                while reader.available() > 4 {
                    total += table.decode(&mut reader).unwrap() as u64;
                }
            }

            assert_eq!(total, 1747026);
        })
    });
}

criterion_group!(benches, iterative_benchmark, lookup_benchmark);
criterion_main!(benches);
