use std::{
    cell::UnsafeCell,
    mem::{self, MaybeUninit},
};

#[derive(Copy, Clone)]
struct Root {
    next: u32,
}

#[derive(Copy, Clone)]
struct Free {
    next: u32,
}

#[derive(Copy, Clone)]
struct Item<T: Copy> {
    value: T,
}

union Node<T: Copy> {
    root: Root,
    free: Free,
    item: Item<T>,
}

pub trait NodeArena<T: Copy> {
    /// inserts a new value into the arena and returns its index
    fn insert(&mut self, value: T) -> Option<u32>;

    /// retrieves an immutable value from the arena by its index
    unsafe fn get_unchecked(&self, idx: u32) -> &T;

    /// retrieves a mutable value from the arena by its index
    unsafe fn get_unchecked_mut(&mut self, idx: u32) -> &mut T;

    /// releases a value from the arena by its index, allowing it to be reused
    unsafe fn release_unchecked(&mut self, idx: u32);
}

pub struct NodeArray<T: Copy, const U: usize> {
    counter: u32,
    entries: Box<[Node<T>; U]>,
}

impl<T: Copy, const U: usize> NodeArray<T, U> {
    pub fn new() -> Self {
        // allocate an array of nodes with uninitialized memory
        // the first node is a root node, the rest are free nodes
        // we don't want to touch the memory until we actually need it
        // because it will trigger unnecessary allocation in OS
        let mut entries = unsafe {
            let uninitialized = Box::new_uninit().assume_init();
            let initialized = mem::transmute::<Box<[MaybeUninit<Node<T>>; U]>, Box<[Node<T>; U]>>(uninitialized);

            initialized
        };

        entries[0] = Node { root: Root { next: 0 } };
        NodeArray { entries: entries, counter: 0 }
    }

    pub fn capacity(&self) -> usize {
        self.entries.len() - 1
    }

    pub fn allocated(&self) -> usize {
        self.counter as usize
    }
}

impl<T: Copy, const U: usize> NodeArena<T> for NodeArray<T, U> {
    unsafe fn get_unchecked(&self, idx: u32) -> &T {
        return &self.entries.get_unchecked(idx as usize).item.value;
    }

    unsafe fn get_unchecked_mut(&mut self, idx: u32) -> &mut T {
        return &mut self.entries.get_unchecked_mut(idx as usize).item.value;
    }

    fn insert(&mut self, value: T) -> Option<u32> {
        // find the next available index
        let root = unsafe { self.entries.get_unchecked(0).root.next };
        let next = if root > 0 { root } else { self.counter + 1 };

        // check if we have enough capacity
        if next as usize >= self.entries.len() {
            return None;
        }

        // if counter was used, increment it
        if root == 0 {
            self.counter += 1;
        }

        // if root was used, consumed shift head of the linked list
        if root > 0 {
            let link = unsafe { self.entries.get_unchecked(next as usize).free.next };
            unsafe { self.entries.get_unchecked_mut(0).root.next = link };
        }

        // insert the new value
        unsafe {
            self.entries.get_unchecked_mut(next as usize).item.value = value;
        }

        return Some(next);
    }

    unsafe fn release_unchecked(&mut self, idx: u32) {
        // find the head of the linked list (if available)
        let next = unsafe { self.entries.get_unchecked_mut(0).root.next };

        // change the head of the list
        unsafe { self.entries.get_unchecked_mut(0).root.next = idx };

        // point at the previous head
        unsafe { self.entries.get_unchecked_mut(idx as usize).free.next = next };
    }
}

impl<'a, T: Copy, A: NodeArena<T>> NodeArena<T> for &UnsafeCell<A> {
    fn insert(&mut self, value: T) -> Option<u32> {
        unsafe { (&mut *self.get()).insert(value) }
    }

    unsafe fn get_unchecked(&self, idx: u32) -> &T {
        unsafe { (&*self.get()).get_unchecked(idx) }
    }

    unsafe fn get_unchecked_mut(&mut self, idx: u32) -> &mut T {
        unsafe { (&mut *self.get()).get_unchecked_mut(idx) }
    }

    unsafe fn release_unchecked(&mut self, idx: u32) {
        unsafe { (&mut *self.get()).release_unchecked(idx) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Copy, Clone)]
    struct TestNode {
        value: i32,
    }

    #[test]
    fn can_create_arena() {
        let arena: NodeArray<i32, 10> = NodeArray::new();

        assert_eq!(arena.capacity(), 9);
        assert_eq!(arena.allocated(), 0);
    }

    #[test]
    fn can_insert_new_node_in_not_allocated_area() {
        let mut arena: NodeArray<i32, 10> = NodeArray::new();
        let node = TestNode { value: 42 };

        let idx = arena.insert(node.value);
        assert_eq!(idx, Some(1));

        let fetched = unsafe { arena.get_unchecked(idx.unwrap()) };
        assert_eq!(*fetched, node.value);

        assert_eq!(arena.capacity(), 9);
        assert_eq!(arena.allocated(), 1);
    }

    #[test]
    fn can_insert_and_release_new_nodes_using_arena_ref() {
        let arena: NodeArray<i32, 10> = NodeArray::new();
        let arena = UnsafeCell::new(arena);

        let node1 = TestNode { value: 42 };
        let node2 = TestNode { value: 43 };

        let mut ref1 = &arena;
        let mut ref2 = &arena;

        let idx1 = ref1.insert(node1.value);
        assert_eq!(idx1, Some(1));

        let idx2 = ref2.insert(node2.value);
        assert_eq!(idx2, Some(2));

        let fetched1 = unsafe { ref1.get_unchecked(idx1.unwrap()) };
        assert_eq!(*fetched1, node1.value);

        let fetched2 = unsafe { ref2.get_unchecked(idx2.unwrap()) };
        assert_eq!(*fetched2, node2.value);

        unsafe {
            assert_eq!((&*arena.get()).capacity(), 9);
            assert_eq!((&*arena.get()).allocated(), 2);
        }

        unsafe { ref1.release_unchecked(idx1.unwrap()) };
        unsafe { ref2.release_unchecked(idx2.unwrap()) };

        unsafe {
            assert_eq!((&*arena.get()).capacity(), 9);
            assert_eq!((&*arena.get()).allocated(), 2);
        }
    }

    #[test]
    fn cannot_insert_too_many_nodes() {
        let mut arena: NodeArray<i32, 10> = NodeArray::new();

        // following inserts should succeed
        for i in 0..arena.capacity() {
            let node = TestNode { value: i as i32 };
            let idx = arena.insert(node.value);

            assert!(idx.is_some());
        }

        assert_eq!(arena.capacity(), 9);
        assert_eq!(arena.allocated(), 9);

        // the next one will fail
        let node = TestNode { value: 42 };
        let idx = arena.insert(node.value);

        assert_eq!(idx, None);
        assert_eq!(arena.capacity(), 9);
        assert_eq!(arena.allocated(), 9);
    }

    #[test]
    fn cannot_insert_too_many_nodes_after_few_releases() {
        let mut arena: NodeArray<i32, 10> = NodeArray::new();

        // following inserts should succeed
        for i in 0..arena.capacity() {
            let node = TestNode { value: i as i32 };
            let idx = arena.insert(node.value);

            assert!(idx.is_some());
        }

        assert_eq!(arena.capacity(), 9);
        assert_eq!(arena.allocated(), 9);

        // release every second node
        for i in 0..arena.capacity() {
            if i % 2 == 0 {
                unsafe { arena.release_unchecked(i as u32 + 1) };
            }
        }

        assert_eq!(arena.capacity(), 9);
        assert_eq!(arena.allocated(), 9);

        // following inserts should also succeed
        // because we released every second node
        for i in 0..arena.capacity() {
            if i % 2 == 0 {
                let node = TestNode { value: i as i32 };
                let idx = arena.insert(node.value);

                assert!(idx.is_some());
            }
        }

        assert_eq!(arena.capacity(), 9);
        assert_eq!(arena.allocated(), 9);

        // the next one will fail
        let node = TestNode { value: 42 };
        let idx = arena.insert(node.value);

        assert_eq!(idx, None);
        assert_eq!(arena.capacity(), 9);
        assert_eq!(arena.allocated(), 9);
    }

    #[test]
    fn can_release_existing_node() {
        let mut arena: NodeArray<i32, 10> = NodeArray::new();
        let node = TestNode { value: 42 };

        let idx = arena.insert(node.value).unwrap();
        unsafe { arena.release_unchecked(idx) };

        // allocated also count released nodes
        assert_eq!(arena.capacity(), 9);
        assert_eq!(arena.allocated(), 1);
    }

    #[test]
    fn can_insert_new_node_in_already_released_area() {
        let mut arena: NodeArray<i32, 10> = NodeArray::new();
        let node1 = TestNode { value: 42 };
        let node2 = TestNode { value: 43 };

        let idx1 = arena.insert(node1.value).unwrap();
        let _ = arena.insert(node2.value).unwrap();

        // 2 nodes are active in 2 allocations
        assert_eq!(arena.capacity(), 9);
        assert_eq!(arena.allocated(), 2);

        unsafe { arena.release_unchecked(idx1) };

        // 1 node is active in 2 allocations
        assert_eq!(arena.capacity(), 9);
        assert_eq!(arena.allocated(), 2);
        let node3 = TestNode { value: 44 };

        let idx3 = arena.insert(node3.value);

        assert_eq!(idx3, Some(idx1));

        let fetched3 = unsafe { arena.get_unchecked(idx3.unwrap()) };
        assert_eq!(*fetched3, node3.value);

        // after reusing released node, we still have the same allocation
        assert_eq!(arena.capacity(), 9);
        assert_eq!(arena.allocated(), 2);
    }

    #[test]
    fn can_insert_new_node_in_already_released_area_twice() {
        let mut arena: NodeArray<i32, 10> = NodeArray::new();
        let node1 = TestNode { value: 42 };
        let node2 = TestNode { value: 43 };
        let node3 = TestNode { value: 44 };
        let node4 = TestNode { value: 45 };

        let idx1 = arena.insert(node1.value).unwrap();
        let _ = arena.insert(node2.value).unwrap();

        let idx3 = arena.insert(node3.value).unwrap();
        let _ = arena.insert(node4.value).unwrap();

        // 4 nodes are active in 4 allocations
        assert_eq!(arena.capacity(), 9);
        assert_eq!(arena.allocated(), 4);

        unsafe { arena.release_unchecked(idx3) };
        unsafe { arena.release_unchecked(idx1) };

        // 2 nodes are active in 4 allocations
        assert_eq!(arena.capacity(), 9);
        assert_eq!(arena.allocated(), 4);

        let node5 = TestNode { value: 46 };
        let idx5 = arena.insert(node5.value);

        let node6 = TestNode { value: 47 };
        let idx6 = arena.insert(node6.value);

        assert_eq!(idx5, Some(idx1));
        assert_eq!(idx6, Some(idx3));

        let fetched5 = unsafe { arena.get_unchecked(idx5.unwrap()) };
        assert_eq!(*fetched5, node5.value);

        let fetched6 = unsafe { arena.get_unchecked(idx6.unwrap()) };
        assert_eq!(*fetched6, node6.value);

        // after reusing released nodes, we still have the same allocation
        assert_eq!(arena.capacity(), 9);
        assert_eq!(arena.allocated(), 4);
    }
}
