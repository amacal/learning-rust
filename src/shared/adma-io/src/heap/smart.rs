use ::core::marker::*;
use ::core::ops::*;
use ::core::mem;

use super::*;

pub struct Smart<T> {
    ptr: usize,
    len: usize,
    _pd: PhantomData<T>,
}

impl <T> Smart<T> {
    fn new(ptr: usize, len: usize) -> Self {
        Self {
            ptr: ptr,
            len: len,
            _pd: PhantomData,
        }
    }

    pub fn allocate() -> Option<Smart<T>> {
        let len = mem::size_of::<SmartBox<T>>();
        let heap = match mem_alloc(len) {
            MemoryAllocation::Succeeded(heap) => heap,
            MemoryAllocation::Failed(_) => return None,
        };

        unsafe {
            (*(heap.ptr as *mut SmartBox<T>)).cnt = 1;
        }

        Some(Smart::new(heap.ptr, heap.len))
    }
}

impl<T> Smart<T> {
    pub fn duplicate(&self) -> Smart<T> {
        unsafe {
            (*(self.ptr as *mut SmartBox<T>)).cnt += 1;
        }

        Self {
            ptr: self.ptr,
            len: self.len,
            _pd: self._pd,
        }
    }
}

impl<T> Drop for Smart<T> {
    fn drop(&mut self) {
        unsafe { (*(self.ptr as *mut SmartBox<T>)).cnt -= 1 }
    }
}

struct SmartBox<T> {
    val: T,
    cnt: usize,
}

impl<T> Deref for Smart<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &(*(self.ptr as *const SmartBox<T>)).val }
    }
}

impl<T> DerefMut for Smart<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut (*(self.ptr as *mut SmartBox<T>)).val }
    }
}
