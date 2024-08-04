use super::*;

impl Heap {
    pub fn at(ptr: usize, len: usize) -> Self {
        Self { ptr: ptr, len: len }
    }

    pub fn from(src: &HeapRef) -> Self {
        Self {
            ptr: src.ptr,
            len: src.len,
        }
    }

    pub fn as_ref(&self) -> HeapRef {
        HeapRef::new(self.ptr, self.len)
    }
}

impl HeapRef {
    pub fn new(ptr: usize, len: usize) -> Self {
        Self { ptr, len }
    }

    pub fn as_ptr(&self) -> (usize, usize) {
        (self.ptr, self.len)
    }

    pub fn ptr(&self) -> usize {
        self.ptr
    }

    pub fn len(&self) -> usize {
        self.len
    }
}
