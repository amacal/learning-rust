#[macro_use]
extern crate criterion;
use criterion::Criterion;


fn loop_chunk_count(haystack: &[u8], needle: u8) -> usize {
    let mut count: usize = 0;

    for i in 0..haystack.len() {
        if haystack[i] == needle {
            count += 1;
        }
    }

    count
}

fn criterion_benchmark(c: &mut Criterion) {
    let chunk = b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let mut haystack = Vec::with_capacity(16384);
    let needle = b'a';

    for _ in 0..255 {
        haystack.extend(chunk);
    }

    c.bench_function("loop", |b| b.iter(|| loop_chunk_count(haystack.as_ref(), needle)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
