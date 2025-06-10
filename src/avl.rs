use super::arena::NodeArena;

use rand::Rng;
use std::{cmp::Ordering, fmt::Debug, marker::PhantomData};

#[derive(Copy, Clone)]
struct Tree<T: Copy> {
    root: u32,
    value: T,
}

#[derive(Copy, Clone)]
struct Item<K: Copy + PartialOrd, V: Copy, G: Copy> {
    key: K,
    value: V,
    augment: G,
    left: u32,
    right: u32,
}

#[derive(Copy, Clone)]
union Node<T: Copy, K: Copy + PartialOrd, V: Copy, G: Copy> {
    tree: Tree<T>,
    item: Item<K, V, G>,
}

#[derive(Copy, Clone)]
pub struct AvlNode<T: Copy, K: Copy + PartialOrd, V: Copy, G: Copy>(Node<T, K, V, G>);

pub trait AvlAugment<V, G> {
    fn augment(value: &V, left: Option<&G>, right: Option<&G>) -> G;
}

impl<T: Copy, K: Copy + PartialOrd, V: Copy, G: Copy + AvlAugment<V, G>> Node<T, K, V, G> {
    fn node(key: K, value: V) -> AvlNode<T, K, V, G> {
        AvlNode(Node { item: Item { key, value, left: 0, right: 0, augment: G::augment(&value, None, None) } })
    }

    fn tree(value: T) -> AvlNode<T, K, V, G> {
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

    fn get_augmented(&self) -> G {
        unsafe { self.item.augment }
    }

    fn set_augmented(&mut self, augment: G) {
        self.item.augment = augment;
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

pub struct AvlForest<T: Copy, K: Copy + PartialOrd, V: Copy, G: Copy, N: Copy, A: NodeArena<N>> {
    arena: A,
    key: PhantomData<K>,
    value: PhantomData<V>,
    augment: PhantomData<G>,
    tree: PhantomData<T>,
    into: PhantomData<N>,
}

impl<T: Copy, K: Copy + PartialOrd, V: Copy, G: Copy, N: Copy, A: NodeArena<N>> AvlForest<T, K, V, G, N, A> {
    pub fn new(arena: A) -> Self {
        AvlForest { arena, key: PhantomData, value: PhantomData, augment: PhantomData, tree: PhantomData, into: PhantomData }
    }
}

impl<T: Copy, K: Copy + PartialOrd, V: Copy + Debug, N, G, A: NodeArena<N>> AvlForest<T, K, V, G, N, A>
where
    G: Copy + AvlAugment<V, G>,
    N: Copy + From<AvlNode<T, K, V, G>> + Into<AvlNode<T, K, V, G>>,
    for<'a> &'a N: Into<&'a AvlNode<T, K, V, G>>,
    for<'a> &'a mut N: Into<&'a mut AvlNode<T, K, V, G>>,
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
        let rotated = unsafe { self.insert_recursive(parent, idx, key) & 0x7fffffff };

        // update the root of the tree if it was rotated
        unsafe { self.arena.get_unchecked_mut(tree).into().0.set_root(rotated) };

        // return the index of the newly inserted node
        return Some(idx);
    }

    pub fn root(&self, tree: u32) -> u32 {
        unsafe { self.arena.get_unchecked(tree).into().0.get_root() }
    }

    pub fn value(&self, node: u32) -> V {
        unsafe { self.arena.get_unchecked(node).into().0.get_value() }
    }

    pub fn augmented(&self, node: u32) -> G {
        unsafe { self.arena.get_unchecked(node).into().0.get_augmented() }
    }

    pub fn height(&self, tree: u32) -> u32 {
        // initially we start with the height of 0
        let mut height = 0;

        // and the root of the tree
        let mut idx = unsafe { self.arena.get_unchecked(tree).into().0.get_root() };

        unsafe {
            while idx > 0 {
                // each step down the tree increases the height
                height += 1;

                // we are always picking up the longer branch relying on the balance
                idx = match self.arena.get_unchecked(idx).into().0.get_balance() {
                    Ordering::Equal | Ordering::Less => self.arena.get_unchecked(idx).into().0.get_left(),
                    Ordering::Greater => self.arena.get_unchecked(idx).into().0.get_right(),
                }
            }
        }

        return height;
    }

    unsafe fn update_augmented(&mut self, node: u32) {
        // we need to find left and right children indices
        let left: u32 = unsafe { self.arena.get_unchecked(node).into().0.get_left() };
        let right: u32 = unsafe { self.arena.get_unchecked(node).into().0.get_right() };

        // if the left or right index is not 0, we can get the nodes
        let left: Option<&N> = if left == 0 { None } else { Some(unsafe { self.arena.get_unchecked(left) }) };
        let right: Option<&N> = if right == 0 { None } else { Some(unsafe { self.arena.get_unchecked(right) }) };

        // if the left or right node is not None, we can get augmented values
        let left: Option<G> = if let Some(left) = left { Some(left.into().0.get_augmented()) } else { None };
        let right: Option<G> = if let Some(right) = right { Some(right.into().0.get_augmented()) } else { None };

        // now we can get the value of the current node and augment it
        let value = unsafe { self.arena.get_unchecked(node).into().0.get_value() };
        let augmented = G::augment(&value, left.as_ref(), right.as_ref());

        // and finally we can set the augmented value of the current node
        unsafe { self.arena.get_unchecked_mut(node).into().0.set_augmented(augmented) };
    }

    unsafe fn insert_recursive(&mut self, parent: u32, node: u32, key: K) -> u32 {
        // recursion base case
        if parent == 0 {
            // the subtree grew by one node
            return node | 0x80000000;
        }

        // we are ok with copying the parent node here, the variable is read-only
        let (idx, parent) = (parent, unsafe { self.arena.get_unchecked(parent) });

        unsafe {
            if key <= parent.into().0.get_key() {
                println!("Inserting {:?} into left subtree of {:?}", node, parent.into().0.get_value());
                let left = self.insert_recursive(parent.into().0.get_left(), node, key);
                let (left, grew) = (left & 0x7fffffff, left & 0x80000000);

                self.arena.get_unchecked_mut(idx).into().0.set_left(left);
                self.update_augmented(idx);

                if grew == 0 {
                    return idx;
                }

                match self.arena.get_unchecked(idx).into().0.get_balance() {
                    Ordering::Equal => {
                        println!("Setting balance to Less for node {:?}", idx);
                        self.arena.get_unchecked_mut(idx).into().0.set_balance(Ordering::Less);
                        return idx | 0x80000000;
                    }
                    Ordering::Greater => {
                        println!("Setting balance to Equal for node {:?}", idx);
                        self.arena.get_unchecked_mut(idx).into().0.set_balance(Ordering::Equal);
                    }
                    Ordering::Less => match self.arena.get_unchecked(left).into().0.get_balance() {
                        Ordering::Less => {
                            println!("Rotating left-left case at node {:?}", idx);
                            return self.rotate_ll(idx);
                        }
                        Ordering::Equal | Ordering::Greater => {
                            println!("Rotating left-right case at node {:?}", idx);
                            return self.rotate_lr(idx);
                        }
                    },
                }
            } else {
                println!("Inserting {:?} into right subtree of {:?}", node, parent.into().0.get_value());
                let right = self.insert_recursive(parent.into().0.get_right(), node, key);
                let (right, grew) = (right & 0x7fffffff, right & 0x80000000);

                self.arena.get_unchecked_mut(idx).into().0.set_right(right);
                self.update_augmented(idx);

                if grew == 0 {
                    return idx;
                }

                match self.arena.get_unchecked(idx).into().0.get_balance() {
                    Ordering::Equal => {
                        println!("Setting balance to Greater for node {:?}", idx);
                        self.arena.get_unchecked_mut(idx).into().0.set_balance(Ordering::Greater);
                        return idx | 0x80000000;
                    }
                    Ordering::Less => {
                        println!("Setting balance to Equal for node {:?}", idx);
                        self.arena.get_unchecked_mut(idx).into().0.set_balance(Ordering::Equal);
                    }
                    Ordering::Greater => match self.arena.get_unchecked(right).into().0.get_balance() {
                        Ordering::Greater => {
                            println!("Rotating right-right case at node {:?}", idx);
                            return self.rotate_rr(idx);
                        }
                        Ordering::Equal | Ordering::Less => {
                            println!("Rotating right-left case at node {:?}", idx);
                            return self.rotate_rl(idx);
                        }
                    },
                }
            }
        }

        return idx;
    }

    // Rotate left-left case:
    //
    //           z (-2)                 y (+1/0)
    //          / \                    / \
    //  (-1/0) y   b (?)    =>    (0) x   z (-1/0)
    //        / \                        / \
    //   (0) x   a (?)              (?) a   b (?)
    //
    fn rotate_ll(&mut self, z: u32) -> u32 {
        unsafe {
            // get indices of the nodes involved in the rotation
            let y = self.arena.get_unchecked(z).into().0.get_left();
            let a = self.arena.get_unchecked(y).into().0.get_right();

            // relink the nodes
            self.arena.get_unchecked_mut(z).into().0.set_left(a);
            self.arena.get_unchecked_mut(y).into().0.set_right(z);

            // adjust balances of z and y
            self.arena.get_unchecked_mut(z).into().0.set_balance(Ordering::Equal);
            self.arena.get_unchecked_mut(y).into().0.set_balance(Ordering::Equal);

            // match self.arena.get_unchecked(y).into().0.get_balance() {
            //     Ordering::Equal => {
            //     }
            //     Ordering::Greater => {
            //         assert!(false);
            //         self.arena.get_unchecked_mut(z).into().0.set_balance(Ordering::Less);
            //         self.arena.get_unchecked_mut(y).into().0.set_balance(Ordering::Greater);
            //     }
            //     Ordering::Less => {
            //         assert!(false);
            //     }
            // }

            // update the augmented values of the nodes
            self.update_augmented(z);
            self.update_augmented(y);

            // return y as the new root of the subtree
            return y;
        }
    }

    // Rotate left-right case:
    //
    //            z (-2)                    x (0)
    //           / \                       / \
    //     (+1) y   d (?)    =>    (-1/0) y   z (+1/0)
    //         / \                      / \   / \
    //    (?) c   x (?)                c  a   b  d
    //           / \
    //      (?) a   b (?)
    //
    unsafe fn rotate_lr(&mut self, z: u32) -> u32 {
        unsafe {
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

            // update the augmented values of the nodes
            self.update_augmented(z);
            self.update_augmented(y);
            self.update_augmented(x);

            // and x becomes the new root of the subtree
            return x;
        }
    }

    // Rotate right-right case:
    //
    //          z (+2)                   y (0/-1)
    //         / \                      / \
    //    (?) b   y (+1/0)  =>  (+1/0) z   x (0)
    //           / \                  / \
    //      (?) a   x (0)        (?) b   a (?)
    //
    fn rotate_rr(&mut self, z: u32) -> u32 {
        unsafe {
            // get indices of the nodes involved in the rotation
            let y = self.arena.get_unchecked(z).into().0.get_right();
            let a = self.arena.get_unchecked(y).into().0.get_left();

            // relink the nodes
            self.arena.get_unchecked_mut(z).into().0.set_right(a);
            self.arena.get_unchecked_mut(y).into().0.set_left(z);

            // adjust balances of z and y
            self.arena.get_unchecked_mut(z).into().0.set_balance(Ordering::Equal);
            self.arena.get_unchecked_mut(y).into().0.set_balance(Ordering::Equal);

            // match self.arena.get_unchecked(y).into().0.get_balance() {
            //     Ordering::Greater => {
            //     }
            //     Ordering::Equal => {
            //         assert!(false);
            //         self.arena.get_unchecked_mut(z).into().0.set_balance(Ordering::Greater);
            //         self.arena.get_unchecked_mut(y).into().0.set_balance(Ordering::Less);
            //     }
            //     Ordering::Less => {
            //         assert!(false);
            //     }
            // }

            // update the augmented values of the nodes
            self.update_augmented(z);
            self.update_augmented(y);

            // return y as the new root of the subtree
            return y;
        }
    }

    // Rotate right-left case:
    // - z is right-heavy (+2)
    // - y is left-heavy (-1)
    //
    //            z (+2)                    x (0)
    //             \                       / \
    //              y (-1)    =>   (-1/0) z   y (+1/0)
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

        // not forget about the balance of x
        self.arena.get_unchecked_mut(x).into().0.set_balance(Ordering::Equal);

        // update the augmented values of the nodes
        self.update_augmented(z);
        self.update_augmented(y);
        self.update_augmented(x);

        // and x becomes the new root of the subtree
        return x;
    }
}

impl<T: Copy + Debug, K: Copy + Debug + PartialOrd, V: Copy + Debug, N, G, A: NodeArena<N>> AvlForest<T, K, V, G, N, A>
where
    G: Copy + Debug + AvlAugment<V, G>,
    N: Copy + From<AvlNode<T, K, V, G>> + Into<AvlNode<T, K, V, G>>,
    for<'a> &'a N: Into<&'a AvlNode<T, K, V, G>>,
    for<'a> &'a mut N: Into<&'a mut AvlNode<T, K, V, G>>,
{
    pub fn print(&self, tree: u32) {
        unsafe { self.print_recursive(self.root(tree), 0) };
    }

    unsafe fn print_recursive(&self, node: u32, depth: usize) {
        // indent by depth
        for _ in 0..depth {
            print!("  ");
        }

        if node == 0 {
            println!("- nil");
            return;
        }

        let node_ref = unsafe { self.arena.get_unchecked(node).into() };
        let key = node_ref.0.get_key();
        let value = node_ref.0.get_value();
        let aug = node_ref.0.get_augmented();
        let balance = node_ref.0.get_balance();
        let left = node_ref.0.get_left();
        let right = node_ref.0.get_right();

        println!("- key: {:?}, val: {:?}, aug: {:?}, bal: {:?}, idx: {}", key, value, aug, balance, node);

        unsafe {
            self.print_recursive(left, depth + 1);
            self.print_recursive(right, depth + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::NodeArray;

    #[derive(Copy, Clone, Debug)]
    struct TestI32(i32);

    impl AvlAugment<i32, TestI32> for TestI32 {
        fn augment(value: &i32, left: Option<&TestI32>, right: Option<&TestI32>) -> TestI32 {
            let left = left.map_or(0, |l| l.0);
            let right = right.map_or(0, |r| r.0);

            return TestI32(left + right + value);
        }
    }

    impl Default for TestI32 {
        fn default() -> Self {
            TestI32(0)
        }
    }

    impl From<i32> for TestI32 {
        fn from(value: i32) -> Self {
            TestI32(value)
        }
    }

    #[derive(Copy, Clone)]
    union Foreign {
        avl: AvlNode<i32, i32, i32, TestI32>,
    }

    impl From<AvlNode<i32, i32, i32, TestI32>> for Foreign {
        fn from(node: AvlNode<i32, i32, i32, TestI32>) -> Self {
            Foreign { avl: node }
        }
    }

    impl Into<AvlNode<i32, i32, i32, TestI32>> for Foreign {
        fn into(self) -> AvlNode<i32, i32, i32, TestI32> {
            unsafe { self.avl }
        }
    }

    impl<'a> Into<&'a AvlNode<i32, i32, i32, TestI32>> for &'a Foreign {
        fn into(self) -> &'a AvlNode<i32, i32, i32, TestI32> {
            unsafe { &self.avl }
        }
    }

    impl<'a> Into<&'a mut AvlNode<i32, i32, i32, TestI32>> for &'a mut Foreign {
        fn into(self) -> &'a mut AvlNode<i32, i32, i32, TestI32> {
            unsafe { &mut self.avl }
        }
    }

    #[test]
    fn can_create_avl_forest_with_a_new_root() {
        let arena: NodeArray<AvlNode<i32, i32, i32, TestI32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13);
        assert!(tree.is_some());

        let root = forest.insert(tree.unwrap(), 1, 2.into());
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

        let root = forest.insert(tree.unwrap(), 1, 2.into());
        assert!(root.is_some());

        let height = forest.height(tree.unwrap());
        assert_eq!(height, 1);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_ll_pure() {
        let arena: NodeArray<AvlNode<i32, i32, i32, TestI32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 30, 300.into()).unwrap();
        let _ = forest.insert(tree, 20, 200.into()).unwrap();
        let _ = forest.insert(tree, 10, 100.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 2);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 200);
        assert_eq!(forest.augmented(root).0, 600);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_ll_grew() {
        let arena: NodeArray<AvlNode<i32, i32, i32, TestI32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 30, 300.into()).unwrap();
        let _ = forest.insert(tree, 20, 200.into()).unwrap();
        let _ = forest.insert(tree, 35, 350.into()).unwrap();
        let _ = forest.insert(tree, 10, 100.into()).unwrap();
        let _ = forest.insert(tree, 25, 250.into()).unwrap();
        let _ = forest.insert(tree, 5, 50.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 3);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 200);
        assert_eq!(forest.augmented(root).0, 1250);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_lr_pure() {
        let arena: NodeArray<AvlNode<i32, i32, i32, TestI32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 30, 300.into()).unwrap();
        let _ = forest.insert(tree, 20, 200.into()).unwrap();
        let _ = forest.insert(tree, 25, 250.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 2);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 250);
        assert_eq!(forest.augmented(root).0, 750);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_lr_grew_case_1() {
        let arena: NodeArray<AvlNode<i32, i32, i32, TestI32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 30, 300.into()).unwrap();
        let _ = forest.insert(tree, 35, 350.into()).unwrap();
        let _ = forest.insert(tree, 20, 200.into()).unwrap();
        let _ = forest.insert(tree, 15, 150.into()).unwrap();
        let _ = forest.insert(tree, 25, 250.into()).unwrap();
        let _ = forest.insert(tree, 23, 230.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 3);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 250);
        assert_eq!(forest.augmented(root).0, 1480);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_lr_grew_case_2() {
        let arena: NodeArray<AvlNode<i32, i32, i32, TestI32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 30, 300.into()).unwrap();
        let _ = forest.insert(tree, 35, 350.into()).unwrap();
        let _ = forest.insert(tree, 20, 200.into()).unwrap();
        let _ = forest.insert(tree, 15, 150.into()).unwrap();
        let _ = forest.insert(tree, 25, 250.into()).unwrap();
        let _ = forest.insert(tree, 27, 270.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 3);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 250);
        assert_eq!(forest.augmented(root).0, 1520);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_rr_pure() {
        let arena: NodeArray<AvlNode<i32, i32, i32, TestI32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 10, 100.into()).unwrap();
        let _ = forest.insert(tree, 20, 200.into()).unwrap();
        let _ = forest.insert(tree, 30, 300.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 2);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 200);
        assert_eq!(forest.augmented(root).0, 600);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_rr_grew() {
        let arena: NodeArray<AvlNode<i32, i32, i32, TestI32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 10, 100.into()).unwrap();
        let _ = forest.insert(tree, 5, 50.into()).unwrap();
        let _ = forest.insert(tree, 20, 200.into()).unwrap();
        let _ = forest.insert(tree, 15, 150.into()).unwrap();
        let _ = forest.insert(tree, 25, 250.into()).unwrap();
        let _ = forest.insert(tree, 23, 230.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 3);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 200);
        assert_eq!(forest.augmented(root).0, 980);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_rl() {
        let arena: NodeArray<AvlNode<i32, i32, i32, TestI32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 10, 100.into()).unwrap();
        let _ = forest.insert(tree, 20, 200.into()).unwrap();
        let _ = forest.insert(tree, 15, 150.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 2);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 150);
        assert_eq!(forest.augmented(root).0, 450);
    }

    #[test]
    fn can_augment_sum_of_thousand_nodes_of_fiften_nodes() {
        for _ in 0..10000 {
            let arena: NodeArray<AvlNode<i32, i16, i32, TestI32>, 2000000> = NodeArray::new();
            let mut forest = AvlForest::new(arena);

            let mut rng = rand::rng();
            let tree = forest.append(13).unwrap();

            for i in 0..15 {
                let _ = forest.insert(tree, rng.random(), i.into()).unwrap();
            }

            if forest.height(tree) > 5 {
                forest.print(tree);
                assert!(false);
            }

            if forest.augmented(forest.root(tree)).0 != 105 {
                forest.print(tree);
                assert!(false);
            }
        }
    }

    #[test]
    fn can_augment_sum_of_thousand_nodes_of_thirty_one_nodes() {
        for _ in 0..10000 {
            let arena: NodeArray<AvlNode<i32, i16, i32, TestI32>, 2000000> = NodeArray::new();
            let mut forest = AvlForest::new(arena);

            let mut rng = rand::rng();
            let tree = forest.append(13).unwrap();

            for i in 0..31 {
                let _ = forest.insert(tree, rng.random(), i.into()).unwrap();
            }

            if forest.height(tree) > 6 {
                forest.print(tree);
                assert!(false);
            }

            if forest.augmented(forest.root(tree)).0 != 465 {
                forest.print(tree);
                assert!(false);
            }
        }
    }

    #[test]
    fn can_augment_sum_of_thousand_nodes_of_sixty_three_nodes() {
        for _ in 0..10000 {
            let arena: NodeArray<AvlNode<i32, i16, i32, TestI32>, 2000000> = NodeArray::new();
            let mut forest = AvlForest::new(arena);

            let mut rng = rand::rng();
            let tree = forest.append(13).unwrap();

            for i in 0..63 {
                let _ = forest.insert(tree, rng.random(), i.into()).unwrap();
            }

            if forest.height(tree) > 8 {
                forest.print(tree);
                assert!(false);
            }

            if forest.augmented(forest.root(tree)).0 != 1953 {
                forest.print(tree);
                assert_eq!(forest.augmented(forest.root(tree)).0, 1953);
                assert!(false);
            }
        }
    }

    #[test]
    fn can_build_avl_tree_of_seven_nodes() {
        let arena: NodeArray<AvlNode<i32, i16, i32, TestI32>, 200> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let items: [i16; 7] = [17032, -22888, 30521, -27236, 20409, -11128, -4109];
        let tree = forest.append(13).unwrap();

        for (i, item) in items.iter().enumerate() {
            let _ = forest.insert(tree, *item, i as i32).unwrap();

            println!();
            forest.print(tree);
        }

        assert_eq!(forest.height(tree), 4);
    }

    #[test]
    fn can_build_avl_tree_of_fifteen_nodes_case_1() {
        let arena: NodeArray<AvlNode<i32, i16, i32, TestI32>, 200> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let items: [i16; 15] = [12887, -17025, -17760, 25232, 1731, 28198, 16485, -29386, -2389, 14664, -12411, 5699, -4286, 27501, 19256];
        let tree = forest.append(13).unwrap();

        for (i, key) in items.iter().enumerate() {
            let _ = forest.insert(tree, *key, i as i32).unwrap();
        }

        assert_eq!(forest.height(tree), 5);
    }

    #[test]
    fn can_build_avl_tree_of_fifteen_nodes_case_2() {
        let arena: NodeArray<AvlNode<i32, i16, i32, TestI32>, 200> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let items: [i16; 15] = [-24585, 9045, -17767, 17675, -8874, 21324, -27870, 22546, -28679, -15606, 29562, 2519, -17342, 1152, 19778];
        let tree = forest.append(13).unwrap();

        for (i, key) in items.iter().enumerate() {
            let _ = forest.insert(tree, *key, i as i32).unwrap();
        }

        assert_eq!(forest.height(tree), 5);
    }

    #[test]
    fn can_build_avl_tree_of_sixty_three_nodes_case_1() {
        let arena: NodeArray<AvlNode<i32, i16, i32, TestI32>, 200> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let items: [i16; 63] = [
            5070, -8319, -17104, -24662, -28470, -28597, -31027, -26592, -21682, -22041, -24460, -22552, -21841, -18621, -17183, -15431, -16194, -16420,
            -16666, -16109, -16118, -11185, -12346, -15218, -12083, -9903, -9233, -1875, -4494, -7764, -6225, -2318, -2461, -2339, -2155, 2455, -1846, 1382,
            4338, 11945, 7138, 6652, 5824, 6792, 11045, 8269, 7687, 9642, 11527, 20798, 18044, 14157, 13864, 17397, 18502, 19930, 24152, 20938, 23705, 32355,
            30116, 32608, 32765,
        ];
        let tree = forest.append(13).unwrap();

        for (i, key) in items.iter().enumerate() {
            let _ = forest.insert(tree, *key, i as i32).unwrap();
        }

        assert_eq!(forest.height(tree), 7);
    }
}
