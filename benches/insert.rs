use criterion::{Criterion, criterion_group, criterion_main};
use rand::RngCore;
use std::collections::BTreeMap;

use avl::{AvlForest, NodeArray, AvlNode};

const COUNT: usize = 100_000;
const ARENA_SIZE: usize = COUNT + 2;

fn btree_insert(c: &mut Criterion) {
    let mut rng = rand::rng();
    let mut group = c.benchmark_group("insert");

    group.bench_function("btree", |b| {
        b.iter(|| {
            let mut map = BTreeMap::new();

            for _ in 0..COUNT {
                let key = rng.next_u32();
                let value = rng.next_u32();

                map.insert(key, value);
            }
        });
    });

    group.finish();
}

fn arena_insert(c: &mut Criterion) {
    let mut rng = rand::rng();
    let mut group = c.benchmark_group("insert");

    group.bench_function("arena", |b| {
        b.iter(|| {
            let arena: NodeArray<AvlNode<(), _, _, ()>, ARENA_SIZE> = NodeArray::new();
            let mut forest = AvlForest::new(arena);
            let root = forest.insert_tree(()).unwrap();

            for _ in 0..COUNT {
                let key = rng.next_u32();
                let value = rng.next_u32();
                forest.insert_element(root, key, value).unwrap();
            }
        });
    });

    group.finish();
}

criterion_group!(benches, btree_insert, arena_insert);
criterion_main!(benches);
