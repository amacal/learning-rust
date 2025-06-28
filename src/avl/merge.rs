use std::fmt::Debug;

use crate::arena::NodeArena;
use crate::avl::{AvlAugment, AvlForest, AvlLike, AvlLogger, Balance, GREW_BIT};

pub trait AvlHeight {
    // provides the height of the tree
    fn height(&self) -> u8;
}

pub trait AvlMerge<K: Copy, V: Copy> {
    // merges two values on key conflict
    fn merge(key: &K, left: &V, right: &V) -> V;
}

impl<T: Copy, K: Copy + Debug, V: Copy, N: Copy, G: Copy, A, L> AvlForest<T, K, V, G, N, A, L>
where
    V: AvlMerge<K, V>,
    G: AvlAugment<K, V, G> + AvlHeight,
    N: AvlLike<T, K, V, G>,
    A: NodeArena<N>,
    L: AvlLogger<K, V>,
{
    // merge two trees by their roots, but it's only possible
    // when height of trees are available via augmentation
    pub fn merge(&mut self, left: u32, right: u32) -> u32 {
        0
    }

    // joins two disjoint nodes and returns a node
    // left elements < mid element < right elements
    fn join_recursive(&mut self, left: u32, mid: u32, right: u32) -> u32
    where
        K: PartialOrd,
    {
        unsafe {
            // base left case
            if left == 0 {
                self.get_mut(mid).clear();

                let key = self.get_ref(mid).get_key();
                return self.insert_recursive(right, mid, key) & !GREW_BIT;
            }

            // base right case
            if right == 0 {
                self.get_mut(mid).clear();

                let key = self.get_ref(mid).get_key();
                return self.insert_recursive(left, mid, key) & !GREW_BIT;
            }

            // set the left and right children of the new node
            self.get_mut(mid).set_left(left);
            self.get_mut(mid).set_right(right);

            // rebalance the new node
            return self.rebalance_recursive(mid);
        }
    }

    fn rebalance_recursive(&mut self, node: u32) -> u32
    where
        K: PartialOrd,
    {
        let mut node = node;

        unsafe {
            let mut left = self.get_ref(node).get_left();
            let mut right = self.get_ref(node).get_right();

            let mut hleft: i32 = if left == 0 { 0 } else { self.get_ref(left).get_augmented().height().into() };
            let mut hright: i32 = if right == 0 { 0 } else { self.get_ref(right).get_augmented().height().into() };

            if hleft > hright + 1 {
                // find best rotation for left-heavy case
                L::on_rotate((node, self.get_ref(node).get_key()));
                node = match self.get_ref(left).get_balance() {
                    Balance::Equal => self.rotate_ll(node, true),
                    Balance::LeftHeavy => self.rotate_ll(node, false),
                    Balance::RightHeavy => self.rotate_lr(node),
                };

                // new left and right children after rotation
                left = self.get_ref(node).get_left();
                right = self.get_ref(node).get_right();

                // recursive call to rebalance again
                right = self.rebalance_recursive(right);
                self.get_mut(node).set_right(right);

                // update heights after rotation
                hleft = self.get_ref(left).get_augmented().height().into();
                hright = self.get_ref(right).get_augmented().height().into();
            }

            if hright > hleft + 1 {
                // find best rotation for right-heavy case
                L::on_rotate((node, self.get_ref(node).get_key()));
                node = match self.get_ref(right).get_balance() {
                    Balance::Equal => self.rotate_rr(node, true),
                    Balance::RightHeavy => self.rotate_rr(node, false),
                    Balance::LeftHeavy => self.rotate_rl(node),
                };

                // new left and right children after rotation
                left = self.get_ref(node).get_left();
                right = self.get_ref(node).get_right();

                // recursive call to rebalance again
                left = self.rebalance_recursive(left);
                self.get_mut(node).set_left(left);

                // update heights after rotation
                hleft = self.get_ref(left).get_augmented().height().into();
                hright = self.get_ref(right).get_augmented().height().into();
            }

            match hright - hleft {
                0 => {
                    L::on_rebalance((node, self.get_ref(node).get_key()), Balance::Equal);
                    self.get_mut(node).set_balance(Balance::Equal);
                }
                1 => {
                    L::on_rebalance((node, self.get_ref(node).get_key()), Balance::RightHeavy);
                    self.get_mut(node).set_balance(Balance::RightHeavy);
                }
                -1 => {
                    L::on_rebalance((node, self.get_ref(node).get_key()), Balance::LeftHeavy);
                    self.get_mut(node).set_balance(Balance::LeftHeavy);
                }
                _ => {}
            }

            self.update_augmented(node);
        }

        node
    }

    // splits the tree at the given index by the key
    fn split_recursive(&mut self, idx: u32, key: K) -> (u32, u32, u32)
    where
        K: PartialOrd,
    {
        // recursion base case
        if idx == 0 {
            return (0, 0, 0);
        }

        // get the node at the current index
        let (left, right, pkey) = {
            let node = unsafe { self.get_ref(idx) };
            (node.get_left(), node.get_right(), node.get_key())
        };

        if pkey > key {
            // split recursively into the left subtree
            println!("split_recursive: idx={}, key={:?}, pkey={:?}", idx, key, pkey);
            let (lt, mid, rt) = self.split_recursive(left, key);

            // join two disjoint right sides
            println!("join_recursive: lt={}, mid={}, rt={}, right={}", lt, mid, rt, right);
            return (lt, mid, self.join_recursive(rt, idx, right));
        }

        if pkey < key {
            // split recursively into the right subtree
            println!("split_recursive: idx={}, key={:?}, pkey={:?}", idx, key, pkey);
            let (lt, mid, rt) = self.split_recursive(right, key);

            // join two disjoint left sides
            println!("join_recursive: left={}, mid={}, lt={}, idx={}", left, mid, lt, idx);
            return (self.join_recursive(left, idx, lt), mid, rt);
        }

        unsafe {
            // clear the current node
            self.get_mut(idx).set_left(0);
            self.get_mut(idx).set_right(0);

            // update the current node
            L::on_rebalance((idx, pkey), Balance::Equal);
            self.get_mut(idx).set_balance(Balance::Equal);

            // aggregate node and return it
            self.update_augmented(idx);
            return (left, idx, right);
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::seq::SliceRandom;

    use crate::arena::NodeArray;
    use crate::avl::merge::{AvlHeight, AvlMerge};
    use crate::avl::{AvlAugment, AvlForest, AvlNode};

    #[derive(Copy, Clone, Debug, PartialEq)]
    struct AugmentInTest(u8);

    #[derive(Copy, Clone, Debug)]
    struct ValueInTest(u32);

    impl<T> AvlAugment<T, ValueInTest, AugmentInTest> for AugmentInTest {
        fn augment(_key: &T, _value: &ValueInTest, left: Option<&AugmentInTest>, right: Option<&AugmentInTest>) -> AugmentInTest {
            let left = left.map_or(0, |l| l.0);
            let right = right.map_or(0, |r| r.0);

            return AugmentInTest(std::cmp::max(left, right) + 1);
        }
    }

    impl AvlHeight for AugmentInTest {
        fn height(&self) -> u8 {
            self.0
        }
    }

    impl AvlMerge<i32, ValueInTest> for ValueInTest {
        fn merge(_key: &i32, left: &ValueInTest, right: &ValueInTest) -> ValueInTest {
            ValueInTest(std::cmp::max(left.0, right.0))
        }
    }

    impl From<u32> for ValueInTest {
        fn from(value: u32) -> Self {
            ValueInTest(value)
        }
    }

    #[test]
    fn can_join_two_nodes() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        let n10 = forest.insert_element(t1, 10, 100.into()).unwrap();

        let t2 = forest.insert_tree(()).unwrap();
        let n20 = forest.insert_element(t2, 20, 200.into()).unwrap();

        let t3 = forest.insert_tree(()).unwrap();
        let n15 = forest.insert_element(t3, 15, 150.into()).unwrap();

        let joined = forest.join_recursive(n10, n15, n20);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, vec![10, 15, 20]);
    }

    #[test]
    fn can_join_three_nodes() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        let n10 = forest.insert_element(t1, 10, 100.into()).unwrap();
        let _n15 = forest.insert_element(t1, 15, 150.into()).unwrap();

        let t2 = forest.insert_tree(()).unwrap();
        let n20 = forest.insert_element(t2, 20, 200.into()).unwrap();

        let t3 = forest.insert_tree(()).unwrap();
        let n17 = forest.insert_element(t3, 17, 170.into()).unwrap();

        let joined = forest.join_recursive(n10, n17, n20);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, vec![10, 15, 17, 20]);
    }

    #[test]
    fn can_join_five_nodes() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 20> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        let _n10 = forest.insert_element(t1, 10, 100.into()).unwrap();
        let _n15 = forest.insert_element(t1, 15, 150.into()).unwrap();
        let _n13 = forest.insert_element(t1, 13, 130.into()).unwrap();
        let _n17 = forest.insert_element(t1, 17, 170.into()).unwrap();

        let t2 = forest.insert_tree(()).unwrap();
        let _n20 = forest.insert_element(t2, 20, 200.into()).unwrap();

        let t3 = forest.insert_tree(()).unwrap();
        let n19 = forest.insert_element(t3, 19, 190.into()).unwrap();

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, n19, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, vec![10, 13, 15, 17, 19, 20]);
    }

    #[test]
    fn can_join_seven_nodes() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 20> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        let _n10 = forest.insert_element(t1, 10, 100.into()).unwrap();
        let _n15 = forest.insert_element(t1, 15, 150.into()).unwrap();
        let _n13 = forest.insert_element(t1, 13, 130.into()).unwrap();
        let _n17 = forest.insert_element(t1, 17, 170.into()).unwrap();
        let _n16 = forest.insert_element(t1, 16, 160.into()).unwrap();
        let _n18 = forest.insert_element(t1, 18, 180.into()).unwrap();

        let t2 = forest.insert_tree(()).unwrap();
        let _n20 = forest.insert_element(t2, 20, 200.into()).unwrap();

        let t3 = forest.insert_tree(()).unwrap();
        let n19 = forest.insert_element(t3, 19, 190.into()).unwrap();

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, n19, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, vec![10, 13, 15, 16, 17, 18, 19, 20]);
    }

    #[test]
    fn can_join_1004_nodes_left_heavy() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 10000> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        for i in 0..1000 {
            forest.insert_element(t1, i, ValueInTest(i as u32 * 10)).unwrap();
        }

        let t2 = forest.insert_tree(()).unwrap();
        for i in 1001..1004 {
            forest.insert_element(t2, i,ValueInTest(i as u32 * 10)).unwrap();
        }

        let t3 = forest.insert_tree(()).unwrap();
        let n1000 = forest.insert_element(t3, 1000, 10000.into()).unwrap();

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, n1000, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, (0..1004).collect::<Vec<i32>>());
    }

    #[test]
    fn can_join_1003_nodes_right_heavy() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 10000> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        for i in 0..3 {
            forest.insert_element(t1, i, ValueInTest(i as u32 * 10)).unwrap();
        }

        let t2 = forest.insert_tree(()).unwrap();
        for i in 4..1004 {
            forest.insert_element(t2, i, ValueInTest(i as u32 * 10)).unwrap();
        }

        let t3 = forest.insert_tree(()).unwrap();
        let n3 = forest.insert_element(t3, 3, 30.into()).unwrap();

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, n3, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, (0..1004).collect::<Vec<i32>>());
    }

    #[test]
    fn can_join_10004_nodes_left_heavy() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 20000> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        for i in 0..10000 {
            forest.insert_element(t1, i, ValueInTest(i as u32 * 10)).unwrap();
        }

        let t2 = forest.insert_tree(()).unwrap();
        for i in 10001..10004 {
            forest.insert_element(t2, i, ValueInTest(i as u32 * 10)).unwrap();
        }

        let t3 = forest.insert_tree(()).unwrap();
        let n10000 = forest.insert_element(t3, 10000, 100000.into()).unwrap();

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, n10000, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, (0..10004).collect::<Vec<i32>>());
    }

    #[test]
    fn can_join_10004_nodes_right_heavy() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 20000> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        for i in 0..3 {
            forest.insert_element(t1, i, ValueInTest(i as u32 * 10)).unwrap();
        }

        let t2 = forest.insert_tree(()).unwrap();
        for i in 4..10004 {
            forest.insert_element(t2, i, ValueInTest(i as u32 * 10)).unwrap();
        }

        let t3 = forest.insert_tree(()).unwrap();
        let n3 = forest.insert_element(t3, 3, 30.into()).unwrap();

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, n3, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, (0..10004).collect::<Vec<i32>>());
    }

    #[test]
    fn can_join_1010_nodes_left_heavy() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 10000> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        for i in 0..1000 {
            forest.insert_element(t1, i, ValueInTest(i as u32 * 10)).unwrap();
        }

        let t2 = forest.insert_tree(()).unwrap();
        for i in 1001..1010 {
            forest.insert_element(t2, i, ValueInTest(i as u32 * 10)).unwrap();
        }

        let t3 = forest.insert_tree(()).unwrap();
        let n1000 = forest.insert_element(t3, 1000, 10000.into()).unwrap();

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, n1000, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, (0..1010).collect::<Vec<i32>>());
    }

    #[test]
    fn can_join_1010_nodes_right_heavy() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 10000> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        for i in 0..3 {
            forest.insert_element(t1, i, ValueInTest(i as u32 * 10)).unwrap();
        }

        let t2 = forest.insert_tree(()).unwrap();
        for i in 4..1010 {
            forest.insert_element(t2, i, ValueInTest(i as u32 * 10)).unwrap();
        }

        let t3 = forest.insert_tree(()).unwrap();
        let n3 = forest.insert_element(t3, 3, 30.into()).unwrap();

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, n3, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, (0..1010).collect::<Vec<i32>>());
    }

    #[test]
    fn can_split_avl_three_nodes_in_the_middle() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(()).unwrap();
        let n20 = forest.insert_element(tree, 20, 200.into()).unwrap();
        let _n30 = forest.insert_element(tree, 30, 300.into()).unwrap();
        let _n10 = forest.insert_element(tree, 10, 100.into()).unwrap();

        let (lt, mid, rt) = forest.split_recursive(n20, 20);

        let litems: Vec<_> = forest.inorder(lt).collect();
        let lkeys = litems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        let mitems: Vec<_> = forest.inorder(mid).collect();
        let mkeys = mitems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        let ritems: Vec<_> = forest.inorder(rt).collect();
        let rkeys = ritems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(lkeys, vec![10]);
        assert_eq!(mkeys, vec![20]);
        assert_eq!(rkeys, vec![30]);
    }

    #[test]
    fn can_split_avl_three_nodes_in_the_left() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(()).unwrap();
        let n20 = forest.insert_element(tree, 20, 200.into()).unwrap();
        let _n30 = forest.insert_element(tree, 30, 300.into()).unwrap();
        let _n10 = forest.insert_element(tree, 10, 100.into()).unwrap();

        let (lt, mid, rt) = forest.split_recursive(n20, 15);

        let litems: Vec<_> = forest.inorder(lt).collect();
        let lkeys = litems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        let mitems: Vec<_> = forest.inorder(mid).collect();
        let mkeys = mitems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        let ritems: Vec<_> = forest.inorder(rt).collect();
        let rkeys = ritems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(lkeys, vec![10]);
        assert_eq!(mkeys, vec![]);
        assert_eq!(rkeys, vec![20, 30]);
    }

    #[test]
    fn can_split_avl_three_nodes_in_the_left_outside() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let tree = forest.insert_tree(()).unwrap();
        let n20 = forest.insert_element(tree, 20, 200.into()).unwrap();
        let _n30 = forest.insert_element(tree, 30, 300.into()).unwrap();
        let _n10 = forest.insert_element(tree, 10, 100.into()).unwrap();

        let (lt, mid, rt) = forest.split_recursive(n20, 0);

        let litems: Vec<_> = forest.inorder(lt).collect();
        let lkeys = litems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        let mitems: Vec<_> = forest.inorder(mid).collect();
        let mkeys = mitems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        let ritems: Vec<_> = forest.inorder(rt).collect();
        let rkeys = ritems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(lkeys, vec![]);
        assert_eq!(mkeys, vec![]);
        assert_eq!(rkeys, vec![10, 20, 30]);
    }

    #[test]
    fn can_split_avl_three_nodes_in_the_right_exact() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let tree = forest.insert_tree(()).unwrap();
        let n20 = forest.insert_element(tree, 20, 200.into()).unwrap();
        let _n30 = forest.insert_element(tree, 30, 300.into()).unwrap();
        let _n10 = forest.insert_element(tree, 10, 100.into()).unwrap();

        let (lt, mid, rt) = forest.split_recursive(n20, 30);

        let litems: Vec<_> = forest.inorder(lt).collect();
        let lkeys = litems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        let mitems: Vec<_> = forest.inorder(mid).collect();
        let mkeys = mitems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        let ritems: Vec<_> = forest.inorder(rt).collect();
        let rkeys = ritems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(lkeys, vec![10, 20]);
        assert_eq!(mkeys, vec![30]);
        assert_eq!(rkeys, vec![]);
    }

    #[test]
    fn can_split_avl_five_nodes_in_the_left() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(()).unwrap();
        let _n20 = forest.insert_element(tree, 20, 200.into()).unwrap();
        let _n30 = forest.insert_element(tree, 30, 300.into()).unwrap();
        let _n10 = forest.insert_element(tree, 10, 100.into()).unwrap();
        let _n15 = forest.insert_element(tree, 15, 150.into()).unwrap();
        let _n1 = forest.insert_element(tree, 5, 50.into()).unwrap();

        let root = forest.root(tree);
        let (lt, mid, rt) = forest.split_recursive(root, 12);

        let litems: Vec<_> = forest.inorder(lt).collect();
        let lkeys = litems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        let mitems: Vec<_> = forest.inorder(mid).collect();
        let mkeys = mitems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        let ritems: Vec<_> = forest.inorder(rt).collect();
        let rkeys = ritems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(lkeys, vec![5, 10]);
        assert_eq!(mkeys, vec![]);
        assert_eq!(rkeys, vec![15, 20, 30]);
    }

    #[test]
    fn can_split_avl_five_nodes_in_the_right() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let tree = forest.insert_tree(()).unwrap();
        let _n20 = forest.insert_element(tree, 20, 200.into()).unwrap();
        let _n30 = forest.insert_element(tree, 30, 300.into()).unwrap();
        let _n10 = forest.insert_element(tree, 10, 100.into()).unwrap();
        let _n15 = forest.insert_element(tree, 15, 150.into()).unwrap();
        let _n1 = forest.insert_element(tree, 5, 50.into()).unwrap();

        let root = forest.root(tree);
        let (lt, mid, rt) = forest.split_recursive(root, 25);

        let litems: Vec<_> = forest.inorder(lt).collect();
        let lkeys = litems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        let mitems: Vec<_> = forest.inorder(mid).collect();
        let mkeys = mitems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        let ritems: Vec<_> = forest.inorder(rt).collect();
        let rkeys = ritems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(lkeys, vec![5, 10, 15, 20]);
        assert_eq!(mkeys, vec![]);
        assert_eq!(rkeys, vec![30]);
    }

    #[test]
    fn can_split_avl_hundreds_nodes() {
        let arena: NodeArray<AvlNode<(), i32, ValueInTest, AugmentInTest>, 2000> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let number_of_elements = 1000;
        let tree = forest.insert_tree(()).unwrap();

        let mut rng = rand::rng();
        let mut numbers: Vec<i32> = (0..number_of_elements).collect();

        numbers.shuffle(&mut rng);
        for (idx, key) in numbers.iter().enumerate() {
            forest.insert_element(tree, *key, ValueInTest(idx as u32)).unwrap();
        }

        let (pivot, root) = (734, forest.root(tree));
        let (lt, mid, rt) = forest.split_recursive(root, pivot);

        let litems: Vec<_> = forest.inorder(lt).collect();
        let lkeys = litems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        let mitems: Vec<_> = forest.inorder(mid).collect();
        let mkeys = mitems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        let ritems: Vec<_> = forest.inorder(rt).collect();
        let rkeys = ritems.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(lkeys, (0..pivot).collect::<Vec<i32>>());
        assert_eq!(mkeys, vec![pivot]);
        assert_eq!(rkeys, (pivot + 1..number_of_elements).collect::<Vec<i32>>());
    }
}
