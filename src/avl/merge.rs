use std::fmt::Debug;

use crate::arena::NodeArena;
use crate::avl::{AvlAugment, AvlForest, AvlHeight, AvlLike, AvlLogger, Balance, Node, SHRANK_BIT};

impl<T: Copy, K: Copy + Debug, V: Copy, N: Copy, G: Copy, A, L> AvlForest<T, K, V, G, N, A, L>
where
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
    // left elements must be less than right elements
    fn join_recursive(&mut self, left: u32, right: u32) -> u32
    where
        K: PartialOrd,
    {
        let mut left = left;

        unsafe {
            // we need to find the rightmost node in the left subtree
            let mut rightmost = left;
            while self.get_ref(rightmost).get_right() > 0 {
                rightmost = self.get_ref(rightmost).get_right();
            }

            // we can replace the current node with the rightmost node
            let key = self.get_ref(rightmost).get_key();
            let value = self.get_ref(rightmost).get_value();

            // remove the rightmost node from the left subtree
            left = self.remove_recursive(left, key);

            // allocate a new node in the arena, in the just released slot
            let idx = self.arena.insert_unchecked(N::from_node(Node::node(key, value)));

            // set the left and right children of the new node
            self.get_mut(idx).set_left(left);
            self.get_mut(idx).set_right(right);

            // rebalance the new node
            return self.rebalance_recursive(idx);
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
    // it doesn't preserve the AVL tree properties
    fn split(&mut self, idx: u32, key: K) -> (u32, u32, u32)
    where
        K: PartialOrd,
    {
        // recursion base case
        if idx == 0 {
            return (0, 0, 0);
        }

        // get the node at the current index
        let node = unsafe { self.get_ref(idx) };
        let (mut idx, pkey) = (idx, node.get_key());
        println!("Splitting at idx = {}, key = {:?}, pkey = {:?}", idx, key, pkey);

        if key < pkey {
            unsafe {
                // split recursively into the left subtree
                let left = node.get_left();
                let (lt, mid, rt) = self.split(left, key);
                println!("Left Split: key={:?}, left = {}, mid = {}, right = {}", pkey, lt, mid, rt);

                // update affected node
                self.get_mut(idx).set_left(rt);
                self.update_augmented(idx);

                let left = self.get_ref(idx).get_left();
                let left: i16 = if left == 0 { 0 } else { self.get_ref(left).get_augmented().height().into() };

                let right = self.get_ref(idx).get_right();
                let right: i16 = if right == 0 { 0 } else { self.get_ref(right).get_augmented().height().into() };

                match right - left {
                    0 => {
                        L::on_rebalance((idx, pkey), Balance::Equal);
                        self.get_mut(idx).set_balance(Balance::Equal);
                    }
                    1 => {
                        L::on_rebalance((idx, pkey), Balance::RightHeavy);
                        self.get_mut(idx).set_balance(Balance::RightHeavy);
                    }
                    -1 => {
                        L::on_rebalance((idx, pkey), Balance::LeftHeavy);
                        self.get_mut(idx).set_balance(Balance::LeftHeavy);
                    }
                    2 => {
                        let right = self.get_ref(idx).get_right();
                        let balance = self.get_ref(right).get_balance();

                        match balance {
                            Balance::LeftHeavy => {
                                L::on_rotate((idx, pkey));
                                idx = self.rotate_ll(idx, true);
                            }
                            Balance::RightHeavy => {
                                L::on_rotate((idx, pkey));
                                idx = self.rotate_lr(idx);
                            }
                            Balance::Equal => {
                                // won't happen, because grow cannot lead to equal balance
                                println!("wont happen");
                            }
                        }
                    }
                    _ => println!("Invalid balance state"),
                }

                println!("Left split: key={:?}, left height = {}, right height = {}", pkey, left, right);
                return (lt, mid, idx);
            }
        }

        if key > pkey {
            unsafe {
                // split recursively into the right subtree
                let right = node.get_right();
                let (lt, mid, rt) = self.split(right, key);
                println!("Right Split: key={:?}, left = {}, mid = {}, right = {}", pkey, lt, mid, rt);

                // update affected node
                self.get_mut(idx).set_right(lt);
                self.update_augmented(idx);

                let left = self.get_ref(idx).get_left();
                let left: i16 = if left == 0 { 0 } else { self.get_ref(left).get_augmented().height().into() };

                let right = self.get_ref(idx).get_right();
                let right: i16 = if right == 0 { 0 } else { self.get_ref(right).get_augmented().height().into() };

                match right - left {
                    0 => {
                        L::on_rebalance((idx, pkey), Balance::Equal);
                        self.get_mut(idx).set_balance(Balance::Equal);
                    }
                    1 => {
                        L::on_rebalance((idx, pkey), Balance::RightHeavy);
                        self.get_mut(idx).set_balance(Balance::RightHeavy);
                    }
                    -1 => {
                        L::on_rebalance((idx, pkey), Balance::LeftHeavy);
                        self.get_mut(idx).set_balance(Balance::LeftHeavy);
                    }
                    -2 => {
                        idx = self.remove_rebalance_right(idx, pkey) & !SHRANK_BIT;
                    }
                    _ => println!("Invalid balance state"),
                }

                println!("Right split: key={:?}, left height = {}, right height = {}", pkey, left, right);
                return (idx, mid, rt);
            }
        }

        unsafe {
            // extract children
            let left = node.get_left();
            let right = node.get_right();

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
    use crate::avl::{AvlAugment, AvlForest, AvlHeight, AvlNode};

    #[derive(Copy, Clone, Debug, PartialEq)]
    struct AugmentInTest(u8);

    impl<T> AvlAugment<T, u32, AugmentInTest> for AugmentInTest {
        fn augment(_key: &T, _value: &u32, left: Option<&AugmentInTest>, right: Option<&AugmentInTest>) -> AugmentInTest {
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

    #[test]
    fn can_join_two_nodes() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        let n10 = forest.insert_element(t1, 10, 100).unwrap();

        let t2 = forest.insert_tree(()).unwrap();
        let n20 = forest.insert_element(t2, 20, 200).unwrap();

        let joined = forest.join_recursive(n10, n20);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, vec![10, 20]);
    }

    #[test]
    fn can_join_three_nodes() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        let n10 = forest.insert_element(t1, 10, 100).unwrap();
        let _n15 = forest.insert_element(t1, 15, 150).unwrap();

        let t2 = forest.insert_tree(()).unwrap();
        let n20 = forest.insert_element(t2, 20, 200).unwrap();

        let joined = forest.join_recursive(n10, n20);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, vec![10, 15, 20]);
    }

    #[test]
    fn can_join_five_nodes() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        let _n10 = forest.insert_element(t1, 10, 100).unwrap();
        let _n15 = forest.insert_element(t1, 15, 150).unwrap();
        let _n13 = forest.insert_element(t1, 13, 130).unwrap();
        let _n17 = forest.insert_element(t1, 17, 170).unwrap();

        let t2 = forest.insert_tree(()).unwrap();
        let _n20 = forest.insert_element(t2, 20, 200).unwrap();

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, vec![10, 13, 15, 17, 20]);
    }

    #[test]
    fn can_join_seven_nodes() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        let _n10 = forest.insert_element(t1, 10, 100).unwrap();
        let _n15 = forest.insert_element(t1, 15, 150).unwrap();
        let _n13 = forest.insert_element(t1, 13, 130).unwrap();
        let _n17 = forest.insert_element(t1, 17, 170).unwrap();
        let _n16 = forest.insert_element(t1, 16, 160).unwrap();
        let _n18 = forest.insert_element(t1, 18, 180).unwrap();

        let t2 = forest.insert_tree(()).unwrap();
        let _n20 = forest.insert_element(t2, 20, 200).unwrap();

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, vec![10, 13, 15, 16, 17, 18, 20]);
    }

    #[test]
    fn can_join_1003_nodes_left_heavy() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 10000> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        for i in 0..1000 {
            forest.insert_element(t1, i, i as u32 * 10).unwrap();
        }

        let t2 = forest.insert_tree(()).unwrap();
        for i in 1000..1003 {
            forest.insert_element(t2, i, i as u32 * 10).unwrap();
        }

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, (0..1003).collect::<Vec<i32>>());
    }

    #[test]
    fn can_join_1003_nodes_right_heavy() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 10000> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        for i in 0..3 {
            forest.insert_element(t1, i, i as u32 * 10).unwrap();
        }

        let t2 = forest.insert_tree(()).unwrap();
        for i in 3..1003 {
            forest.insert_element(t2, i, i as u32 * 10).unwrap();
        }

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, (0..1003).collect::<Vec<i32>>());
    }

    #[test]
    fn can_join_10003_nodes_left_heavy() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 20000> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        for i in 0..10000 {
            forest.insert_element(t1, i, i as u32 * 10).unwrap();
        }

        let t2 = forest.insert_tree(()).unwrap();
        for i in 10000..10003 {
            forest.insert_element(t2, i, i as u32 * 10).unwrap();
        }

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, (0..10003).collect::<Vec<i32>>());
    }

    #[test]
    fn can_join_10003_nodes_right_heavy() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 20000> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        for i in 0..3 {
            forest.insert_element(t1, i, i as u32 * 10).unwrap();
        }

        let t2 = forest.insert_tree(()).unwrap();
        for i in 3..10003 {
            forest.insert_element(t2, i, i as u32 * 10).unwrap();
        }

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, (0..10003).collect::<Vec<i32>>());
    }

    #[test]
    fn can_join_1010_nodes_left_heavy() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 10000> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        for i in 0..1000 {
            forest.insert_element(t1, i, i as u32 * 10).unwrap();
        }

        let t2 = forest.insert_tree(()).unwrap();
        for i in 1000..1010 {
            forest.insert_element(t2, i, i as u32 * 10).unwrap();
        }

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, (0..1010).collect::<Vec<i32>>());
    }

    #[test]
    fn can_join_1010_nodes_right_heavy() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 10000> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let t1 = forest.insert_tree(()).unwrap();
        for i in 0..3 {
            forest.insert_element(t1, i, i as u32 * 10).unwrap();
        }

        let t2 = forest.insert_tree(()).unwrap();
        for i in 3..1010 {
            forest.insert_element(t2, i, i as u32 * 10).unwrap();
        }

        let r1 = forest.root(t1);
        let r2 = forest.root(t2);

        let joined = forest.join_recursive(r1, r2);
        let iter = forest.inorder(joined);

        let items: Vec<_> = iter.collect();
        let keys = items.iter().map(|(k, _, _)| *k).collect::<Vec<_>>();

        assert_eq!(keys, (0..1010).collect::<Vec<i32>>());
    }

    #[test]
    fn can_split_avl_three_nodes_in_the_middle() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(()).unwrap();
        let n20 = forest.insert_element(tree, 20, 200).unwrap();
        let n30 = forest.insert_element(tree, 30, 300).unwrap();
        let n10 = forest.insert_element(tree, 10, 100).unwrap();

        let (lt, mid, rt) = forest.split(n20, 20);
        assert_eq!((lt, mid, rt), (n10, n20, n30));
    }

    #[test]
    fn can_split_avl_three_nodes_in_the_left() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(()).unwrap();
        let n20 = forest.insert_element(tree, 20, 200).unwrap();
        let _n30 = forest.insert_element(tree, 30, 300).unwrap();
        let n10 = forest.insert_element(tree, 10, 100).unwrap();

        let (lt, mid, rt) = forest.split(n20, 15);
        assert_eq!((lt, mid, rt), (n10, 0, n20));
    }

    #[test]
    fn can_split_avl_three_nodes_in_the_left_outside() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(()).unwrap();
        let n20 = forest.insert_element(tree, 20, 200).unwrap();
        let _n30 = forest.insert_element(tree, 30, 300).unwrap();
        let _n10 = forest.insert_element(tree, 10, 100).unwrap();

        let (lt, mid, rt) = forest.split(n20, 0);
        assert_eq!((lt, mid, rt), (0, 0, n20));
    }

    #[test]
    fn can_split_avl_three_nodes_in_the_right_exact() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(()).unwrap();
        let n20 = forest.insert_element(tree, 20, 200).unwrap();
        let n30 = forest.insert_element(tree, 30, 300).unwrap();
        let _n10 = forest.insert_element(tree, 10, 100).unwrap();

        let (lt, mid, rt) = forest.split(n20, 30);
        assert_eq!((lt, mid, rt), (n20, n30, 0));
    }

    #[test]
    fn can_split_avl_five_nodes_in_the_left() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(()).unwrap();
        let n20 = forest.insert_element(tree, 20, 200).unwrap();
        let _n30 = forest.insert_element(tree, 30, 300).unwrap();
        let n10 = forest.insert_element(tree, 10, 100).unwrap();
        let _n15 = forest.insert_element(tree, 15, 150).unwrap();
        let _n1 = forest.insert_element(tree, 5, 50).unwrap();

        let root = forest.root(tree);
        let (lt, mid, rt) = forest.split(root, 12);

        assert_eq!((lt, mid, rt), (n10, 0, n20));
    }

    // #[test]
    fn can_split_avl_five_nodes_in_the_right() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 10> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let tree = forest.insert_tree(()).unwrap();
        let n20 = forest.insert_element(tree, 20, 200).unwrap();
        let n30 = forest.insert_element(tree, 30, 300).unwrap();
        let _n10 = forest.insert_element(tree, 10, 100).unwrap();
        let _n15 = forest.insert_element(tree, 15, 150).unwrap();
        let _n1 = forest.insert_element(tree, 5, 50).unwrap();

        forest.print(tree);
        let root = forest.root(tree);
        let (lt, mid, rt) = forest.split(root, 25);

        forest.print_node(lt);
        assert_eq!((lt, mid, rt), (n20, 0, n30));
        assert_eq!(unsafe { forest.get_ref(n20).get_augmented().height() }, 3);
        assert!(false);
    }

    // #[test]
    fn can_split_avl_hundreds_nodes() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 2000> = NodeArray::new();
        let mut forest = AvlForest::debug(arena);

        let number_of_trials = 10;
        let tree = forest.insert_tree(()).unwrap();

        let mut rng = rand::rng();
        let mut numbers: Vec<i32> = (0..number_of_trials).collect();

        numbers.shuffle(&mut rng);
        for (idx, key) in numbers.iter().enumerate() {
            forest.insert_element(tree, *key, idx as u32).unwrap();
        }

        let (pivot, root, mut counter) = (7, forest.root(tree), 0);
        let (lt, mid, rt) = forest.split(root, pivot);

        for (key, _, _) in forest.inorder(lt) {
            counter += 1;
            assert!(key < pivot);
        }

        for (key, _, _) in forest.inorder(mid) {
            counter += 1;
            assert!(key == pivot);
        }

        for (key, _, _) in forest.inorder(rt) {
            counter += 1;
            assert!(key > pivot);
        }

        assert_eq!(counter, number_of_trials);
        assert!(false);
    }
}
