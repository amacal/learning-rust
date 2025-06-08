mod arena;
mod avl;

use std::cmp::Ordering;

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

    fn get_balance(&self) -> Ordering {
        let left = self.left & 0x80000000;
        let right = self.right & 0x80000000;

        if left == right {
            return Ordering::Equal;
        }

        if left > 0 {
            return Ordering::Less;
        }

        return Ordering::Greater;
    }

    fn set_balance(&mut self, balance: Ordering) {
        match balance {
            Ordering::Equal => {
                self.left &= 0x7fffffff;
                self.right &= 0x7fffffff;
            }
            Ordering::Less => {
                self.left |= 0x80000000;
                self.right &= 0x7fffffff;
            }
            Ordering::Greater => {
                self.left &= 0x7fffffff;
                self.right |= 0x80000000;
            }
        }
    }

    fn get_left(&self) -> u32 {
        self.left & 0x7fffffff
    }

    fn set_left(&mut self, left: u32) {
        self.left = left | self.left & 0x80000000;
    }

    fn get_right(&self) -> u32 {
        self.right & 0x7fffffff
    }

    fn set_right(&mut self, right: u32) {
        self.right = right | self.right & 0x80000000;
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
