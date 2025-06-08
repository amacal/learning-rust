use super::arena::NodeArena;
use std::{cmp::Ordering, marker::PhantomData};

#[derive(Copy, Clone)]
struct Tree<T: Copy> {
    root: u32,
    value: T,
}

#[derive(Copy, Clone)]
struct Item<K: Copy + PartialOrd, V: Copy> {
    key: K,
    value: V,
    left: u32,
    right: u32,
}

#[derive(Copy, Clone)]
union Node<T: Copy, K: Copy + PartialOrd, V: Copy> {
    tree: Tree<T>,
    item: Item<K, V>,
}

#[derive(Copy, Clone)]
pub struct AvlNode<T: Copy, K: Copy + PartialOrd, V: Copy>(Node<T, K, V>);

impl<T: Copy, K: Copy + PartialOrd, V: Copy> Node<T, K, V> {
    fn node(key: K, value: V) -> AvlNode<T, K, V> {
        AvlNode(Node { item: Item { key, value, left: 0, right: 0 } })
    }

    fn tree(value: T) -> AvlNode<T, K, V> {
        AvlNode(Node { tree: Tree { root: 0, value } })
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

pub struct AvlForest<T: Copy, K: Copy + PartialOrd, V: Copy, N: Copy, A: NodeArena<N>> {
    arena: A,
    key: PhantomData<K>,
    value: PhantomData<V>,
    tree: PhantomData<T>,
    into: PhantomData<N>,
}

impl<T: Copy, K: Copy + PartialOrd, V: Copy, N: Copy, A: NodeArena<N>> AvlForest<T, K, V, N, A> {
    pub fn new(arena: A) -> Self {
        AvlForest { arena, key: PhantomData, value: PhantomData, tree: PhantomData, into: PhantomData }
    }
}

impl<T: Copy, K: Copy + PartialOrd, V: Copy, N: Copy + From<AvlNode<T, K, V>>, A: NodeArena<N>> AvlForest<T, K, V, N, A>
where
    N: Into<AvlNode<T, K, V>>,
    for<'a> &'a N: Into<&'a AvlNode<T, K, V>>,
    for<'a> &'a mut N: Into<&'a mut AvlNode<T, K, V>>,
{
    pub fn append(&mut self, value: T) -> Option<u32> {
        // return the index of the newly inserted node as the root of the tree
        self.arena.insert(N::from(Node::tree(value)))
    }

    pub fn insert(&mut self, tree: u32, key: K, value: V) -> Option<u32> {
        // allocate a new node in the arena
        let idx = self.arena.insert(N::from(Node::node(key, value)))?;

        // find the root of the tree
        let parent = unsafe { self.arena.get_unchecked(tree).into().0.get_root() };

        // trigger recursive insertion, may rotate the root
        let rotated = unsafe { self.insert_recursive(parent, idx, key) };

        // update the root of the tree if it was rotated
        if parent != rotated {
            unsafe { self.arena.get_unchecked_mut(tree).into().0.set_root(rotated) };
        }

        // return the index of the newly inserted node
        return Some(idx);
    }

    pub fn height(&self, tree: u32) -> u32 {
        let mut height = 0;
        let mut idx = unsafe { self.arena.get_unchecked(tree).into().0.get_root() };

        unsafe {
            while idx > 0 {
                height += 1;
                idx = match self.arena.get_unchecked(idx).into().0.get_balance() {
                    Ordering::Equal | Ordering::Less => self.arena.get_unchecked(idx).into().0.get_left(),
                    Ordering::Greater => self.arena.get_unchecked(idx).into().0.get_right(),
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

        if key <= parent.into().0.get_key() {
            let left = self.insert_recursive(parent.into().0.get_left(), node, key);
            self.arena.get_unchecked_mut(idx).into().0.set_left(left);

            match self.arena.get_unchecked(idx).into().0.get_balance() {
                Ordering::Equal => {
                    self.arena.get_unchecked_mut(idx).into().0.set_balance(Ordering::Less);
                }
                Ordering::Greater => {
                    self.arena.get_unchecked_mut(idx).into().0.set_balance(Ordering::Equal);
                }
                Ordering::Less => match self.arena.get_unchecked(left).into().0.get_balance() {
                    Ordering::Less | Ordering::Equal => {
                        return self.rotate_ll(idx);
                    }
                    Ordering::Greater => {
                        return self.rotate_lr(idx);
                    }
                },
            }
        } else {
            let right = self.insert_recursive(parent.into().0.get_right(), node, key);
            self.arena.get_unchecked_mut(idx).into().0.set_right(right);

            match self.arena.get_unchecked(idx).into().0.get_balance() {
                Ordering::Equal => {
                    self.arena.get_unchecked_mut(idx).into().0.set_balance(Ordering::Greater);
                }
                Ordering::Less => {
                    self.arena.get_unchecked_mut(idx).into().0.set_balance(Ordering::Equal);
                }
                Ordering::Greater => match self.arena.get_unchecked(right).into().0.get_balance() {
                    Ordering::Greater | Ordering::Equal => {
                        return self.rotate_rr(idx);
                    }
                    Ordering::Less => {
                        return self.rotate_rl(idx);
                    }
                },
            }
        }

        return idx;
    }

    // Rotate left-left case:
    // - z is left-heavy (-2).
    // - y is either balanced (0) or right-heavy (+1).
    //
    //           z (-2)                 y (0)
    //          / \                    / \
    //  (-1/0) y   b (?)    =>    (0) x   z (-1/0)
    //        / \                        / \
    //   (0) x   a (?)              (?) a   b (?)
    //
    unsafe fn rotate_ll(&mut self, z: u32) -> u32 {
        let y = self.arena.get_unchecked(z).into().0.get_left();
        let a = self.arena.get_unchecked(y).into().0.get_right();
        let balance = if a != 0 { Ordering::Less } else { Ordering::Equal };

        self.arena.get_unchecked_mut(z).into().0.set_left(a);
        self.arena.get_unchecked_mut(z).into().0.set_balance(balance);

        self.arena.get_unchecked_mut(y).into().0.set_right(z);
        self.arena.get_unchecked_mut(y).into().0.set_balance(Ordering::Equal);

        // return y as the new root of the subtree
        return y;
    }

    // Rotate left-right case:
    // - z is left-heavy (-2)
    // - y is right-heavy (+1)
    //
    //            z (-2)                    x (0)
    //           / \                       / \
    //     (+1) y   b (?)    =>    (-1/0) y   z (+1/0)
    //         / \                        \   /
    //    (?) c   x (?)                (?) a b (?)
    //           / \
    //      (?) a   b (?)
    //
    unsafe fn rotate_lr(&mut self, z: u32) -> u32 {
        // extract all nodes involved in the rotations
        let y = self.arena.get_unchecked(z).into().0.get_left();
        let x = self.arena.get_unchecked(y).into().0.get_right();
        let a = self.arena.get_unchecked(x).into().0.get_left();
        let b = self.arena.get_unchecked(x).into().0.get_right();

        // first rotation: y becomes left child of x
        self.arena.get_unchecked_mut(y).into().0.set_right(a);
        self.arena.get_unchecked_mut(x).into().0.set_left(y);

        // second rotation: x becomes new root of the subtree
        self.arena.get_unchecked_mut(z).into().0.set_left(b);
        self.arena.get_unchecked_mut(x).into().0.set_right(z);

        // adjust all balances
        match self.arena.get_unchecked(x).into().0.get_balance() {
            Ordering::Less => {
                self.arena.get_unchecked_mut(z).into().0.set_balance(Ordering::Greater);
                self.arena.get_unchecked_mut(y).into().0.set_balance(Ordering::Equal);
            }
            Ordering::Greater => {
                self.arena.get_unchecked_mut(z).into().0.set_balance(Ordering::Equal);
                self.arena.get_unchecked_mut(y).into().0.set_balance(Ordering::Less);
            }
            Ordering::Equal => {
                self.arena.get_unchecked_mut(z).into().0.set_balance(Ordering::Equal);
                self.arena.get_unchecked_mut(y).into().0.set_balance(Ordering::Equal);
            }
        }

        // not forget about the balance of x
        self.arena.get_unchecked_mut(x).into().0.set_balance(Ordering::Equal);

        // and x becomes the new root of the subtree
        return x;
    }

    // Rotate right-right case:
    // - z is right-heavy (+2).
    // - y is left-heavy (+1) or balanced (0).
    //
    //          z (2)                    y (0)
    //         / \                      / \
    //    (?) b   y (+1/0)  =>  (+1/0) z   x (0)
    //           / \                      / \
    //      (?) a   x (0)            (?) b   a (?)
    //
    unsafe fn rotate_rr(&mut self, z: u32) -> u32 {
        let y = self.arena.get_unchecked(z).into().0.get_right();
        let a = self.arena.get_unchecked(y).into().0.get_left();
        let balance = if a != 0 { Ordering::Greater } else { Ordering::Equal };

        self.arena.get_unchecked_mut(z).into().0.set_right(a);
        self.arena.get_unchecked_mut(z).into().0.set_balance(balance);

        self.arena.get_unchecked_mut(y).into().0.set_left(z);
        self.arena.get_unchecked_mut(y).into().0.set_balance(Ordering::Equal);

        // return y as the new root of the subtree
        return y;
    }

    // Rotate right-left case:
    // - z is right-heavy (+2)
    // - y is left-heavy (-1)
    //
    //            z (+2)                    x (0)
    //           / \                       / \
    //      (?) b   y (-1)    =>   (-1/0) z   y (+1/0)
    //             / \                    \   /
    //        (?) x   c (?)            (?) a b (?)
    //           / \
    //      (?) a   b (?)
    //
    unsafe fn rotate_rl(&mut self, z: u32) -> u32 {
        // extract all nodes involved in the rotations
        let y = self.arena.get_unchecked(z).into().0.get_right();
        let x = self.arena.get_unchecked(y).into().0.get_left();
        let a = self.arena.get_unchecked(x).into().0.get_left();
        let b = self.arena.get_unchecked(x).into().0.get_right();

        // first rotation: y becomes right child of x
        self.arena.get_unchecked_mut(y).into().0.set_left(b);
        self.arena.get_unchecked_mut(x).into().0.set_right(y);

        // second rotation: x becomes new root of subtree
        self.arena.get_unchecked_mut(z).into().0.set_right(a);
        self.arena.get_unchecked_mut(x).into().0.set_left(z);

        // adjust balances
        match self.arena.get_unchecked(x).into().0.get_balance() {
            Ordering::Less => {
                self.arena.get_unchecked_mut(z).into().0.set_balance(Ordering::Equal);
                self.arena.get_unchecked_mut(y).into().0.set_balance(Ordering::Greater);
            }
            Ordering::Greater => {
                self.arena.get_unchecked_mut(z).into().0.set_balance(Ordering::Less);
                self.arena.get_unchecked_mut(y).into().0.set_balance(Ordering::Equal);
            }
            Ordering::Equal => {
                self.arena.get_unchecked_mut(z).into().0.set_balance(Ordering::Equal);
                self.arena.get_unchecked_mut(y).into().0.set_balance(Ordering::Equal);
            }
        }

        self.arena.get_unchecked_mut(x).into().0.set_balance(Ordering::Equal);
        return x;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::NodeArray;

    #[derive(Copy, Clone)]
    union Foreign {
        avl: AvlNode<i32, i32, i32>,
        other: u32,
    }

    impl From<AvlNode<i32, i32, i32>> for Foreign {
        fn from(node: AvlNode<i32, i32, i32>) -> Self {
            Foreign { avl: node }
        }
    }

    impl Into<AvlNode<i32, i32, i32>> for Foreign {
        fn into(self) -> AvlNode<i32, i32, i32> {
            unsafe { self.avl }
        }
    }

    impl<'a> Into<&'a AvlNode<i32, i32, i32>> for &'a Foreign {
        fn into(self) -> &'a AvlNode<i32, i32, i32> {
            unsafe { &self.avl }
        }
    }

    impl<'a> Into<&'a mut AvlNode<i32, i32, i32>> for &'a mut Foreign {
        fn into(self) -> &'a mut AvlNode<i32, i32, i32> {
            unsafe { &mut self.avl }
        }
    }

    #[test]
    fn can_create_avl_forest_with_a_new_root() {
        let arena: NodeArray<AvlNode<i32, i32, i32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13);
        assert!(tree.is_some());

        let root = forest.insert(tree.unwrap(), 1, 2);
        assert!(root.is_some());

        let height = forest.height(tree.unwrap());
        assert_eq!(height, 1);
    }

    #[test]
    fn can_create_avl_forest_with_foreign_node() {
        let arena: NodeArray<Foreign, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13);
        assert!(tree.is_some());

        let root = forest.insert(tree.unwrap(), 1, 2);
        assert!(root.is_some());

        let height = forest.height(tree.unwrap());
        assert_eq!(height, 1);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_ll() {
        let arena: NodeArray<AvlNode<i32, i32, i32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 30, 300).unwrap();
        let _ = forest.insert(tree, 20, 200).unwrap();
        let _ = forest.insert(tree, 10, 100).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 2);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_lr() {
        let arena: NodeArray<AvlNode<i32, i32, i32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 30, 300).unwrap();
        let _ = forest.insert(tree, 20, 200).unwrap();
        let _ = forest.insert(tree, 25, 250).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 2);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_rr() {
        let arena: NodeArray<AvlNode<i32, i32, i32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 10, 100).unwrap();
        let _ = forest.insert(tree, 20, 200).unwrap();
        let _ = forest.insert(tree, 30, 300).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 2);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_rl() {
        let arena: NodeArray<AvlNode<i32, i32, i32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 10, 100).unwrap();
        let _ = forest.insert(tree, 20, 200).unwrap();
        let _ = forest.insert(tree, 15, 150).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 2);
    }
}
