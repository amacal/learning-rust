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
    fn new(key: K, value: V) -> AvlNode<K, V> {
        AvlNode(Node { item: Item { key, value, left: 0, right: 0 } })
    }

    fn get_root(&self) -> u32 {
        unsafe { self.tree.root }
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
        let idx = self.arena.insert(Node::new(key, value))?;

        // return the index of the newly inserted node as the root of the tree
        return Some(idx);
    }

    pub fn insert(&mut self, tree: u32, key: K, value: V) -> Option<u32> {
        // allocate a new node in the arena
        let idx = self.arena.insert(Node::new(key, value))?;

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
                idx = match self.arena.get_unchecked(idx).0.get_balance() {
                    Ordering::Equal | Ordering::Less => self.arena.get_unchecked(idx).0.get_left(),
                    Ordering::Greater => self.arena.get_unchecked(idx).0.get_right(),
                }
            }
        }

        return height;
    }

    unsafe fn insert_recursive(&mut self, parent: u32, node: u32, key: K) -> u32 {
        println!("Inserting node, parent {}, node {}", parent, node);

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
                    println!("Setting balance to Less for node {}", idx);
                    self.arena.get_unchecked_mut(idx).0.set_balance(Ordering::Less);
                }
                Ordering::Greater => {
                    println!("Setting balance to Equal for node {}", idx);
                    self.arena.get_unchecked_mut(idx).0.set_balance(Ordering::Equal);
                }
                Ordering::Less => {
                    println!("Node {} is already left heavy, rotating", idx);
                    return self.rotate_ll(idx);
                }
            }
        } else {
            let right = self.insert_recursive(parent.0.get_right(), node, key);
            self.arena.get_unchecked_mut(idx).0.set_right(right);
        }

        return idx;
    }

    //         z (-2)           y (0)
    //        /                / \
    //       y (-1 or 0)  (0) x   z (-1 or 0)
    //      / \                  /
    // (0) x   a (?)            a (?)
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
        let root = forest.insert(root, 20, 200).unwrap();
        let root = forest.insert(root, 10, 100).unwrap();

        let height = forest.height(root);
        assert_eq!(height, 2);
    }
}
