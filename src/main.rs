mod arena;
mod avl;

use arena::NodeArray;
use avl::{AvlForest, AvlNode};

#[derive(Copy, Clone)]
struct TestNode {
    key: i32,
    value: i32,
    left: u32,
    right: u32,
}

impl AvlNode for TestNode {
    type KeyType = i32;
    type ValueType = i32;

    fn new(key: Self::KeyType, value: Self::ValueType) -> Self {
        TestNode { key, value, left: 0, right: 0 }
    }

    fn get_key(&self) -> Self::KeyType {
        self.key
    }

    fn get_value(&self) -> Self::ValueType {
        self.value
    }

    fn get_left(&self) -> u32 {
        self.left
    }

    fn set_left(&mut self, left: u32) {
        self.left = left;
    }

    fn get_right(&self) -> u32 {
        self.right
    }

    fn set_right(&mut self, right: u32) {
        self.right = right;
    }
}

fn main() {
    let arena: NodeArray<TestNode, 10> = NodeArray::new();
    let mut forest = AvlForest::new(arena);

    let root = forest.append(10, 100).unwrap();
    let _ = forest.insert(root, 20, 200).unwrap();
    let _ = forest.insert(root, 5, 50).unwrap();

    println!("AVL Forest created with nodes inserted.");
}
