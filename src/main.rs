mod arena;
mod avl;

use arena::NodeArray;
use avl::{AvlForest, AvlNode};

fn main() {
    let arena: NodeArray<AvlNode<i32, i32>, 2_000_000_000> = NodeArray::new();
    let mut forest = AvlForest::new(arena);

    let root = forest.append().unwrap();
    let _ = forest.insert(root, 10, 100).unwrap();
    let _ = forest.insert(root, 20, 200).unwrap();
    let _ = forest.insert(root, 5, 50).unwrap();

    println!("AVL Forest created with nodes inserted.");
    loop {};
}
