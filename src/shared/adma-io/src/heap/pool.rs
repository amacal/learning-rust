use crate::trace::*;

pub struct HeapRef {
    pub ptr: usize,
    pub len: usize,
}

impl HeapRef {
    pub fn new(ptr: usize, len: usize) -> Self {
        Self { ptr, len }
    }
}

pub struct HeapPool<const T: usize> {
    slots: [Option<HeapRef>; T],
    index: usize,
}

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

        trace1(b"test %d\n", self.index);
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
