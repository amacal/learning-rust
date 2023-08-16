#[macro_use]
extern crate criterion;
use criterion::Criterion;

pub struct BitStream<'a> {
    data: &'a [u8],
    offset: usize,
    offset_bit: u8,
}

impl<'a> BitStream<'a> {
    pub fn try_from(data: &'a [u8]) -> Option<Self> {
        Some(Self {
            data: data,
            offset: 0,
            offset_bit: 0x01,
        })
    }

    pub fn next_bit(&mut self) -> Option<u8> {
        let bit_set = match self.data.get(self.offset) {
            None => return None,
            Some(value) => value & self.offset_bit
        };
     
        self.offset_bit = (self.offset_bit << 1) | (self.offset_bit >> 7);
        
        if self.offset_bit == 0x01 {
            self.offset += 1;
        }

        Some(if bit_set != 0 { 1 }  else { 0 })
    }
}

pub fn count_bits(data: &[u8]) {
    let mut count = 0;
    let mut bitstream = BitStream::try_from(data).unwrap();

    while let Some(_) = bitstream.next_bit() {
        count += 1;
    }

    assert_eq!(count, 512);
}

fn criterion_benchmark(c: &mut Criterion) {
    let data = b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".as_ref();
    c.bench_function("slice", |b| b.iter(|| count_bits(data)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
