use super::arena::NodeArena;

use std::{fmt::Debug, marker::PhantomData};

// Indicates that the tree grew by one node
const GREW_MASK: u32 = 0x80000000;

// Indicates the balance factor of the node
const BALANCE_MASK: u32 = 0x80000000;

#[derive(Debug)]
enum Balance {
    LeftHeavy,
    Equal,
    RightHeavy,
}

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

    fn get_balance(&self) -> Balance {
        let left = unsafe { self.item.left & BALANCE_MASK };
        let right = unsafe { self.item.right & BALANCE_MASK };

        if left == right {
            return Balance::Equal;
        }

        if left > 0 {
            return Balance::LeftHeavy;
        }

        return Balance::RightHeavy;
    }

    fn set_balance(&mut self, balance: Balance) {
        unsafe {
            match balance {
                Balance::Equal => {
                    self.item.left &= !BALANCE_MASK;
                    self.item.right &= !BALANCE_MASK;
                }
                Balance::LeftHeavy => {
                    self.item.left |= BALANCE_MASK;
                    self.item.right &= !BALANCE_MASK;
                }
                Balance::RightHeavy => {
                    self.item.left &= !BALANCE_MASK;
                    self.item.right |= BALANCE_MASK;
                }
            }
        }
    }

    fn get_left(&self) -> u32 {
        unsafe { self.item.left & !BALANCE_MASK }
    }

    fn set_left(&mut self, left: u32) {
        unsafe { self.item.left = left | self.item.left & BALANCE_MASK };
    }

    fn get_right(&self) -> u32 {
        unsafe { self.item.right & !BALANCE_MASK }
    }

    fn set_right(&mut self, right: u32) {
        unsafe { self.item.right = right | self.item.right & BALANCE_MASK };
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
        let parent = unsafe { self.get_ref(tree).get_root() };

        // trigger recursive insertion, may rotate the root
        let rotated = unsafe { self.insert_recursive(parent, idx, key) & !GREW_MASK };

        // update the root of the tree if it was rotated
        unsafe { self.get_mut(tree).set_root(rotated) };

        // return the index of the newly inserted node
        return Some(idx);
    }

    pub fn root(&self, tree: u32) -> u32 {
        unsafe { self.get_ref(tree).get_root() }
    }

    pub fn value(&self, node: u32) -> V {
        unsafe { self.get_ref(node).get_value() }
    }

    pub fn augmented(&self, node: u32) -> G {
        unsafe { self.get_ref(node).get_augmented() }
    }

    pub fn height(&self, tree: u32) -> u32 {
        // initially we start with the height of 0
        let mut height = 0;

        // and the root of the tree
        let mut idx = unsafe { self.get_ref(tree).get_root() };

        unsafe {
            while idx > 0 {
                // each step down the tree increases the height
                height += 1;

                // we are always picking up the longer branch relying on the balance
                idx = match self.get_ref(idx).get_balance() {
                    Balance::Equal | Balance::LeftHeavy => self.get_ref(idx).get_left(),
                    Balance::RightHeavy => self.get_ref(idx).get_right(),
                }
            }
        }

        return height;
    }

    unsafe fn get_ref(&self, node: u32) -> &Node<T, K, V, G> {
        let node: &N = unsafe { self.arena.get_unchecked(node) };
        let avl: &AvlNode<T, K, V, G> = node.into();

        return &avl.0;
    }

    unsafe fn get_mut(&mut self, node: u32) -> &mut Node<T, K, V, G> {
        let node: &mut N = unsafe { self.arena.get_unchecked_mut(node) };
        let avl: &mut AvlNode<T, K, V, G> = node.into();

        return &mut avl.0;
    }

    unsafe fn update_augmented(&mut self, node: u32) {
        // we need to find left and right children indices
        let left: u32 = unsafe { self.get_ref(node).get_left() };
        let right: u32 = unsafe { self.get_ref(node).get_right() };

        // if the left or right index is not 0, we can get the values
        let left = if left == 0 { None } else { Some(unsafe { self.get_ref(left).get_augmented() }) };
        let right = if right == 0 { None } else { Some(unsafe { self.get_ref(right).get_augmented() }) };

        // now we can get the value of the current node and augment it
        let value = unsafe { self.get_ref(node).get_value() };
        let augmented = G::augment(&value, left.as_ref(), right.as_ref());

        // and finally we can set the augmented value of the current node
        unsafe { self.get_mut(node).set_augmented(augmented) };
    }

    unsafe fn insert_recursive(&mut self, parent: u32, node: u32, key: K) -> u32 {
        // recursion base case
        if parent == 0 {
            // the subtree grew by one node
            return node | GREW_MASK;
        }

        // we are ok with copying the parent node here, the variable is read-only
        let (idx, parent) = (parent, unsafe { self.get_ref(parent) });

        unsafe {
            if key <= parent.get_key() {
                println!("Inserting {:?} into left subtree of {:?}", node, parent.get_value());
                let left = self.insert_recursive(parent.get_left(), node, key);
                let (left, grew) = (left & !GREW_MASK, left & GREW_MASK);

                self.get_mut(idx).set_left(left);
                self.update_augmented(idx);

                if grew == 0 {
                    return idx;
                }

                match self.get_ref(idx).get_balance() {
                    Balance::Equal => {
                        println!("Setting balance to Less for node {:?}", idx);
                        self.get_mut(idx).set_balance(Balance::LeftHeavy);
                        return idx | GREW_MASK;
                    }
                    Balance::RightHeavy => {
                        println!("Setting balance to Equal for node {:?}", idx);
                        self.get_mut(idx).set_balance(Balance::Equal);
                    }
                    Balance::LeftHeavy => match self.get_ref(left).get_balance() {
                        Balance::LeftHeavy => {
                            println!("Rotating left-left case at node {:?}", idx);
                            return self.rotate_ll(idx); // didn't grow
                        }
                        Balance::Equal | Balance::RightHeavy => {
                            println!("Rotating left-right case at node {:?}", idx);
                            return self.rotate_lr(idx); // didn't grow
                        }
                    },
                }
            } else {
                println!("Inserting {:?} into right subtree of {:?}", node, parent.get_value());
                let right = self.insert_recursive(parent.get_right(), node, key);
                let (right, grew) = (right & !GREW_MASK, right & GREW_MASK);

                self.get_mut(idx).set_right(right);
                self.update_augmented(idx);

                if grew == 0 {
                    return idx;
                }

                match self.get_ref(idx).get_balance() {
                    Balance::Equal => {
                        println!("Setting balance to Greater for node {:?}", idx);
                        self.get_mut(idx).set_balance(Balance::RightHeavy);
                        return idx | GREW_MASK;
                    }
                    Balance::LeftHeavy => {
                        println!("Setting balance to Equal for node {:?}", idx);
                        self.get_mut(idx).set_balance(Balance::Equal);
                    }
                    Balance::RightHeavy => match self.get_ref(right).get_balance() {
                        Balance::RightHeavy => {
                            println!("Rotating right-right case at node {:?}", idx);
                            return self.rotate_rr(idx); // didn't grow
                        }
                        Balance::Equal | Balance::LeftHeavy => {
                            println!("Rotating right-left case at node {:?}", idx);
                            return self.rotate_rl(idx); // didn't grow
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
            let y = self.get_ref(z).get_left();
            let a = self.get_ref(y).get_right();

            // relink the nodes
            self.get_mut(z).set_left(a);
            self.get_mut(y).set_right(z);

            // adjust balances of z and y
            self.get_mut(z).set_balance(Balance::Equal);
            self.get_mut(y).set_balance(Balance::Equal);

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
            let y = self.get_ref(z).get_left();
            let x = self.get_ref(y).get_right();
            let a = self.get_ref(x).get_left();
            let b = self.get_ref(x).get_right();

            // first rotation: y becomes left child of x
            self.get_mut(y).set_right(a);
            self.get_mut(x).set_left(y);

            // second rotation: x becomes new root of the subtree
            self.get_mut(z).set_left(b);
            self.get_mut(x).set_right(z);

            // adjust all balances
            match self.get_ref(x).get_balance() {
                Balance::LeftHeavy => {
                    self.get_mut(z).set_balance(Balance::RightHeavy);
                    self.get_mut(y).set_balance(Balance::Equal);
                }
                Balance::RightHeavy => {
                    self.get_mut(z).set_balance(Balance::Equal);
                    self.get_mut(y).set_balance(Balance::LeftHeavy);
                }
                Balance::Equal => {
                    self.get_mut(z).set_balance(Balance::Equal);
                    self.get_mut(y).set_balance(Balance::Equal);
                }
            }

            // not forget about the balance of x
            self.get_mut(x).set_balance(Balance::Equal);

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
            let y = self.get_ref(z).get_right();
            let a = self.get_ref(y).get_left();

            // relink the nodes
            self.get_mut(z).set_right(a);
            self.get_mut(y).set_left(z);

            // adjust balances of z and y
            self.get_mut(z).set_balance(Balance::Equal);
            self.get_mut(y).set_balance(Balance::Equal);

            // update the augmented values of the nodes
            self.update_augmented(z);
            self.update_augmented(y);

            // return y as the new root of the subtree
            return y;
        }
    }

    // Rotate right-left case:
    //
    //            z (+2)                    x (0)
    //     (?) d / \                       / \
    //              y (-1)    =>   (-1/0) z   y (+1/0)
    //             / \                   /\   /\
    //        (?) x   c (?)             d  a b  c
    //           / \
    //      (?) a   b (?)
    //
    unsafe fn rotate_rl(&mut self, z: u32) -> u32 {
        unsafe {
            // extract all nodes involved in the rotations
            let y = self.get_ref(z).get_right();
            let x = self.get_ref(y).get_left();
            let a = self.get_ref(x).get_left();
            let b = self.get_ref(x).get_right();

            // first rotation: y becomes right child of x
            self.get_mut(y).set_left(b);
            self.get_mut(x).set_right(y);

            // second rotation: x becomes new root of subtree
            self.get_mut(z).set_right(a);
            self.get_mut(x).set_left(z);

            // adjust balances
            match self.get_ref(x).get_balance() {
                Balance::LeftHeavy => {
                    self.get_mut(z).set_balance(Balance::Equal);
                    self.get_mut(y).set_balance(Balance::RightHeavy);
                }
                Balance::RightHeavy => {
                    self.get_mut(z).set_balance(Balance::LeftHeavy);
                    self.get_mut(y).set_balance(Balance::Equal);
                }
                Balance::Equal => {
                    self.get_mut(z).set_balance(Balance::Equal);
                    self.get_mut(y).set_balance(Balance::Equal);
                }
            }

            // not forget about the balance of x
            self.get_mut(x).set_balance(Balance::Equal);

            // update the augmented values of the nodes
            self.update_augmented(z);
            self.update_augmented(y);
            self.update_augmented(x);

            // and x becomes the new root of the subtree
            return x;
        }
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

        let node_ref = unsafe { self.get_ref(node) };
        let key = node_ref.get_key();
        let value = node_ref.get_value();
        let aug = node_ref.get_augmented();
        let balance = node_ref.get_balance();
        let left = node_ref.get_left();
        let right = node_ref.get_right();

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
    use rand::Rng;

    #[derive(Copy, Clone, Debug)]
    struct TestU32(u32);

    impl AvlAugment<u32, TestU32> for TestU32 {
        fn augment(value: &u32, left: Option<&TestU32>, right: Option<&TestU32>) -> TestU32 {
            let left = left.map_or(0, |l| l.0);
            let right = right.map_or(0, |r| r.0);

            return TestU32(left + right + value);
        }
    }

    impl Default for TestU32 {
        fn default() -> Self {
            TestU32(0)
        }
    }

    impl From<u32> for TestU32 {
        fn from(value: u32) -> Self {
            TestU32(value)
        }
    }

    #[derive(Copy, Clone)]
    union Foreign {
        avl: AvlNode<i32, i32, u32, TestU32>,
    }

    impl From<AvlNode<i32, i32, u32, TestU32>> for Foreign {
        fn from(node: AvlNode<i32, i32, u32, TestU32>) -> Self {
            Foreign { avl: node }
        }
    }

    impl Into<AvlNode<i32, i32, u32, TestU32>> for Foreign {
        fn into(self) -> AvlNode<i32, i32, u32, TestU32> {
            unsafe { self.avl }
        }
    }

    impl<'a> Into<&'a AvlNode<i32, i32, u32, TestU32>> for &'a Foreign {
        fn into(self) -> &'a AvlNode<i32, i32, u32, TestU32> {
            unsafe { &self.avl }
        }
    }

    impl<'a> Into<&'a mut AvlNode<i32, i32, u32, TestU32>> for &'a mut Foreign {
        fn into(self) -> &'a mut AvlNode<i32, i32, u32, TestU32> {
            unsafe { &mut self.avl }
        }
    }

    fn avl_max_height(nodes: u32) -> u32 {
        if nodes == 0 {
            return 0;
        }

        let (mut prev1, mut prev2) = (0, 1);
        let mut height = 1;

        loop {
            let next = prev1 + prev2 + 1;
            if next > nodes {
                break;
            }

            prev1 = prev2;
            prev2 = next;
            height += 1;
        }

        return height;
    }

    #[test]
    fn can_compute_max_height() {
        assert_eq!(avl_max_height(0), 0);
        assert_eq!(avl_max_height(1), 1);
        assert_eq!(avl_max_height(3), 2);
        assert_eq!(avl_max_height(7), 4);
        assert_eq!(avl_max_height(15), 5);
        assert_eq!(avl_max_height(63), 8);
    }

    #[test]
    fn can_create_avl_forest_with_a_new_root() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13);
        assert!(tree.is_some());

        let root = forest.insert(tree.unwrap(), 1, 2u32.into());
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

        let root = forest.insert(tree.unwrap(), 1, 2u32.into());
        assert!(root.is_some());

        let height = forest.height(tree.unwrap());
        assert_eq!(height, 1);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_ll_pure() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert(tree, 10, 100u32.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 2);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 200);
        assert_eq!(forest.augmented(root).0, 600);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_ll_grew() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert(tree, 35, 350u32.into()).unwrap();
        let _ = forest.insert(tree, 10, 100u32.into()).unwrap();
        let _ = forest.insert(tree, 25, 250u32.into()).unwrap();
        let _ = forest.insert(tree, 5, 50u32.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 3);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 200);
        assert_eq!(forest.augmented(root).0, 1250);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_lr_pure() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert(tree, 25, 250u32.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 2);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 250);
        assert_eq!(forest.augmented(root).0, 750);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_lr_grew_case_1() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert(tree, 35, 350u32.into()).unwrap();
        let _ = forest.insert(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert(tree, 15, 150u32.into()).unwrap();
        let _ = forest.insert(tree, 25, 250u32.into()).unwrap();
        let _ = forest.insert(tree, 23, 230u32.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 3);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 250);
        assert_eq!(forest.augmented(root).0, 1480);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_lr_grew_case_2() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert(tree, 35, 350u32.into()).unwrap();
        let _ = forest.insert(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert(tree, 15, 150u32.into()).unwrap();
        let _ = forest.insert(tree, 25, 250u32.into()).unwrap();
        let _ = forest.insert(tree, 27, 270u32.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 3);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 250);
        assert_eq!(forest.augmented(root).0, 1520);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_rr_pure() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 10, 100u32.into()).unwrap();
        let _ = forest.insert(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert(tree, 30, 300u32.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 2);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 200);
        assert_eq!(forest.augmented(root).0, 600);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_rr_grew() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 10, 100u32.into()).unwrap();
        let _ = forest.insert(tree, 5, 50u32.into()).unwrap();
        let _ = forest.insert(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert(tree, 15, 150u32.into()).unwrap();
        let _ = forest.insert(tree, 25, 250u32.into()).unwrap();
        let _ = forest.insert(tree, 23, 230u32.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 3);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 200);
        assert_eq!(forest.augmented(root).0, 980);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_rl_pure() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 10, 100u32.into()).unwrap();
        let _ = forest.insert(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert(tree, 15, 150u32.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 2);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 150);
        assert_eq!(forest.augmented(root).0, 450);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_rl_grew_case_1() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 70, 300u32.into()).unwrap();
        let _ = forest.insert(tree, 65, 350u32.into()).unwrap();
        let _ = forest.insert(tree, 80, 200u32.into()).unwrap();
        let _ = forest.insert(tree, 85, 150u32.into()).unwrap();
        let _ = forest.insert(tree, 75, 250u32.into()).unwrap();
        let _ = forest.insert(tree, 77, 230u32.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 3);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 250);
        assert_eq!(forest.augmented(root).0, 1480);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_rl_grew_case_2() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.append(13).unwrap();
        let _ = forest.insert(tree, 70, 300u32.into()).unwrap();
        let _ = forest.insert(tree, 65, 350u32.into()).unwrap();
        let _ = forest.insert(tree, 80, 200u32.into()).unwrap();
        let _ = forest.insert(tree, 85, 150u32.into()).unwrap();
        let _ = forest.insert(tree, 75, 250u32.into()).unwrap();
        let _ = forest.insert(tree, 73, 270u32.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 3);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 250);
        assert_eq!(forest.augmented(root).0, 1520);
    }

    #[test]
    fn can_augment_sum_of_thousand_nodes_of_fiften_nodes() {
        let number_of_nodes = 15;
        let number_of_trials = 10000;

        let expected_height = avl_max_height(number_of_nodes);
        let expected_sum: u32 = (0..number_of_nodes).sum();

        for _ in 0..number_of_trials {
            let arena: NodeArray<AvlNode<i32, i16, u32, TestU32>, 2000000> = NodeArray::new();
            let mut forest = AvlForest::new(arena);

            let mut rng = rand::rng();
            let tree = forest.append(13).unwrap();

            for i in 0..number_of_nodes {
                let _ = forest.insert(tree, rng.random(), i).unwrap();
            }

            if forest.height(tree) > expected_height {
                forest.print(tree);
                assert!(false);
            }

            if forest.augmented(forest.root(tree)).0 != expected_sum {
                forest.print(tree);
                assert!(false);
            }
        }
    }

    #[test]
    fn can_augment_sum_of_thousand_nodes_of_thirty_one_nodes() {
        let number_of_nodes = 31;
        let number_of_trials = 10000;

        let expected_height = avl_max_height(number_of_nodes);
        let expected_sum: u32 = (0..number_of_nodes).sum();

        for _ in 0..number_of_trials {
            let arena: NodeArray<AvlNode<i32, i16, u32, TestU32>, 2000000> = NodeArray::new();
            let mut forest = AvlForest::new(arena);

            let mut rng = rand::rng();
            let tree = forest.append(13).unwrap();

            for i in 0..number_of_nodes {
                let _ = forest.insert(tree, rng.random(), i).unwrap();
            }

            if forest.height(tree) > expected_height {
                forest.print(tree);
                assert!(false);
            }

            if forest.augmented(forest.root(tree)).0 != expected_sum {
                forest.print(tree);
                assert!(false);
            }
        }
    }

    #[test]
    fn can_augment_sum_of_thousand_nodes_of_sixty_three_nodes() {
        let number_of_nodes = 63;
        let number_of_trials = 10000;

        let expected_height = avl_max_height(number_of_nodes);
        let expected_sum: u32 = (0..number_of_nodes).sum();

        for _ in 0..number_of_trials {
            let arena: NodeArray<AvlNode<i32, i16, u32, TestU32>, 2000000> = NodeArray::new();
            let mut forest = AvlForest::new(arena);

            let mut rng = rand::rng();
            let tree = forest.append(13).unwrap();

            for i in 0..number_of_nodes {
                let _ = forest.insert(tree, rng.random(), i).unwrap();
            }

            if forest.height(tree) > expected_height {
                forest.print(tree);
                assert!(false);
            }

            if forest.augmented(forest.root(tree)).0 != expected_sum {
                forest.print(tree);
                assert!(false);
            }
        }
    }

    #[test]
    fn can_build_avl_tree_of_seven_nodes() {
        let arena: NodeArray<AvlNode<i32, i16, u32, TestU32>, 200> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let items: [i16; 7] = [17032, -22888, 30521, -27236, 20409, -11128, -4109];
        let tree = forest.append(13).unwrap();

        for (i, item) in items.iter().enumerate() {
            let _ = forest.insert(tree, *item, i as u32).unwrap();
        }

        assert_eq!(forest.height(tree), 4);
    }

    #[test]
    fn can_build_avl_tree_of_fifteen_nodes_case_1() {
        let arena: NodeArray<AvlNode<i32, i16, u32, TestU32>, 200> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let items: [i16; 15] = [12887, -17025, -17760, 25232, 1731, 28198, 16485, -29386, -2389, 14664, -12411, 5699, -4286, 27501, 19256];
        let tree = forest.append(13).unwrap();

        for (i, key) in items.iter().enumerate() {
            let _ = forest.insert(tree, *key, i as u32).unwrap();
        }

        assert_eq!(forest.height(tree), 5);
    }

    #[test]
    fn can_build_avl_tree_of_fifteen_nodes_case_2() {
        let arena: NodeArray<AvlNode<i32, i16, u32, TestU32>, 200> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let items: [i16; 15] = [-24585, 9045, -17767, 17675, -8874, 21324, -27870, 22546, -28679, -15606, 29562, 2519, -17342, 1152, 19778];
        let tree = forest.append(13).unwrap();

        for (i, key) in items.iter().enumerate() {
            let _ = forest.insert(tree, *key, i as u32).unwrap();
        }

        assert_eq!(forest.height(tree), 5);
    }

    #[test]
    fn can_build_avl_tree_of_sixty_three_nodes_case_1() {
        let arena: NodeArray<AvlNode<i32, i16, u32, TestU32>, 200> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let items: [i16; 63] = [
            5070, -8319, -17104, -24662, -28470, -28597, -31027, -26592, -21682, -22041, -24460, -22552, -21841, -18621, -17183, -15431, -16194, -16420,
            -16666, -16109, -16118, -11185, -12346, -15218, -12083, -9903, -9233, -1875, -4494, -7764, -6225, -2318, -2461, -2339, -2155, 2455, -1846, 1382,
            4338, 11945, 7138, 6652, 5824, 6792, 11045, 8269, 7687, 9642, 11527, 20798, 18044, 14157, 13864, 17397, 18502, 19930, 24152, 20938, 23705, 32355,
            30116, 32608, 32765,
        ];
        let tree = forest.append(13).unwrap();

        for (i, key) in items.iter().enumerate() {
            let _ = forest.insert(tree, *key, i as u32).unwrap();
        }

        assert_eq!(forest.height(tree), 7);
    }
}
