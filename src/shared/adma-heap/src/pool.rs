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

        let heap = match self.slots.get_mut(self.index - 1) {
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

    fn drop_ref(&mut self) {
        trace2(b"releasing heap-pool droplet; idx=%d, size=%d\n", self.index, self.slots.len());

        unsafe {
            for idx in 0..self.index {
                match self.slots.get_unchecked_mut(idx) {
                    None => trace1(b"releasing heap-pool droplet; warning, idx=%d\n", idx),
                    Some(heap) => match Heap::from(heap).free() {
                        Ok(()) => trace1(b"releasing heap-pool droplet; succeeded, idx=%d\n", idx),
                        Err(_) => trace1(b"releasing heap-pool droplet; failed, idx=%d\n", idx),
                    },
                }
            }
        }

        self.index = 0;
    }

    pub fn droplet(self) -> Droplet<Self> {
        trace2(b"creating heap-pool droplet; idx=%x, size=%d\n", self.index, self.slots.len());
        Droplet::from(self, Self::drop_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_heap_success() {
        let pool = HeapPool::<1>::new();
        let mut pool = pool.droplet();

        let heap = match Heap::allocate(128) {
            Ok(value) => value.as_ref(),
            Err(_) => return assert!(false),
        };

        if let Some(_) = pool.release(heap) {
            assert!(false);
        }

        assert_eq!(pool.index, 1);
        drop(pool);
    }

    #[test]
    fn release_heap_failure_due_to_missing_slots() {
        let pool = HeapPool::<1>::new();
        let mut pool = pool.droplet();

        let heap = match Heap::allocate(128) {
            Ok(value) => value.as_ref(),
            Err(_) => return assert!(false),
        };

        if let Some(_) = pool.release(heap) {
            assert!(false);
        }

        let (ptr, heap) = match Heap::allocate(128) {
            Ok(value) => (value.ptr, value.as_ref()),
            Err(_) => return assert!(false),
        };

        match pool.release(heap) {
            Some(value) => assert_eq!(value.ptr, ptr),
            None => return assert!(false),
        }
    }

    #[test]
    fn acquire_heap() {
        let pool = HeapPool::<1>::new();
        let mut pool = pool.droplet();

        let (ptr, heap) = match Heap::allocate(128) {
            Ok(value) => (value.ptr, value.as_ref()),
            Err(_) => return assert!(false),
        };

        if let Some(_) = pool.release(heap) {
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
        let pool = HeapPool::<1>::new();
        let mut pool = pool.droplet();

        if let Some(_) = pool.acquire(256) {
            assert!(false);
        };
    }
}
