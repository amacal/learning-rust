use crate::trace::*;

use super::*;

impl<const T: usize> HeapPool<T> {
    pub fn new() -> Self {
        Self {
            slots: [const { None }; T],
            index: 0,
        }
    }

    pub fn acquire(&mut self, len: usize) -> Option<HeapRef> {
        if len > 4096 || self.index == 0 || self.index > T {
            return None;
        }

        let heap = match self.slots.get_mut(self.index-1) {
            Some(heap) => heap.take(),
            None => return None,
        };

        self.index -= 1;
        heap
    }

    pub fn release(&mut self, heap: HeapRef) -> Option<HeapRef> {
        if heap.len != 4096 {
            return Some(heap);
        }

        let slot = match self.slots.get_mut(self.index) {
            Some(heap) => heap,
            None => return Some(heap),
        };

        *slot = Some(heap);
        self.index += 1;

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_heap() {
        let mut pool = HeapPool::<1>::new();
        let heap = match Heap::allocate(128) {
            Ok(value) => value.as_ref(),
            Err(_) => return assert!(false),
        };

        if let Some(heap) = pool.release(heap) {
            assert!(false);
        }
    }

    #[test]
    fn release_heap_failure_due_to_missing_slots() {
        let mut pool = HeapPool::<1>::new();
        let heap = match Heap::allocate(128) {
            Ok(value) => value.as_ref(),
            Err(_) => return assert!(false),
        };

        if let Some(heap) = pool.release(heap) {
            assert!(false);
        }

        let (ptr, heap) = match Heap::allocate(128) {
            Ok(value) => (value.ptr(), value.as_ref()),
            Err(_) => return assert!(false),
        };

        match pool.release(heap) {
            Some(value) => assert_eq!(value.ptr, ptr),
            None => return assert!(false),
        }
    }

    #[test]
    fn acquire_heap() {
        let mut pool = HeapPool::<1>::new();
        let (ptr, heap) = match Heap::allocate(128) {
            Ok(value) => (value.ptr(), value.as_ref()),
            Err(_) => return assert!(false),
        };

        if let Some(heap) = pool.release(heap) {
            assert!(false);
        }

        let heap = match pool.acquire(256) {
            Some(heap) => heap,
            None => return assert!(false),
        };

        assert_eq!(heap.ptr, ptr);
        assert_eq!(heap.len, 4096);
    }

    #[test]
    fn acquire_heap_failure_due_to_missing_release() {
        let mut pool = HeapPool::<1>::new();

        if let Some(_) = pool.acquire(256) {
            assert!(false);
        };
    }
}
