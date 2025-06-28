use crate::arena::NodeArena;
use crate::avl::inline::InlineStack;
use crate::avl::{AvlForest, AvlLike, AvlLogger};

struct InorderIterator<'a, T: Copy, K: Copy, V: Copy, G: Copy, N: Copy, A, L>
where
    N: AvlLike<T, K, V, G>,
    A: NodeArena<N>,
    L: AvlLogger<K, V>,
{
    forest: &'a AvlForest<T, K, V, G, N, A, L>,
    path: InlineStack<u32, 64>,
    idx: u32,
}

impl<'a, T: Copy, K: Copy, V: Copy, G: Copy, N: Copy, A, L> InorderIterator<'a, T, K, V, G, N, A, L>
where
    N: AvlLike<T, K, V, G>,
    A: NodeArena<N>,
    L: AvlLogger<K, V>,
{
    // creates a new inorder iterator for a given subtree
    fn new(forest: &'a AvlForest<T, K, V, G, N, A, L>, idx: u32) -> Self {
        let mut path = InlineStack::new();

        Self::leftmost(forest, &mut path, idx);
        InorderIterator { forest, path, idx: 0 }
    }

    // records the leftmost path from the given index to the leftmost leaf
    fn leftmost(forest: &AvlForest<T, K, V, G, N, A, L>, path: &mut InlineStack<u32, 64>, idx: u32) {
        let mut idx = idx;

        while idx > 0 {
            path.push(idx);

            match unsafe { forest.get_ref(idx).get_left() } {
                0 => return,
                left => idx = left,
            }
        }
    }

    // extracts the key, value, and augmented data from the node at the given index
    #[inline(always)]
    fn node(&self, idx: u32) -> (K, V, G) {
        let node = unsafe { self.forest.get_node(idx) };
        (node.0.get_key(), node.0.get_value(), node.0.get_augmented())
    }
}

impl<'a, T: Copy, K: Copy, V: Copy, G: Copy, N: Copy, A, L> Iterator for InorderIterator<'a, T, K, V, G, N, A, L>
where
    N: AvlLike<T, K, V, G>,
    A: NodeArena<N>,
    L: AvlLogger<K, V>,
{
    type Item = (K, V, G);

    fn next(&mut self) -> Option<Self::Item> {
        // no more nodes to traverse
        if self.path.empty() {
            return None;
        }

        // get the current and right node
        let idx = self.path.pop();
        let right = unsafe { self.forest.get_ref(idx).get_right() };

        // expand the right subtree
        Self::leftmost(self.forest, &mut self.path, right);

        // return the current node
        return Some(self.node(idx));
    }
}

impl<T: Copy, K: Copy, V: Copy, N: Copy, G: Copy, A, L> AvlForest<T, K, V, G, N, A, L>
where
    N: AvlLike<T, K, V, G>,
    A: NodeArena<N>,
    L: AvlLogger<K, V>,
{
    /// Returns an iterator over the elements in the AVL tree in sorted order.
    pub fn inorder<'a>(&'a self, node: u32) -> impl Iterator<Item = (K, V, G)> + 'a {
        InorderIterator::new(self, node)
    }
}

#[cfg(test)]
mod tests {
    use crate::{AvlForest, AvlNode, NodeArray};

    #[test]
    fn can_traverse_empty_tree() {
        let arena: NodeArray<AvlNode<(), i32, u32, ()>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(()).unwrap();
        let root = forest.root(tree);

        let mut iter = forest.inorder(root);
        assert!(iter.next().is_none());
    }

    #[test]
    fn can_traverse_simple_tree() {
        let arena: NodeArray<AvlNode<(), i32, u32, ()>, 10> = NodeArray::new();
        let mut forest = AvlForest::new(arena);

        let tree = forest.insert_tree(()).unwrap();
        let _ = forest.insert_element(tree, 20, 200).unwrap();
        let _ = forest.insert_element(tree, 30, 300).unwrap();
        let _ = forest.insert_element(tree, 10, 100).unwrap();

        let root = forest.root(tree);
        let mut iter = forest.inorder(root);

        assert_eq!(iter.next().unwrap(), (10, 100, ()));
        assert_eq!(iter.next().unwrap(), (20, 200, ()));
        assert_eq!(iter.next().unwrap(), (30, 300, ()));
        assert!(iter.next().is_none());
    }
}
