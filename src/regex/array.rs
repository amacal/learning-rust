use std::marker::PhantomData;
use std::mem;

use super::heap::*;

pub trait ArrayLike {}
pub trait StackLike {}

pub struct Array<LIKE, T, const SIZE: usize, GUARD: Guard<T, SIZE>> {
    heap: Heap<T, SIZE, GUARD>,
    like: PhantomData<LIKE>,
    head: u16,
    tail: u16,
}

impl<LIKE, T, const SIZE: usize, GUARD: Guard<T, SIZE>> Array<LIKE, T, SIZE, GUARD> {
    pub fn new() -> Self {
        Self { heap: Heap::alloc(), like: PhantomData, head: 0, tail: (SIZE / mem::size_of::<T>()) as u16 - 1 }
    }
}

impl<LIKE: StackLike, T: Copy, const SIZE: usize, GUARD: Guard<T, SIZE>> Array<LIKE, T, SIZE, GUARD> {
    pub fn stack_push_front(&mut self, val: T) {
        self.heap.set0(val, self.head);
        self.head += 1;
    }

    pub fn stack_push_back(&mut self, val: T) {
        self.heap.set0(val, self.tail);
        self.tail -= 1;
    }

    pub fn stack_pop_front(&mut self) -> T {
        self.head -= 1;
        self.heap.get0(self.head)
    }

    pub fn stack_pop_back(&mut self) -> T {
        self.tail += 1;
        self.heap.get0(self.tail)
    }

    pub fn stack_size_front(&self) -> u16 {
        self.head
    }

    pub fn stack_size_back(&self) -> u16 {
        (SIZE / size_of::<T>()) as u16 - self.tail
    }

    pub fn stack_peek_front(&self) -> T {
        self.heap.get0(self.head - 1)
    }

    pub fn stack_peek_back(&self) -> T {
        self.heap.get0(self.tail + 1)
    }

    // #[cfg(test)]
    pub fn as_bytes_front(&self) -> &[T] {
        self.heap.as_bytes(self.stack_size_front().into())
    }

    #[cfg(test)]
    pub fn as_bytes_back(&self) -> &[T] {
        self.heap.as_bytes(self.stack_size_back().into())
    }
}

impl<LIKE: ArrayLike, T: Copy, const SIZE: usize, GUARD: Guard<T, SIZE>> Array<LIKE, T, SIZE, GUARD> {
    pub fn array_get(&self, off: u16) -> T {
        self.heap.get0(off)
    }

    pub fn array_set(&mut self, off: u16, val: T) {
        self.heap.set0(val, off);
    }
}
