use crate::arena::NodeArena;
use crate::avl::{AvlAugment, AvlForest, AvlHeight, AvlLike, AvlLogger};

impl<T: Copy, K: Copy, V: Copy, N: Copy, G: Copy, A, L> AvlForest<T, K, V, G, N, A, L>
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

    pub fn split(&mut self, idx: u32, key: K) -> (u32, u32, u32)
    where
        K: PartialOrd,
    {
        // recursion base case
        if idx == 0 {
            return (0, 0, 0);
        }

        // get the node at the current index
        let node = unsafe { self.get_ref(idx) };
        let pkey = node.get_key();

        if key < pkey {
            unsafe {
                // split recursively into the left subtree
                let left = node.get_left();
                let (lt, mid, gt) = self.split(left, key);

                // update affected node
                self.get_mut(idx).set_left(gt);
                self.update_augmented(idx);

                return (lt, mid, idx);
            }
        }

        if key > pkey {
            unsafe {
                // split recursively into the right subtree
                let right = node.get_right();
                let (lt, mid, gt) = self.split(right, key);

                // update affected node
                self.get_mut(idx).set_right(lt);
                self.update_augmented(idx);

                return (idx, mid, gt);
            }
        }

        unsafe {
            // extract children
            let left = node.get_left();
            let right = node.get_right();

            // clear the current node
            self.get_mut(idx).set_left(0);
            self.get_mut(idx).set_right(0);

            // aggregate node and return it
            self.update_augmented(idx);
            return (left, idx, right);
        }
    }
}

#[cfg(test)]
mod tests {
    use core::num;

    use rand::seq::SliceRandom;

    use crate::arena::NodeArray;
    use crate::avl::{AvlAugment, AvlForest, AvlHeight, AvlNode};

    #[derive(Copy, Clone, Debug)]
    struct AugmentInTest(u8);

    impl<T> AvlAugment<T, u32, AugmentInTest> for AugmentInTest {
        fn augment(_key: &T, _value: &u32, left: Option<&AugmentInTest>, right: Option<&AugmentInTest>) -> AugmentInTest {
            let left = left.map_or(0, |l| l.0);
            let right = right.map_or(0, |r| r.0);

            return AugmentInTest(left + right + 1);
        }
    }

    impl AvlHeight for AugmentInTest {
        fn height(&self) -> u8 {
            self.0
        }
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
    fn can_split_avl_hundreds_nodes() {
        let arena: NodeArray<AvlNode<(), i32, u32, AugmentInTest>, 1000> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let number_of_trials = 100;
        let tree = forest.insert_tree(()).unwrap();

        let mut rng = rand::rng();
        let mut numbers: Vec<i32> = (0..number_of_trials).collect();

        numbers.shuffle(&mut rng);
        for i in 0..100 {
            forest.insert_element(tree, numbers[i], i as u32).unwrap();
        }

        let root = forest.root(tree);
        let (lt, mid, rt) = forest.split(root, 17);

    }
}
