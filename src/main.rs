mod arena;
mod avl;

use std::{collections::BTreeMap, time::Instant, u32};

use arena::NodeArray;
use avl::{AvlForest, AvlNode, AvlAugment};
use rand::RngCore;

type Tag = ();
type Key = u32;
type Value = ();
type Augmented = (u32, u32);

impl AvlAugment<Key, Value, Augmented> for Augmented {
    fn augment(_: &Key, _: &Value, left: Option<&Augmented>, right: Option<&Augmented>) -> Augmented {
        let (left_height, left_count) = left.unwrap_or(&(0, 0));
        let (right_height, right_count) = right.unwrap_or(&(0, 0));
        (1 + std::cmp::max(left_height, right_height), 1 + left_count + right_count)
    }
}

fn main() {
    run_avl();
}

fn run_avl() {
    let arena: NodeArray<AvlNode<Tag, Key, Value, ()>, 1_000_000_000> = NodeArray::new();
    let mut forest = AvlForest::new(arena);

    let mut last = Instant::now();
    let mut removals = [0; 1000];
    let tree = forest.insert_tree(()).unwrap();
    let (mut rng, mut counter) = (rand::rng(), 0);
    let mut center = rng.next_u32()  % (u32::MAX / 4);

    loop {
        let key = rng.next_u32() % (u32::MAX / 4) + center;

        if forest.insert_element(tree, key, ()).is_none() {
            let height = forest.height(tree);
            println!("Counter: {}, Height: {}", counter, height);
            break;
        }

        if counter % 1000 == 0 {
            unsafe { *removals.get_unchecked_mut(counter / 1000 % 1000) = key };
        }

        if counter % 10_000_000 == 0 {
            let height = forest.height(tree);
            let elapsed = Instant::now().duration_since(last);

            println!("Counter: {}, Height: {}, Elapsed: {:.3?}", counter, height, elapsed);
            last = Instant::now();
            center = rng.next_u32() % (u32::MAX / 4);
        }

        if counter % 1_000_000 == 0 {
            for key in removals.iter() {
                forest.remove_element(tree, *key);
                counter += 1;
            }
        }

        counter += 1;
    }
}

fn run_avl_augmented() {
    let arena: NodeArray<AvlNode<Tag, Key, Value, Augmented>, 1_000_000_000> = NodeArray::new();
    let mut forest = AvlForest::new(arena);

    let mut last = Instant::now();
    let mut removals = [0; 1000];
    let tree = forest.insert_tree(()).unwrap();
    let (mut rng, mut counter) = (rand::rng(), 0);
    let mut center = rng.next_u32()  % (u32::MAX / 4);

    loop {
        let key = rng.next_u32() % (u32::MAX / 4) + center;

        if forest.insert_element(tree, key, ()).is_none() {
            let height = forest.height(tree);
            let root = forest.root(tree);
            let (perfect, count) = forest.augmented(root);

            println!("Failed to insert value: {}", key);
            println!("Counter: {}, Height: {}, Count: {}, Perfect: {}", counter, height, count, perfect);
            break;
        }

        if counter % 1000 == 0 {
            unsafe { *removals.get_unchecked_mut(counter / 1000 % 1000) = key };
        }

        if counter % 10_000_000 == 0 {
            let height = forest.height(tree);
            let root = forest.root(tree);

            let (perfect, count) = forest.augmented(root);
            let elapsed = Instant::now().duration_since(last);

            println!("Counter: {}, Height: {}, Count: {}, Perfect: {}, Elapsed: {:.3?}", counter, height, count, perfect, elapsed);
            last = Instant::now();
            center = rng.next_u32() % (u32::MAX / 4);
        }

        if counter % 1_000_000 == 0 {
            for key in removals.iter() {
                forest.remove_element(tree, *key);
                counter += 1;
            }
        }

        counter += 1;
    }
}

fn run_btree() {
    let mut tree = BTreeMap::<Key, Value>::new();
    let mut last = Instant::now();
    let mut removals = [0; 1000];
    let (mut rng, mut counter) = (rand::rng(), 0);
    let mut center = rng.next_u32() % (u32::MAX / 4);

    while tree.len() < 1_000_000_000 {
        let key = rng.next_u32() % (u32::MAX / 4) + center;
        tree.insert(key, ());

        if counter % 1000 == 0 {
            removals[counter / 1000 % 1000] = key;
        }

        if counter % 10_000_000 == 0 {
            let elapsed = Instant::now().duration_since(last);
            println!("Counter: {}, Size: {}, Elapsed: {:.3?}", counter, tree.len(), elapsed);
            last = Instant::now();
            center = rng.next_u32() % (u32::MAX / 4);
        }

        if counter % 1_000_000 == 0 {
            for &key in &removals {
                tree.remove(&key);
                counter += 1;
            }
        }

        counter += 1;
    }
}
