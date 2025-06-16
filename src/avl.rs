use super::arena::NodeArena;

use std::{fmt::Debug, marker::PhantomData};

// Indicates that the tree grew by one node
const GREW_BIT: u32 = 0x80000000;

// Indicates that the tree shrank by one node
const SHRANK_BIT: u32 = 0x80000000;

// Indicates the balance factor of the node
const BALANCE_BIT: u32 = 0x80000000;

struct InlineStack<T, const U: usize> {
    items: [T; U],
    index: usize,
}

impl<T: Copy + Default, const U: usize> InlineStack<T, U> {
    fn new() -> Self {
        InlineStack { items: [T::default(); U], index: 0 }
    }

    fn empty(&self) -> bool {
        self.index == 0
    }

    fn depth(&self) -> usize {
        self.index
    }

    fn push(&mut self, value: T) {
        unsafe {
            *self.items.get_unchecked_mut(self.index) = value;
            self.index += 1;
        }
    }

    fn pop(&mut self) -> T {
        unsafe {
            self.index -= 1;
            *self.items.get_unchecked(self.index)
        }
    }
}

#[derive(Debug)]
pub enum Balance {
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
struct Item<K: Copy, V: Copy, G: Copy> {
    key: K,
    value: V,
    augment: G,
    left: u32,
    right: u32,
}

#[derive(Copy, Clone)]
union Node<T: Copy, K: Copy, V: Copy, G: Copy> {
    tree: Tree<T>,
    item: Item<K, V, G>,
}

#[derive(Copy, Clone)]
pub struct AvlNode<T: Copy, K: Copy, V: Copy, G: Copy>(Node<T, K, V, G>);

pub trait AvlAugment<K, V, G> {
    fn augment(key: &K, value: &V, left: Option<&G>, right: Option<&G>) -> G;
}

impl<K, V> AvlAugment<K, V, ()> for () {
    fn augment(_: &K, _: &V, _: Option<&()>, _: Option<&()>) -> () {
        ()
    }
}

pub trait AvlSearch<K: Copy, G: Copy> {
    // extract ranges from the augmented value
    fn extract(value: &G) -> (K, K);

    // appends augmeneted value to the result
    fn append(&mut self, value: &G);
}

pub trait AvlLike<T: Copy, K: Copy, V: Copy, G: Copy> {
    fn from_node(node: AvlNode<T, K, V, G>) -> Self;
    fn as_ref(&self) -> &AvlNode<T, K, V, G>;
    fn as_mut(&mut self) -> &mut AvlNode<T, K, V, G>;
}

impl<T: Copy, K: Copy, V: Copy, G: Copy> AvlLike<T, K, V, G> for AvlNode<T, K, V, G> {
    #[inline(always)]
    fn from_node(node: AvlNode<T, K, V, G>) -> Self {
        node
    }

    #[inline(always)]
    fn as_ref(&self) -> &AvlNode<T, K, V, G> {
        self
    }

    #[inline(always)]
    fn as_mut(&mut self) -> &mut AvlNode<T, K, V, G> {
        self
    }
}

pub trait AvlLogger<K, V> {
    fn on_insert(parent: (u32, K), node: (u32, K));
    fn on_remove(parent: (u32, K), node: (u32, K));
    fn on_rotate(node: (u32, K));
    fn on_rebalance(node: (u32, K), balance: Balance);
}

pub struct NoLogger {}
impl<K, V> AvlLogger<K, V> for NoLogger {
    fn on_insert(_parent: (u32, K), _node: (u32, K)) {}
    fn on_remove(_parent: (u32, K), _node: (u32, K)) {}
    fn on_rotate(_node: (u32, K)) {}
    fn on_rebalance(_node: (u32, K), _balance: Balance) {}
}

pub struct DebugLogger {}
impl<K: Debug, V: Debug> AvlLogger<K, V> for DebugLogger {
    fn on_insert(parent: (u32, K), node: (u32, K)) {
        println!("inserting {}|{:?} under {}|{:?}", node.0, node.1, parent.0, parent.1);
    }

    fn on_remove(parent: (u32, K), node: (u32, K)) {
        println!("removing {}|{:?} from {}|{:?}", node.0, node.1, parent.0, parent.1);
    }

    fn on_rotate(node: (u32, K)) {
        println!("rotating {}|{:?}", node.0, node.1);
    }

    fn on_rebalance(node: (u32, K), balance: Balance) {
        println!("rebalancing {}|{:?} to {:?}", node.0, node.1, balance);
    }
}

impl<T: Copy, K: Copy, V: Copy, G: Copy> Node<T, K, V, G> {
    fn node(key: K, value: V) -> AvlNode<T, K, V, G>
    where
        G: AvlAugment<K, V, G>,
    {
        AvlNode(Node { item: Item { key, value, left: 0, right: 0, augment: G::augment(&key, &value, None, None) } })
    }

    #[inline(always)]
    fn tree(value: T) -> AvlNode<T, K, V, G> {
        AvlNode(Node { tree: Tree { root: 0, value } })
    }

    #[inline(always)]
    fn get_root(&self) -> u32 {
        unsafe { self.tree.root }
    }

    #[inline(always)]
    fn set_root(&mut self, root: u32) {
        self.tree.root = root;
    }

    #[inline(always)]
    fn get_key(&self) -> K {
        unsafe { self.item.key }
    }

    #[inline(always)]
    fn set_key(&mut self, key: K) {
        self.item.key = key;
    }

    #[inline(always)]
    fn get_value(&self) -> V {
        unsafe { self.item.value }
    }

    #[inline(always)]
    fn set_value(&mut self, value: V) {
        self.item.value = value;
    }

    #[inline(always)]
    fn get_augmented(&self) -> G {
        unsafe { self.item.augment }
    }

    #[inline(always)]
    fn set_augmented(&mut self, augment: G) {
        self.item.augment = augment;
    }

    #[inline(always)]
    fn get_balance(&self) -> Balance {
        // balance is stored in the left and right fields of the item
        // because each side can only sacrifice one bit for balance
        let left = unsafe { self.item.left & BALANCE_BIT };
        let right = unsafe { self.item.right & BALANCE_BIT };

        match (left, right) {
            (_, BALANCE_BIT) => return Balance::RightHeavy,
            (BALANCE_BIT, _) => return Balance::LeftHeavy,
            (_, _) => return Balance::Equal,
        }
    }

    #[inline(always)]
    fn set_balance(&mut self, balance: Balance) {
        // similarly here we store the balance
        // in the left and right fields of the item
        unsafe {
            match balance {
                Balance::Equal => {
                    self.item.left &= !BALANCE_BIT;
                    self.item.right &= !BALANCE_BIT;
                }
                Balance::LeftHeavy => {
                    self.item.left |= BALANCE_BIT;
                    self.item.right &= !BALANCE_BIT;
                }
                Balance::RightHeavy => {
                    self.item.left &= !BALANCE_BIT;
                    self.item.right |= BALANCE_BIT;
                }
            }
        }
    }

    #[inline(always)]
    fn get_left(&self) -> u32 {
        unsafe { self.item.left & !BALANCE_BIT }
    }

    #[inline(always)]
    fn set_left(&mut self, left: u32) {
        unsafe { self.item.left = left | self.item.left & BALANCE_BIT };
    }

    #[inline(always)]
    fn get_right(&self) -> u32 {
        unsafe { self.item.right & !BALANCE_BIT }
    }

    #[inline(always)]
    fn set_right(&mut self, right: u32) {
        unsafe { self.item.right = right | self.item.right & BALANCE_BIT };
    }
}

pub struct AvlForest<T: Copy, K: Copy, V: Copy, G: Copy, N: Copy, A: NodeArena<N>, L: AvlLogger<K, V>> {
    arena: A,
    key: PhantomData<K>,
    value: PhantomData<V>,
    augment: PhantomData<G>,
    tree: PhantomData<T>,
    into: PhantomData<N>,
    logger: PhantomData<L>,
}

impl<T: Copy, K: Copy, V: Copy, G: Copy, N: Copy, A: NodeArena<N>> AvlForest<T, K, V, G, N, A, NoLogger> {
    pub fn new(arena: A) -> Self {
        AvlForest { arena, key: PhantomData, value: PhantomData, augment: PhantomData, tree: PhantomData, into: PhantomData, logger: PhantomData }
    }
}

impl<T: Copy, K: Copy + Debug, V: Copy + Debug, G: Copy, N: Copy, A: NodeArena<N>> AvlForest<T, K, V, G, N, A, DebugLogger> {
    pub fn debug(arena: A) -> Self {
        AvlForest { arena, key: PhantomData, value: PhantomData, augment: PhantomData, tree: PhantomData, into: PhantomData, logger: PhantomData }
    }
}

impl<T: Copy, K: Copy, V: Copy, N: Copy, G: Copy, A, L> AvlForest<T, K, V, G, N, A, L>
where
    N: AvlLike<T, K, V, G>,
    A: NodeArena<N>,
    L: AvlLogger<K, V>,
{
    #[inline(always)]
    unsafe fn get_ref(&self, node: u32) -> &Node<T, K, V, G> {
        let node: &N = unsafe { self.arena.get_unchecked(node) };
        let avl: &AvlNode<T, K, V, G> = node.as_ref();

        return &avl.0;
    }

    #[inline(always)]
    unsafe fn get_mut(&mut self, node: u32) -> &mut Node<T, K, V, G> {
        let node: &mut N = unsafe { self.arena.get_unchecked_mut(node) };
        let avl: &mut AvlNode<T, K, V, G> = node.as_mut();

        return &mut avl.0;
    }

    #[inline(always)]
    pub fn root(&self, tree: u32) -> u32 {
        unsafe { self.get_ref(tree).get_root() }
    }

    #[inline(always)]
    pub fn value(&self, node: u32) -> V {
        unsafe { self.get_ref(node).get_value() }
    }

    #[inline(always)]
    pub fn augmented(&self, node: u32) -> G {
        unsafe { self.get_ref(node).get_augmented() }
    }

    pub fn height(&self, tree: u32) -> u32 {
        // initially we start with the height of 0 and the root of the tree
        let mut height = 0;
        let mut idx = self.root(tree);

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
}

impl<T: Copy, K: Copy, V: Copy, N: Copy, G: Copy, A, L> AvlForest<T, K, V, G, N, A, L>
where
    G: AvlAugment<K, V, G>,
    N: AvlLike<T, K, V, G>,
    A: NodeArena<N>,
    L: AvlLogger<K, V>,
{
    pub fn search<S>(&self, tree: u32, search: &mut S, from: K, to: K)
    where
        K: PartialOrd,
        S: AvlSearch<K, G>,
    {
        self.search_recursive(self.root(tree), search, from, to);
    }

    fn search_recursive<S>(&self, idx: u32, search: &mut S, from: K, to: K)
    where
        K: PartialOrd,
        S: AvlSearch<K, G>,
    {
        if idx == 0 {
            return;
        }

        let node = unsafe { self.get_ref(idx) };
        let key = node.get_key();
        let augment = node.get_augmented();
        let range = S::extract(&augment);

        // if the range is outside the search bounds, skip this node
        if range.1 < from || range.0 > to {
            return;
        }

        // if the range is fully inside the search bounds, append the augment
        if range.0 >= from && range.1 <= to {
            search.append(&augment);
            return;
        }

        // if the range is exactly the key of the node, append the augment
        if from <= key && key <= to {
            search.append(&G::augment(&key, &node.get_value(), None, None));
        }

        // partially inside the range, visit both branches
        self.search_recursive(node.get_left(), search, from, to);
        self.search_recursive(node.get_right(), search, from, to);
    }
}

impl<T: Copy, K: Copy, V: Copy, N: Copy, G: Copy, A, L> AvlForest<T, K, V, G, N, A, L>
where
    G: AvlAugment<K, V, G>,
    N: AvlLike<T, K, V, G>,
    A: NodeArena<N>,
    L: AvlLogger<K, V>,
{
    #[inline(always)]
    pub fn insert_tree(&mut self, value: T) -> Option<u32> {
        // return the index of the newly inserted node as the root of the tree
        self.arena.insert(N::from_node(Node::tree(value)))
    }

    pub fn insert_element(&mut self, tree: u32, key: K, value: V) -> Option<u32>
    where
        K: PartialOrd,
    {
        // allocate a new node in the arena
        let idx = self.arena.insert(N::from_node(Node::node(key, value)))?;

        // find the root of the tree
        let parent = unsafe { self.get_ref(tree).get_root() };

        // trigger recursive insertion, may rotate the root
        let rotated = unsafe { self.insert_recursive(parent, idx, key) & !GREW_BIT };

        // update the root of the tree if it was rotated
        unsafe { self.get_mut(tree).set_root(rotated) };

        // return the index of the newly inserted node
        return Some(idx);
    }

    unsafe fn insert_recursive(&mut self, parent: u32, node: u32, key: K) -> u32
    where
        K: PartialOrd,
    {
        // recursion base case
        if parent == 0 {
            return node | GREW_BIT;
        }

        let idx = parent;
        let parent = unsafe { self.get_ref(idx) };
        let pkey = parent.get_key();

        unsafe {
            if key < pkey {
                L::on_insert((idx, pkey), (node, key));
                let left = self.insert_recursive(parent.get_left(), node, key);
                let (left, grew) = (left & !GREW_BIT, left & GREW_BIT);

                self.get_mut(idx).set_left(left);
                self.update_augmented(idx);

                if grew == 0 {
                    return idx;
                }

                match self.get_ref(idx).get_balance() {
                    Balance::Equal => {
                        L::on_rebalance((idx, pkey), Balance::LeftHeavy);
                        self.get_mut(idx).set_balance(Balance::LeftHeavy);
                        return idx | GREW_BIT;
                    }
                    Balance::RightHeavy => {
                        L::on_rebalance((idx, pkey), Balance::Equal);
                        self.get_mut(idx).set_balance(Balance::Equal);
                    }
                    Balance::LeftHeavy => match self.get_ref(left).get_balance() {
                        Balance::LeftHeavy => {
                            L::on_rotate((idx, pkey));
                            return self.rotate_ll(idx, true); // didn't grow
                        }
                        Balance::RightHeavy => {
                            L::on_rotate((idx, pkey));
                            return self.rotate_lr(idx); // didn't grow
                        }
                        Balance::Equal => {
                            // won't happen, because grow cannot lead to equal balance
                        }
                    },
                }

                return idx; // didn't grow
            }
        }

        unsafe {
            if key > pkey {
                L::on_insert((idx, pkey), (node, key));
                let right = self.insert_recursive(parent.get_right(), node, key);
                let (right, grew) = (right & !GREW_BIT, right & GREW_BIT);

                self.get_mut(idx).set_right(right);
                self.update_augmented(idx);

                if grew == 0 {
                    return idx;
                }

                match self.get_ref(idx).get_balance() {
                    Balance::Equal => {
                        L::on_rebalance((idx, pkey), Balance::RightHeavy);
                        self.get_mut(idx).set_balance(Balance::RightHeavy);
                        return idx | GREW_BIT;
                    }
                    Balance::LeftHeavy => {
                        L::on_rebalance((idx, pkey), Balance::Equal);
                        self.get_mut(idx).set_balance(Balance::Equal);
                    }
                    Balance::RightHeavy => match self.get_ref(right).get_balance() {
                        Balance::RightHeavy => {
                            L::on_rotate((idx, pkey));
                            return self.rotate_rr(idx, true); // didn't grow
                        }
                        Balance::LeftHeavy => {
                            L::on_rotate((idx, pkey));
                            return self.rotate_rl(idx); // didn't grow
                        }
                        Balance::Equal => {
                            // won't happen, because grow cannot lead to equal balance
                        }
                    },
                }

                return idx; // didn't grow
            }
        }

        // we do knowing, but disposing already allocated node
        unsafe { self.arena.release_unchecked(node) };
        return idx;
    }

    pub fn remove_element(&mut self, tree: u32, key: K)
    where
        K: PartialOrd,
    {
        // find the root of the tree
        let parent = unsafe { self.get_ref(tree).get_root() };

        // trigger recursive removal, may rotate the root
        let rotated = unsafe { self.remove_recursive(parent, key) & !SHRANK_BIT };

        // update the root of the tree if it was rotated
        unsafe { self.get_mut(tree).set_root(rotated) };
    }

    unsafe fn remove_recursive(&mut self, parent: u32, key: K) -> u32
    where
        K: PartialOrd,
    {
        // recursion base case
        if parent == 0 {
            return 0;
        }

        let idx = parent;
        let parent = unsafe { self.get_ref(idx) };
        let pkey = parent.get_key();

        unsafe {
            if key < pkey {
                L::on_remove((idx, pkey), (parent.get_left(), key));
                let left = self.remove_recursive(parent.get_left(), key);
                let (left, shrank) = (left & !SHRANK_BIT, left & SHRANK_BIT);

                self.get_mut(idx).set_left(left);
                self.update_augmented(idx);

                if shrank == 0 {
                    return idx;
                }

                match self.get_ref(idx).get_balance() {
                    Balance::LeftHeavy => {
                        L::on_rebalance((idx, pkey), Balance::Equal);
                        self.get_mut(idx).set_balance(Balance::Equal);
                        return idx | SHRANK_BIT;
                    }
                    Balance::Equal => {
                        L::on_rebalance((idx, pkey), Balance::LeftHeavy);
                        self.get_mut(idx).set_balance(Balance::RightHeavy);
                    }
                    Balance::RightHeavy => {
                        let right = self.get_ref(idx).get_right();
                        match self.get_ref(right).get_balance() {
                            Balance::Equal => {
                                L::on_rotate((idx, pkey));
                                return self.rotate_rr(idx, false); // didn't shrink
                            }
                            Balance::RightHeavy => {
                                L::on_rotate((idx, pkey));
                                return self.rotate_rr(idx, true) | SHRANK_BIT;
                            }
                            Balance::LeftHeavy => {
                                L::on_rotate((idx, pkey));
                                return self.rotate_rl(idx) | SHRANK_BIT;
                            }
                        }
                    }
                }

                return idx;
            }
        }

        unsafe {
            if key > pkey {
                L::on_remove((idx, pkey), (parent.get_right(), key));
                let right = self.remove_recursive(parent.get_right(), key);
                let (right, shrank) = (right & !SHRANK_BIT, right & SHRANK_BIT);

                self.get_mut(idx).set_right(right);
                self.update_augmented(idx);

                if shrank == 0 {
                    return idx;
                }

                match self.get_ref(idx).get_balance() {
                    Balance::RightHeavy => {
                        L::on_rebalance((idx, pkey), Balance::RightHeavy);
                        self.get_mut(idx).set_balance(Balance::Equal);
                        return idx | SHRANK_BIT;
                    }
                    Balance::Equal => {
                        L::on_rebalance((idx, pkey), Balance::Equal);
                        self.get_mut(idx).set_balance(Balance::LeftHeavy);
                    }
                    Balance::LeftHeavy => {
                        let left = self.get_ref(idx).get_left();
                        match self.get_ref(left).get_balance() {
                            Balance::Equal => {
                                L::on_rotate((idx, pkey));
                                return self.rotate_ll(idx, false); // didn't shrink
                            }
                            Balance::LeftHeavy => {
                                L::on_rotate((idx, pkey));
                                return self.rotate_ll(idx, true) | SHRANK_BIT;
                            }
                            Balance::RightHeavy => {
                                L::on_rotate((idx, pkey));
                                return self.rotate_lr(idx) | SHRANK_BIT;
                            }
                        }
                    }
                }

                return idx;
            }
        }

        unsafe {
            let left = parent.get_left();
            let right = parent.get_right();

            // if the node is a leaf, we can just remove it
            if left == 0 && right == 0 {
                self.arena.release_unchecked(idx);
                return 0 | SHRANK_BIT; // we shrank the tree
            }

            // we can just replace the node with its right child
            if left == 0 {
                self.arena.release_unchecked(idx);
                return right | SHRANK_BIT; // we shrank the tree
            }

            // we can just replace the node with its left child
            if right == 0 {
                self.arena.release_unchecked(idx);
                return left | SHRANK_BIT; // we shrank the tree
            }

            // we need to find the rightmost node in the left subtree
            let mut rightmost = left;
            while self.get_ref(rightmost).get_right() > 0 {
                rightmost = self.get_ref(rightmost).get_right();
            }

            // we can replace the current node with the rightmost node
            let key = self.get_ref(rightmost).get_key();
            let value = self.get_ref(rightmost).get_value();

            self.get_mut(idx).set_key(key);
            self.get_mut(idx).set_value(value);

            // we can remove the rightmost node from the left subtree
            let left = self.remove_recursive(left, key);
            let (left, shrank) = (left & !SHRANK_BIT, left & SHRANK_BIT);

            self.get_mut(idx).set_left(left);
            self.update_augmented(idx);

            if shrank == 0 {
                return idx; // didn't shrink
            }

            match self.get_ref(idx).get_balance() {
                Balance::LeftHeavy => {
                    L::on_rebalance((idx, pkey), Balance::LeftHeavy);
                    self.get_mut(idx).set_balance(Balance::Equal);
                    return idx | SHRANK_BIT; // we shrank the tree
                }
                Balance::Equal => {
                    L::on_rebalance((idx, pkey), Balance::Equal);
                    self.get_mut(idx).set_balance(Balance::RightHeavy);
                    return idx; // didn't shrink
                }
                Balance::RightHeavy => {
                    let right = self.get_ref(idx).get_right();
                    match self.get_ref(right).get_balance() {
                        Balance::Equal => {
                            L::on_rotate((idx, pkey));
                            return self.rotate_rr(idx, false); // didn't shrink
                        }
                        Balance::RightHeavy => {
                            L::on_rotate((idx, pkey));
                            return self.rotate_rr(idx, true) | SHRANK_BIT; // we shrank the tree
                        }
                        Balance::LeftHeavy => {
                            L::on_rotate((idx, pkey));
                            return self.rotate_rl(idx) | SHRANK_BIT; // we shrank the tree
                        }
                    }
                }
            }
        }
    }

    unsafe fn update_augmented(&mut self, node: u32) {
        // we need to find left and right children indices
        let left = unsafe { self.get_ref(node).get_left() };
        let right = unsafe { self.get_ref(node).get_right() };

        // if the left or right index is not 0, we can get the values
        let left = if left == 0 { None } else { Some(unsafe { self.get_ref(left).get_augmented() }) };
        let right = if right == 0 { None } else { Some(unsafe { self.get_ref(right).get_augmented() }) };

        // now we can get the key, value of the current node and augment it
        let key = unsafe { self.get_ref(node).get_key() };
        let value = unsafe { self.get_ref(node).get_value() };
        let augmented = G::augment(&key, &value, left.as_ref(), right.as_ref());

        // and finally we can set the augmented value of the current node
        unsafe { self.get_mut(node).set_augmented(augmented) };
    }

    // Rotate left-left case (grew at x):
    //
    //         [h+2] z (-1)                  [h+3] z (-2)                 [h+2] y (0)
    //              / \                           / \                          / \
    //   [h+1] (0) y   b [h]    =>    [h+2] (-1) y   b [h]    =>    [h+1] (0) x   z (0) [h+1]
    //            / \                           / \                              / \
    //       [h] x   a [h]               [h+1] x   a [h]                    [h] a   b [h]
    //
    // Rotate left-left case (shrank at b and y's balance was 0):
    //
    //         [h+2] z (-1)                 [h+2] z (-2)              [h+2] y (+1)
    //              / \                          / \                       / \
    //   [h+1] (0) y   b [h]    =>    [h+1] (0) y   b [h-1]    =>     [h] x   z (-1) [h+1]
    //            / \                          / \                           / \
    //       [h] x   a [h]                [h] x   a [h]                 [h] a   b [h-1]
    //
    // Rotate left-left case (shrank b and y's balance was -1):
    //
    //          [h+2] z (-1)                  [h+2] z (-2)              [h+1] y (0)
    //               / \                           / \                       / \
    //   [h+1] (-1) y   b [h]    =>    [h+1] (-1) y   b [h-1]    =>     [h] x   z (0) [h]
    //             / \                           / \                           / \
    //        [h] x   a [h-1]               [h] x   a [h-1]             [h-1] a   b [h-1]
    //
    fn rotate_ll(&mut self, z: u32, zero: bool) -> u32 {
        unsafe {
            // get indices of the nodes involved in the rotation
            let y = self.get_ref(z).get_left();
            let a = self.get_ref(y).get_right();

            // relink the nodes
            self.get_mut(z).set_left(a);
            self.get_mut(y).set_right(z);

            // adjust balances of z and y
            if zero {
                self.get_mut(z).set_balance(Balance::Equal);
                self.get_mut(y).set_balance(Balance::Equal);
            } else {
                self.get_mut(z).set_balance(Balance::LeftHeavy);
                self.get_mut(y).set_balance(Balance::RightHeavy);
            }

            // update the augmented values of the nodes
            self.update_augmented(z);
            self.update_augmented(y);

            // return y as the new root of the subtree
            return y;
        }
    }

    // Rotate left-right case (grew at x and y's balance was 0):
    // - notice that the balance of y or z depends on the balance of x
    //
    //         [h+2] z (-1)                 [h+3] z (-2)                  [h+2] x (0)
    //              / \                          / \                           / \
    //   [h+1] (0) y   d [h]    =>   [h+2] (+1) y   d [h]    =>    [h+1] (?) y   z (?) [h+1]
    //            / \                          / \                         / \   / \
    //       [h] c   x [h]                [h] c   x (?) (h+1)         [h] c  a   b  d [h]
    //              / \                          / \
    //             a   b                    [h] a   b [h]
    //
    // Rotate left-right case (shrank at d and y's balance was +1):
    // - notice that the balance of y or z depends on the balance of x
    //
    //         [h+2] z (-1)                 [h+2] z (-2)                    [h+1] x (0)
    //              / \                          / \                             / \
    //  [h+1] (+1) y   d [h]    =>   [h+1] (+1) y   d [h-1]    =>      [h] (?) y   z (?) [h]
    //            / \                          / \                            / \   / \
    //     [h-1] c   x [h]              [h-1] c   x (?) [h]            [h-1] c  a   b  d [h-1]
    //              / \                          / \
    //             a   b                        a   b
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
    //          z (+2)                   y (0)
    //         / \                      / \
    //    (?) b   y (+1)    =>     (0) z   x (?)
    //           / \                  / \
    //      (?) a   x (?)        (?) b   a (?)
    //
    //          z (+2)                   y (-1)
    //         / \                      / \
    //    (?) b   y (0)     =>    (+1) z   x (?)
    //           / \                  / \
    //      (?) a   x (?)        (?) b   a (?)
    //
    fn rotate_rr(&mut self, z: u32, zero: bool) -> u32 {
        unsafe {
            // get indices of the nodes involved in the rotation
            let y = self.get_ref(z).get_right();
            let a = self.get_ref(y).get_left();

            // relink the nodes
            self.get_mut(z).set_right(a);
            self.get_mut(y).set_left(z);

            if zero {
                // adjust balances of z and y in basic case
                self.get_mut(z).set_balance(Balance::Equal);
                self.get_mut(y).set_balance(Balance::Equal);
            } else {
                // adjust balances of z and y in delete case
                self.get_mut(z).set_balance(Balance::RightHeavy);
                self.get_mut(y).set_balance(Balance::LeftHeavy);
            }

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
    //           / \                       / \
    //      (?) d   y (-1)    =>   (-1/0) z   y (+1/0)
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

impl<T: Copy + Debug, K: Copy + Debug + PartialOrd, V: Copy + Debug, N: Copy, G: Copy + Debug, A> AvlForest<T, K, V, G, N, A, DebugLogger>
where
    G: Debug,
    N: AvlLike<T, K, V, G>,
    A: NodeArena<N>,
{
    pub fn print(&self, tree: u32) {
        let mut stack = InlineStack::<(u32, usize), 64>::new();
        let (mut idx, mut depth) = (self.root(tree), 0);

        while idx != 0 || !stack.empty() {
            while idx != 0 {
                let node = unsafe { self.get_ref(idx) };
                let (key, val) = (node.get_key(), node.get_value());
                let (aug, bal) = (node.get_augmented(), node.get_balance());

                println!("{:depth$}- idx={}; key={:?}; val={:?}|{:?}; {:?}", "", idx, key, val, aug, bal);

                stack.push((node.get_right(), depth + 1));
                (idx, depth) = (node.get_left(), depth + 1);
            }

            println!("{:depth$}- nil", "");

            if !stack.empty() {
                (idx, depth) = stack.pop();
            }
        }

        println!("{:depth$}- nil", "");
    }
}

#[cfg(test)]
mod tests {
    use std::cmp;

    use super::*;
    use crate::arena::NodeArray;
    use rand::seq::SliceRandom;

    #[derive(Copy, Clone, Debug)]
    struct TestU32(u32);

    impl<T> AvlAugment<T, u32, TestU32> for TestU32 {
        fn augment(_key: &T, value: &u32, left: Option<&TestU32>, right: Option<&TestU32>) -> TestU32 {
            let left = left.map_or(0, |l| l.0);
            let right = right.map_or(0, |r| r.0);

            return TestU32(left + right + value);
        }
    }

    #[derive(Copy, Clone, Debug)]
    struct MinMaxCount {
        min: u32,
        max: u32,
        sum: u64,
        count: u64,
    }

    impl AvlAugment<u32, u32, MinMaxCount> for MinMaxCount {
        fn augment(key: &u32, value: &u32, left: Option<&MinMaxCount>, right: Option<&MinMaxCount>) -> MinMaxCount {
            let min = match (left, right) {
                (Some(left), Some(right)) => cmp::min(left.min, right.min),
                (Some(left), None) => left.min,
                (None, Some(right)) => right.min,
                (None, None) => *key,
            };

            let max = match (left, right) {
                (Some(left), Some(right)) => cmp::max(left.max, right.max),
                (Some(left), None) => left.max,
                (None, Some(right)) => right.max,
                (None, None) => *key,
            };

            let count = match (left, right) {
                (Some(left), Some(right)) => 1 + left.count + right.count,
                (Some(left), None) => left.count,
                (None, Some(right)) => right.count,
                (None, None) => 1,
            };

            let sum = match (left, right) {
                (Some(left), Some(right)) => *value as u64 + left.sum + right.sum,
                (Some(left), None) => *value as u64 + left.sum,
                (None, Some(right)) => *value as u64 + right.sum,
                (None, None) => *value as u64,
            };

            return MinMaxCount { min, max, count, sum };
        }
    }

    struct RangeSearch {
        sum: u64,
        count: u64,
    }

    impl RangeSearch {
        fn new() -> Self {
            RangeSearch { sum: 0, count: 0 }
        }

        fn sum(&self) -> u64 {
            self.sum
        }

        fn count(&self) -> u64 {
            self.count
        }
    }

    impl AvlSearch<u32, MinMaxCount> for RangeSearch {
        fn extract(augmented: &MinMaxCount) -> (u32, u32) {
            (augmented.min, augmented.max)
        }

        fn append(&mut self, value: &MinMaxCount) {
            self.sum += value.sum;
            self.count += value.count;
        }
    }

    #[derive(Copy, Clone)]
    union Foreign {
        avl: AvlNode<i32, i32, u32, TestU32>,
    }

    impl AvlLike<i32, i32, u32, TestU32> for Foreign {
        fn from_node(node: AvlNode<i32, i32, u32, TestU32>) -> Self {
            Foreign { avl: node }
        }

        fn as_ref(&self) -> &AvlNode<i32, i32, u32, TestU32> {
            unsafe { &self.avl }
        }

        fn as_mut(&mut self) -> &mut AvlNode<i32, i32, u32, TestU32> {
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

        let tree = forest.insert_tree(13);
        assert!(tree.is_some());

        let root = forest.insert_element(tree.unwrap(), 1, 2u32.into());
        assert!(root.is_some());

        let height = forest.height(tree.unwrap());
        assert_eq!(height, 1);
    }

    #[test]
    fn can_create_avl_forest_with_foreign_node() {
        let arena: NodeArray<Foreign, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(13);
        assert!(tree.is_some());

        let root = forest.insert_element(tree.unwrap(), 1, 2u32.into());
        assert!(root.is_some());

        let height = forest.height(tree.unwrap());
        assert_eq!(height, 1);
    }

    #[test]
    fn can_insert_nodes_into_avl_forest_ll_pure() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();

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

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 35, 350u32.into()).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();
        let _ = forest.insert_element(tree, 25, 250u32.into()).unwrap();
        let _ = forest.insert_element(tree, 5, 50u32.into()).unwrap();

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

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 25, 250u32.into()).unwrap();

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

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 35, 350u32.into()).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 15, 150u32.into()).unwrap();
        let _ = forest.insert_element(tree, 25, 250u32.into()).unwrap();
        let _ = forest.insert_element(tree, 23, 230u32.into()).unwrap();

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

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 35, 350u32.into()).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 15, 150u32.into()).unwrap();
        let _ = forest.insert_element(tree, 25, 250u32.into()).unwrap();
        let _ = forest.insert_element(tree, 27, 270u32.into()).unwrap();

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

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();

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

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();
        let _ = forest.insert_element(tree, 5, 50u32.into()).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 15, 150u32.into()).unwrap();
        let _ = forest.insert_element(tree, 25, 250u32.into()).unwrap();
        let _ = forest.insert_element(tree, 23, 230u32.into()).unwrap();

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

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 15, 150u32.into()).unwrap();

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

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 70, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 65, 350u32.into()).unwrap();
        let _ = forest.insert_element(tree, 80, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 85, 150u32.into()).unwrap();
        let _ = forest.insert_element(tree, 75, 250u32.into()).unwrap();
        let _ = forest.insert_element(tree, 77, 230u32.into()).unwrap();

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

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 70, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 65, 350u32.into()).unwrap();
        let _ = forest.insert_element(tree, 80, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 85, 150u32.into()).unwrap();
        let _ = forest.insert_element(tree, 75, 250u32.into()).unwrap();
        let _ = forest.insert_element(tree, 73, 270u32.into()).unwrap();

        let height = forest.height(tree);
        assert_eq!(height, 3);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 250);
        assert_eq!(forest.augmented(root).0, 1520);
    }

    #[test]
    fn can_augment_sum_of_fifteen_nodes() {
        let number_of_nodes = 15;
        let number_of_trials = 10000;

        let mut rng = rand::rng();
        let expected_height = avl_max_height(number_of_nodes);
        let expected_sum: u32 = (0..number_of_nodes).sum();

        for _ in 0..number_of_trials {
            let arena: NodeArray<AvlNode<i32, i16, u32, TestU32>, 2000000> = NodeArray::new();
            let mut forest = AvlForest::debug(arena);

            let tree = forest.insert_tree(13).unwrap();
            let mut numbers: Vec<i16> = (0..number_of_trials).collect();

            numbers.shuffle(&mut rng);
            for i in 0..number_of_nodes {
                let _ = forest.insert_element(tree, numbers[i as usize], i).unwrap();
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
    fn can_augment_sum_of_thirty_one_nodes() {
        let number_of_nodes = 31;
        let number_of_trials = 10000;

        let mut rng = rand::rng();
        let expected_height = avl_max_height(number_of_nodes);
        let expected_sum: u32 = (0..number_of_nodes).sum();

        for _ in 0..number_of_trials {
            let arena: NodeArray<AvlNode<i32, i16, u32, TestU32>, 2000000> = NodeArray::new();
            let mut forest = AvlForest::debug(arena);

            let tree = forest.insert_tree(13).unwrap();
            let mut numbers: Vec<i16> = (0..number_of_nodes as i16).collect();

            numbers.shuffle(&mut rng);
            for i in 0..number_of_nodes {
                let _ = forest.insert_element(tree, numbers[i as usize], i).unwrap();
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
    fn can_augment_sum_of_sixty_three_nodes() {
        let number_of_nodes = 63;
        let number_of_trials = 10000;

        let mut rng = rand::rng();
        let expected_height = avl_max_height(number_of_nodes);
        let expected_sum: u32 = (0..number_of_nodes).sum();

        for _ in 0..number_of_trials {
            let arena: NodeArray<AvlNode<i32, i16, u32, TestU32>, 2000000> = NodeArray::new();
            let mut forest = AvlForest::debug(arena);

            let tree = forest.insert_tree(13).unwrap();
            let mut numbers: Vec<i16> = (0..number_of_nodes as i16).collect();

            numbers.shuffle(&mut rng);
            for i in 0..number_of_nodes {
                let _ = forest.insert_element(tree, numbers[i as usize], i).unwrap();
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
        let tree = forest.insert_tree(13).unwrap();

        for (i, item) in items.iter().enumerate() {
            let _ = forest.insert_element(tree, *item, i as u32).unwrap();
        }

        assert_eq!(forest.height(tree), 4);
    }

    #[test]
    fn can_build_avl_tree_of_fifteen_nodes_case_1() {
        let arena: NodeArray<AvlNode<i32, i16, u32, TestU32>, 200> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let items: [i16; 15] = [12887, -17025, -17760, 25232, 1731, 28198, 16485, -29386, -2389, 14664, -12411, 5699, -4286, 27501, 19256];
        let tree = forest.insert_tree(13).unwrap();

        for (i, key) in items.iter().enumerate() {
            let _ = forest.insert_element(tree, *key, i as u32).unwrap();
        }

        assert_eq!(forest.height(tree), 5);
    }

    #[test]
    fn can_build_avl_tree_of_fifteen_nodes_case_2() {
        let arena: NodeArray<AvlNode<i32, i16, u32, TestU32>, 200> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let items: [i16; 15] = [-24585, 9045, -17767, 17675, -8874, 21324, -27870, 22546, -28679, -15606, 29562, 2519, -17342, 1152, 19778];
        let tree = forest.insert_tree(13).unwrap();

        for (i, key) in items.iter().enumerate() {
            let _ = forest.insert_element(tree, *key, i as u32).unwrap();
        }

        assert_eq!(forest.height(tree), 5);
    }

    #[test]
    fn can_build_avl_tree_of_sixty_three_nodes_case_1() {
        let arena: NodeArray<AvlNode<i32, i16, u32, TestU32>, 200> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let items: [i16; 63] = [
            5070, -8319, -17104, -24662, -28470, -28597, -31027, -26592, -21682, -22041, -24460, -22552, -21841, -18621, -17183, -15431, -16194, -16420,
            -16666, -16109, -16118, -11185, -12346, -15218, -12083, -9903, -9233, -1875, -4494, -7764, -6225, -2318, -2461, -2339, -2155, 2455, -1846, 1382,
            4338, 11945, 7138, 6652, 5824, 6792, 11045, 8269, 7687, 9642, 11527, 20798, 18044, 14157, 13864, 17397, 18502, 19930, 24152, 20938, 23705, 32355,
            30116, 32608, 32765,
        ];
        let tree = forest.insert_tree(13).unwrap();

        for (i, key) in items.iter().enumerate() {
            let _ = forest.insert_element(tree, *key, i as u32).unwrap();
        }

        assert_eq!(forest.height(tree), 7);
    }

    #[test]
    fn can_remove_from_tree_no_children_no_rotation_left() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();

        forest.remove_element(tree, 10);
        let height = forest.height(tree);
        assert_eq!(height, 2);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 200);
    }

    #[test]
    fn can_remove_from_tree_no_children_no_rotation_right() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();

        forest.remove_element(tree, 30);
        let height = forest.height(tree);
        assert_eq!(height, 2);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 200);
    }

    #[test]
    fn can_remove_from_tree_no_children_no_rotation_root() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();

        forest.remove_element(tree, 20);
        let height = forest.height(tree);
        assert_eq!(height, 2);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 100);
    }

    #[test]
    fn can_remove_from_five_no_children_rr_left_child() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();
        let _ = forest.insert_element(tree, 35, 300u32.into()).unwrap();

        // it will trigger a right-right rotation
        forest.remove_element(tree, 10);
        let height = forest.height(tree);
        assert_eq!(height, 2);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 300);
        assert_eq!(forest.augmented(root).0, 800);
    }

    #[test]
    fn can_remove_from_five_no_children_ll_right_child() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();
        let _ = forest.insert_element(tree, 5, 50u32.into()).unwrap();

        // it will trigger a left-left rotation
        forest.remove_element(tree, 30);
        let height = forest.height(tree);
        assert_eq!(height, 2);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 100);
        assert_eq!(forest.augmented(root).0, 350);
    }

    #[test]
    fn can_remove_from_five_no_children_rl_left_child() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();
        let _ = forest.insert_element(tree, 25, 250u32.into()).unwrap();

        // it will trigger a right-right rotation
        forest.remove_element(tree, 10);
        let height = forest.height(tree);
        assert_eq!(height, 2);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 250);
        assert_eq!(forest.augmented(root).0, 750);
    }

    #[test]
    fn can_remove_from_five_no_children_lr_right_child() {
        let arena: NodeArray<AvlNode<i32, i32, u32, TestU32>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(13).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();
        let _ = forest.insert_element(tree, 15, 150u32.into()).unwrap();

        // it will trigger a left-left rotation
        forest.remove_element(tree, 30);
        let height = forest.height(tree);
        assert_eq!(height, 2);

        let root = forest.root(tree);
        assert_eq!(forest.value(root), 150);
        assert_eq!(forest.augmented(root).0, 450);
    }

    #[test]
    fn can_add_eight_nodes_and_remove_one() {
        let arena: NodeArray<AvlNode<i32, i16, u32, TestU32>, 200> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let items: [i16; 8] = [1423, 2813, 1677, 2656, 3781, 4401, 4323, 2503];
        let tree = forest.insert_tree(13).unwrap();

        for (i, item) in items.iter().enumerate() {
            let _ = forest.insert_element(tree, *item, i as u32).unwrap();
        }

        assert_eq!(forest.height(tree), 4);
        forest.remove_element(tree, 1423);
        assert_eq!(forest.height(tree), 3);
    }

    #[test]
    fn can_remove_most_of_nodes() {
        let number_of_nodes = 127;
        let number_of_trials = 1000;
        let number_of_removals = 90;

        let mut rng = rand::rng();
        let expected_height = avl_max_height(number_of_nodes - number_of_removals);

        for _ in 0..number_of_trials {
            let arena: NodeArray<AvlNode<i32, i16, u32, TestU32>, 2000000> = NodeArray::new();
            let mut forest = AvlForest::debug(arena);

            let tree = forest.insert_tree(13).unwrap();
            let mut numbers: Vec<i16> = (0..number_of_trials).collect();

            numbers.shuffle(&mut rng);
            for i in 0..number_of_nodes {
                let _ = forest.insert_element(tree, numbers[i as usize], i).unwrap();
            }

            for i in 0..number_of_removals {
                forest.print(tree);
                let _ = forest.remove_element(tree, numbers[i as usize]);
            }

            if forest.height(tree) > expected_height {
                forest.print(tree);
                assert_eq!(forest.height(tree), expected_height);
                assert!(false);
            }
        }
    }

    #[test]
    fn can_search_fully_matched_tree() {
        let arena: NodeArray<AvlNode<(), u32, u32, MinMaxCount>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(()).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();

        let mut search = RangeSearch::new();
        forest.search(tree, &mut search, 10, 30);

        assert_eq!(search.sum(), 600);
        assert_eq!(search.count(), 3);
    }

    #[test]
    fn can_search_partial_left_matched_tree() {
        let arena: NodeArray<AvlNode<(), u32, u32, MinMaxCount>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(()).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();

        let mut search = RangeSearch::new();
        forest.search(tree, &mut search, 10, 20);

        assert_eq!(search.sum(), 300);
        assert_eq!(search.count(), 2);
    }

    #[test]
    fn can_search_partial_right_matched_tree() {
        let arena: NodeArray<AvlNode<(), u32, u32, MinMaxCount>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(()).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();

        let mut search = RangeSearch::new();
        forest.search(tree, &mut search, 20, 30);

        assert_eq!(search.sum(), 500);
        assert_eq!(search.count(), 2);
    }

    #[test]
    fn can_search_partial_node_matched_tree() {
        let arena: NodeArray<AvlNode<(), u32, u32, MinMaxCount>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(()).unwrap();
        let _ = forest.insert_element(tree, 20, 200u32.into()).unwrap();
        let _ = forest.insert_element(tree, 30, 300u32.into()).unwrap();
        let _ = forest.insert_element(tree, 10, 100u32.into()).unwrap();

        let mut search = RangeSearch::new();
        forest.search(tree, &mut search, 19, 21);

        assert_eq!(search.sum(), 200);
        assert_eq!(search.count(), 1);
    }
}
