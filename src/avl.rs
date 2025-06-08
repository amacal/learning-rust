use super::arena::NodeArena;
use std::{cmp::Ordering, marker::PhantomData};

pub trait AvlNode {
    type KeyType: Copy + PartialOrd;
    type ValueType: Copy;

    fn new(key: Self::KeyType, value: Self::ValueType) -> Self;

    fn get_key(&self) -> Self::KeyType;
    fn get_value(&self) -> Self::ValueType;

    fn get_balance(&self) -> Ordering;
    fn set_balance(&mut self, balance: Ordering);

    fn get_left(&self) -> u32;
    fn set_left(&mut self, left: u32);

    fn get_right(&self) -> u32;
    fn set_right(&mut self, right: u32);
}

pub struct AvlForest<T: AvlNode + Copy, A: NodeArena<T>> {
    arena: A,
    item: PhantomData<T>,
}

impl<T: AvlNode + Copy, A: NodeArena<T>> AvlForest<T, A> {
    pub fn new(arena: A) -> Self {
        AvlForest { arena, item: PhantomData }
    }
}

impl<T: AvlNode + Copy, A: NodeArena<T>> AvlForest<T, A> {
    pub fn append(&mut self, key: T::KeyType, value: T::ValueType) -> Option<u32> {
        // allocate a new node in the arena
        let idx = self.arena.insert(T::new(key, value))?;

        // return the index of the newly inserted node as the root of the tree
        return Some(idx);
    }

    pub fn insert(&mut self, tree: u32, key: T::KeyType, value: T::ValueType) -> Option<u32> {
        // allocate a new node in the arena
        let idx = self.arena.insert(T::new(key, value))?;

        // trigger recursive insertion
        let root = unsafe { self.insert_recursive(tree, idx, key) };

        // return the index of the newly inserted node
        return Some(root);
    }

    pub fn height(&self, tree: u32) -> u32 {
        let mut idx = tree;
        let mut height = 0;

        unsafe {
            while idx > 0 {
                height += 1;
                idx = match self.arena.get_unchecked(idx).get_balance() {
                    Ordering::Equal | Ordering::Less => self.arena.get_unchecked(idx).get_left(),
                    Ordering::Greater => self.arena.get_unchecked(idx).get_right(),
                }
            }
        }

        return height;
    }

    unsafe fn insert_recursive(&mut self, parent: u32, node: u32, key: T::KeyType) -> u32 {
        println!("Inserting node, parent {}, node {}", parent, node);

        // recursion base case
        if parent == 0 {
            return node;
        }

        // we are ok with copying the parent node here, the variable is read-only
        let (idx, parent) = (parent, self.arena.get_unchecked(parent).clone());

        if key <= parent.get_key() {
            let left = self.insert_recursive(parent.get_left(), node, key);
            self.arena.get_unchecked_mut(idx).set_left(left);

            match self.arena.get_unchecked(idx).get_balance() {
                Ordering::Equal => {
                    println!("Setting balance to Less for node {}", idx);
                    self.arena.get_unchecked_mut(idx).set_balance(Ordering::Less);
                }
                Ordering::Greater => {
                    println!("Setting balance to Equal for node {}", idx);
                    self.arena.get_unchecked_mut(idx).set_balance(Ordering::Equal);
                }
                Ordering::Less => {
                    println!("Node {} is already left heavy, rotating", idx);
                    return self.rotate_ll(idx);
                }
            }
        } else {
            let right = self.insert_recursive(parent.get_right(), node, key);
            self.arena.get_unchecked_mut(idx).set_right(right);
        }

        return idx;
    }

    //         z (-2)           y (0)
    //        /                / \
    //       y (-1 or 0)  (0) x   z (-1 or 0)
    //      / \                  /
    // (0) x   a (?)            a (?)
    unsafe fn rotate_ll(&mut self, z: u32) -> u32 {
        let y = self.arena.get_unchecked(z).get_left();
        let a = self.arena.get_unchecked(y).get_right();
        let x = self.arena.get_unchecked(y).get_left();
        let balance = if a != 0 { Ordering::Less } else { Ordering::Equal };

        println!("Rotating LL at node {} {} {}", z, y, x);

        self.arena.get_unchecked_mut(z).set_left(a);
        self.arena.get_unchecked_mut(z).set_balance(balance);

        self.arena.get_unchecked_mut(y).set_right(z);
        self.arena.get_unchecked_mut(y).set_balance(Ordering::Equal);

        // return new root
        return y;
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::*;
    use crate::arena::NodeArray;

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

            // left heavy
            if left > 0 {
                return Ordering::Less;
            }

            // right heavy
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

    #[test]
    fn can_create_avl_forest_with_a_new_root() {
        let arena: NodeArray<TestNode, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let root = forest.append(1, 2);
        assert!(root.is_some());

        let height = forest.height(root.unwrap());
        assert_eq!(height, 1);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_ll() {
        let arena: NodeArray<TestNode, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let root = forest.append(30, 300).unwrap();
        let root = forest.insert(root, 20, 200).unwrap();
        let root = forest.insert(root, 10, 100).unwrap();

        let height = forest.height(root);
        assert_eq!(height, 2);
    }
}
