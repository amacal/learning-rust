mod arena;
mod avl;

use core::hash;
use std::time::Instant;

use arena::NodeArray;
use avl::{AvlForest, AvlNode, AvlAugment};
use rand::RngCore;

type Tag = ();
type Key = u32;
type Value = ();
type Augmented = (u32, u32);

impl AvlAugment<Value, Augmented> for Augmented {
    fn augment(_: &Value, left: Option<&Augmented>, right: Option<&Augmented>) -> Augmented {
        let (left_height, left_count) = left.unwrap_or(&(0, 0));
        let (right_height, right_count) = right.unwrap_or(&(0, 0));
        (1 + std::cmp::max(left_height, right_height), 1 + left_count + right_count)
    }
}

fn main() {
    let arena: NodeArray<AvlNode<Tag, Key, Value, Augmented>, 1_000_000_000> = NodeArray::new();
    let mut forest = AvlForest::new(arena);

    let mut last = Instant::now();
    let mut removals = [0; 1000];
    let tree = forest.insert_tree(()).unwrap();
    let (mut rng, mut counter) = (rand::rng(), 0);

    loop {
        let key = rng.next_u32();
        if forest.insert_element(tree, key, ()).is_none() {
            let height = forest.height(tree);
            let root = forest.root(tree);
            let (perfect, count) = forest.augmented(root);

            println!("Failed to insert value: {}", key);
            println!("Height: {}, Count: {}, Perfect: {}", height, count, perfect);
            break;
        }

        if counter % 1000 == 0 {
            unsafe { *removals.get_unchecked_mut(counter / 1000 % 1000) = key };
        }

        if counter % 1000000 == 0 {
            for key in removals.iter() {
                forest.remove_element(tree, *key);
            }

            let height = forest.height(tree);
            let root = forest.root(tree);

            let (perfect, count) = forest.augmented(root);
            let elapsed = Instant::now().duration_since(last);

            println!("Height: {}, Count: {}, Perfect: {}, Elapsed: {:.3?}", height, count, perfect, elapsed);
            last = Instant::now();
        }

        counter += 1;
    }
}
