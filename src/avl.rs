use super::arena::NodeArena;
use std::marker::PhantomData;

pub trait AvlNode {
    type KeyType: Copy + PartialOrd;
    type ValueType: Copy;

    fn new(key: Self::KeyType, value: Self::ValueType) -> Self;

    fn get_key(&self) -> Self::KeyType;
    fn get_value(&self) -> Self::ValueType;

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
        unsafe { self.insert_recursive(tree, idx, key) };

        // return the index of the newly inserted node
        return Some(idx);
    }

    unsafe fn insert_recursive(&mut self, parent: u32, node: u32, key: T::KeyType) {
        let (idx, parent) = (parent, self.arena.get_unchecked(parent));

        if key <= parent.get_key() {
            match parent.get_left() {
                0 => self.arena.get_unchecked_mut(idx).set_left(node),
                left => self.insert_recursive(left, node, key),
            }
        } else {
            match parent.get_right() {
                0 => self.arena.get_unchecked_mut(idx).set_right(node),
                right => self.insert_recursive(right, node, key),
            }
        }
    }
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn can_create_avl_forest_with_a_new_root() {
        let arena: NodeArray<TestNode, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let node = TestNode::new(1, 2);
        let idx = forest.append(node.get_key(), node.get_value());

        assert!(idx.is_some());
    }
}
