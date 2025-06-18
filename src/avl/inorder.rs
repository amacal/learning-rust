use crate::arena::NodeArena;
use crate::avl::{AvlForest, AvlLike, AvlLogger, AvlNode};

struct InorderIterator<'a, T: Copy, K: Copy, V: Copy, G: Copy, N: Copy, A, L>
where
    N: AvlLike<T, K, V, G>,
    A: NodeArena<N>,
    L: AvlLogger<K, V>,
{
    forest: &'a AvlForest<T, K, V, G, N, A, L>,
    idx: u32,
}

impl<'a, T: Copy, K: Copy, V: Copy, G: Copy, N: Copy, A, L> InorderIterator<'a, T, K, V, G, N, A, L>
where
    N: AvlLike<T, K, V, G>,
    A: NodeArena<N>,
    L: AvlLogger<K, V>,
{
    fn new(forest: &'a AvlForest<T, K, V, G, N, A, L>, idx: u32) -> Self {
        InorderIterator { forest, idx }
    }
}

impl<'a, T: Copy, K: Copy, V: Copy, G: Copy, N: Copy, A, L> Iterator for InorderIterator<'a, T, K, V, G, N, A, L>
where
    N: AvlLike<T, K, V, G>,
    A: NodeArena<N>,
    L: AvlLogger<K, V>,
{
    type Item = AvlNode<T, K, V, G>;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

impl<T: Copy, K: Copy, V: Copy, N: Copy, G: Copy, A, L> AvlForest<T, K, V, G, N, A, L>
where
    N: AvlLike<T, K, V, G>,
    A: NodeArena<N>,
    L: AvlLogger<K, V>,
{
    /// Returns an iterator over the elements in the AVL tree in sorted order.
    pub fn inorder(&self, idx: u32) -> impl Iterator<Item = AvlNode<T, K, V, G>> + '_ {
        InorderIterator::new(self, idx)
    }
}
