use super::heap::*;
use std::marker::PhantomData;

pub trait ArrayLike {}
pub trait StackLike {}

pub struct Array<LIKE, T, const SIZE: usize, GUARD: Guard<T, SIZE>> {
    heap: Heap<T, SIZE, GUARD>,
    like: PhantomData<LIKE>,
    head: u16,
}

impl<LIKE, T, const SIZE: usize, GUARD: Guard<T, SIZE>> Array<LIKE, T, SIZE, GUARD> {
    pub fn new() -> Self {
        Self {
            heap: Heap::alloc(),
            like: PhantomData,
            head: 0,
        }
    }
}

impl<LIKE: StackLike, T: Copy, const SIZE: usize, GUARD: Guard<T, SIZE>> Array<LIKE, T, SIZE, GUARD> {
    pub fn stack_push(&mut self, val: T) {
        self.heap.set0(val, self.head);
        self.head = self.head.wrapping_add(1);
    }

    pub fn stack_pop(&mut self) -> T {
        self.head = self.head.wrapping_sub(1);
        self.heap.get0(self.head)
    }

    pub fn stack_size(&self) -> u16 {
        self.head
    }

    pub fn stack_peek(&self) -> T {
        self.heap.get0(self.head.wrapping_sub(1))
    }

    #[cfg(test)]
    pub fn as_bytes(&self) -> &[T] {
        self.heap.as_bytes(self.stack_size().into())
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

#[cfg(test)]
mod tests {
    use super::*;
}
