use super::arena::NodeArena;
use std::{cmp::Ordering, marker::PhantomData};

#[derive(Copy, Clone)]
struct Tree {
    root: u32,
}

#[derive(Copy, Clone)]
struct Item<K: Copy + PartialOrd, V: Copy> {
    key: K,
    value: V,
    left: u32,
    right: u32,
}

#[derive(Copy, Clone)]
union Node<K: Copy + PartialOrd, V: Copy> {
    tree: Tree,
    item: Item<K, V>,
}

#[derive(Copy, Clone)]
pub struct AvlNode<K: Copy + PartialOrd, V: Copy>(Node<K, V>);

impl<K: Copy + PartialOrd, V: Copy> Node<K, V> {
    fn node(key: K, value: V) -> AvlNode<K, V> {
        AvlNode(Node { item: Item { key, value, left: 0, right: 0 } })
    }

    fn tree(root: u32) -> AvlNode<K, V> {
        AvlNode(Node { tree: Tree { root } })
    }

    fn get_root(&self) -> u32 {
        unsafe { self.tree.root }
    }

    fn set_root(&mut self, root: u32) {
        self.tree.root = root;
    }

    fn get_key(&self) -> K {
        unsafe { self.item.key }
    }

    fn get_value(&self) -> V {
        unsafe { self.item.value }
    }

    fn get_balance(&self) -> Ordering {
        let left = unsafe { self.item.left & 0x80000000 };
        let right = unsafe { self.item.right & 0x80000000 };

        if left == right {
            return Ordering::Equal;
        }

        if left > 0 {
            return Ordering::Less;
        }

        return Ordering::Greater;
    }

    fn set_balance(&mut self, balance: Ordering) {
        unsafe {
            match balance {
                Ordering::Equal => {
                    self.item.left &= 0x7fffffff;
                    self.item.right &= 0x7fffffff;
                }
                Ordering::Less => {
                    self.item.left |= 0x80000000;
                    self.item.right &= 0x7fffffff;
                }
                Ordering::Greater => {
                    self.item.left &= 0x7fffffff;
                    self.item.right |= 0x80000000;
                }
            }
        }
    }

    fn get_left(&self) -> u32 {
        unsafe { self.item.left & 0x7fffffff }
    }

    fn set_left(&mut self, left: u32) {
        unsafe { self.item.left = left | self.item.left & 0x80000000 };
    }

    fn get_right(&self) -> u32 {
        unsafe { self.item.right & 0x7fffffff }
    }

    fn set_right(&mut self, right: u32) {
        unsafe { self.item.right = right | self.item.right & 0x80000000 };
    }
}

pub struct AvlForest<K: Copy + PartialOrd, V: Copy, A: NodeArena<AvlNode<K, V>>> {
    arena: A,
    key: PhantomData<K>,
    value: PhantomData<V>,
}

impl<K: Copy + PartialOrd, V: Copy, A: NodeArena<AvlNode<K, V>>> AvlForest<K, V, A> {
    pub fn new(arena: A) -> Self {
        AvlForest { arena, key: PhantomData, value: PhantomData }
    }
}

impl<K: Copy + PartialOrd, V: Copy, A: NodeArena<AvlNode<K, V>>> AvlForest<K, V, A> {
    pub fn append(&mut self, key: K, value: V) -> Option<u32> {
        // allocate a new node in the arena
        let idx = self.arena.insert(Node::node(key, value))?;

        // return the index of the newly inserted node as the root of the tree
        if let Some(root) = self.arena.insert(Node::tree(idx)) {
            return Some(root);
        }

        // if we failed to insert the tree, release the allocated node
        unsafe { self.arena.release_unchecked(idx) };

        return None;
    }

    pub fn insert(&mut self, tree: u32, key: K, value: V) -> Option<u32> {
        // allocate a new node in the arena
        let idx = self.arena.insert(Node::node(key, value))?;

        // find the root of the tree
        let parent = unsafe { self.arena.get_unchecked(tree).0.get_root() };

        // trigger recursive insertion, may rotate the root
        let rotated = unsafe { self.insert_recursive(parent, idx, key) };

        // update the root of the tree if it was rotated
        if parent != rotated {
            unsafe { self.arena.get_unchecked_mut(tree).0.set_root(rotated) };
        }

        // return the index of the newly inserted node
        return Some(idx);
    }

    pub fn height(&self, tree: u32) -> u32 {
        let mut height = 0;
        let mut idx = unsafe { self.arena.get_unchecked(tree).0.get_root() };

        unsafe {
            while idx > 0 {
                height += 1;
                idx = match self.arena.get_unchecked(idx).0.get_balance() {
                    Ordering::Equal | Ordering::Less => self.arena.get_unchecked(idx).0.get_left(),
                    Ordering::Greater => self.arena.get_unchecked(idx).0.get_right(),
                }
            }
        }

        return height;
    }

    unsafe fn insert_recursive(&mut self, parent: u32, node: u32, key: K) -> u32 {
        // recursion base case
        if parent == 0 {
            return node;
        }

        // we are ok with copying the parent node here, the variable is read-only
        let (idx, parent) = (parent, self.arena.get_unchecked(parent).clone());

        if key <= parent.0.get_key() {
            let left = self.insert_recursive(parent.0.get_left(), node, key);
            self.arena.get_unchecked_mut(idx).0.set_left(left);

            match self.arena.get_unchecked(idx).0.get_balance() {
                Ordering::Equal => {
                    self.arena.get_unchecked_mut(idx).0.set_balance(Ordering::Less);
                }
                Ordering::Greater => {
                    self.arena.get_unchecked_mut(idx).0.set_balance(Ordering::Equal);
                }
                Ordering::Less => {
                    return self.rotate_ll(idx);
                }
            }
        } else {
            let right = self.insert_recursive(parent.0.get_right(), node, key);
            self.arena.get_unchecked_mut(idx).0.set_right(right);
        }

        return idx;
    }

    // Rotate left-left case; z is known to be left-heavy (balance == -2).
    // This implies:
    // - b must be null (right subtree of z has height 0)
    // - a is either null or a leaf (max height 1), otherwise this wouldn’t be LL.
    //
    //              z (-2)                        y (0)
    //             / \                           / \
    //  (-1 or 0) y   b (null)     =>       (0) x   z (-1 or 0)
    //           / \                               / \
    //      (0) x   a (null or 0)      (null or 0) a   b (null)
    unsafe fn rotate_ll(&mut self, z: u32) -> u32 {
        let y = self.arena.get_unchecked(z).0.get_left();
        let a = self.arena.get_unchecked(y).0.get_right();
        let balance = if a != 0 { Ordering::Less } else { Ordering::Equal };

        self.arena.get_unchecked_mut(z).0.set_left(a);
        self.arena.get_unchecked_mut(z).0.set_balance(balance);

        self.arena.get_unchecked_mut(y).0.set_right(z);
        self.arena.get_unchecked_mut(y).0.set_balance(Ordering::Equal);

        // return new root
        return y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::NodeArray;

    #[test]
    fn can_create_avl_forest_with_a_new_root() {
        let arena: NodeArray<AvlNode<i32, i32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let root = forest.append(1, 2);
        assert!(root.is_some());

        let height = forest.height(root.unwrap());
        assert_eq!(height, 1);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_ll() {
        let arena: NodeArray<AvlNode<i32, i32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let root = forest.append(30, 300).unwrap();
        let _ = forest.insert(root, 20, 200).unwrap();
        let _ = forest.insert(root, 10, 100).unwrap();

        let height = forest.height(root);
        assert_eq!(height, 2);
    }
}
