mod arena;
mod avl;

use arena::NodeArray;
use avl::{AvlForest, AvlNode, AvlAugment};

impl AvlAugment<i32, i32> for i32 {
    fn augment(value: &i32, left: Option<&i32>, right: Option<&i32>) -> i32 {
        value + left.unwrap_or(&0) + right.unwrap_or(&0)
    }
}

fn main() {
    let arena: NodeArray<AvlNode<i32, i32, i32, i32>, 2_000_000_000> = NodeArray::new();
    let mut forest = AvlForest::debug(arena);

    let root = forest.insert_tree(13).unwrap();
    let _ = forest.insert_element(root, 10, 100).unwrap();
    let _ = forest.insert_element(root, 20, 200).unwrap();
    let _ = forest.insert_element(root, 5, 50).unwrap();

    forest.print(root);

    println!("AVL Forest created with nodes inserted.");
    loop {};
}
