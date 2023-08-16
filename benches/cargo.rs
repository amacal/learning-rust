#[macro_use]
extern crate criterion;
use criterion::Criterion;
use bitstream_io::{BitReader, LittleEndian, BitRead};

pub fn count_bits(data: &[u8]) {
    let mut count = 0;
    let mut bitstream = BitReader::endian(data, LittleEndian);

    while let std::io::Result::Ok(_) = bitstream.read_bit() {
        count += 1;
    }

    assert_eq!(count, 512);
}

fn criterion_benchmark(c: &mut Criterion) {
    let data = b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".as_ref();
    c.bench_function("cargo", |b| b.iter(|| count_bits(data)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
