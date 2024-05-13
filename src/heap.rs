use ::core::ops::Deref;
use ::core::ops::DerefMut;
use ::core::ptr;

use crate::syscall::*;

pub struct HeapSlice {
    pub ptr: *const (),
    pub len: usize,
}

pub struct Heap {
    pub ptr: *mut (),
    pub len: usize,
}

pub enum HeapSlicing {
    Succeeded(HeapSlice),
    InvalidParameters(),
    OutOfRange(),
}

impl Heap {
    pub fn between(&self, start: usize, end: usize) -> HeapSlicing {
        if start > self.len || end > self.len {
            return HeapSlicing::OutOfRange();
        }

        if start > end {
            return HeapSlicing::InvalidParameters();
        }

        let slice = HeapSlice {
            ptr: unsafe { self.ptr.offset(start as isize) },
            len: end - start,
        };

        HeapSlicing::Succeeded(slice)
    }
}

pub enum MemoryAllocation {
    Succeeded(Heap),
    Failed(isize),
}

pub fn mem_alloc(len: usize) -> MemoryAllocation {
    const PROT_READ: usize = 0x00000001;
    const PROT_WRITE: usize = 0x00000002;
    const MAP_PRIVATE: usize = 0x00000002;
    const MAP_ANONYMOUS: usize = 0x00000020;

    let prot = PROT_READ | PROT_WRITE;
    let flags = MAP_PRIVATE | MAP_ANONYMOUS;

    let addr = ptr::null_mut();
    let addr = match sys_mmap(addr, len, prot, flags, 0, 0) {
        value if value <= 0 => return MemoryAllocation::Failed(value),
        value => Heap {
            ptr: value as *mut (),
            len: len,
        },
    };

    MemoryAllocation::Succeeded(addr)
}

#[allow(dead_code)]
pub enum MemoryDeallocation {
    Succeeded(),
    Failed(isize),
}

pub fn mem_free(memory: &mut Heap) -> MemoryDeallocation {
    match sys_munmap(memory.ptr, memory.len) {
        value if value == 0 => MemoryDeallocation::Succeeded(),
        value => MemoryDeallocation::Failed(value),
    }
}

pub struct Droplet<T> {
    target: T,
    destroy: fn(&mut T),
}

impl<T> Droplet<T> {
    pub fn from(target: T, destroy: fn(&mut T)) -> Self {
        Self {
            target: target,
            destroy: destroy,
        }
    }
}

impl<T> Drop for Droplet<T> {
    fn drop(&mut self) {
        (self.destroy)(&mut self.target)
    }
}

impl<T> Deref for Droplet<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.target
    }
}

impl Heap {
    pub fn droplet(self) -> Droplet<Heap> {
        fn free(mem: &mut Heap) {
            mem_free(mem);
        }

        Droplet::from(self, free)
    }

    pub fn boxed<T: HeapLifetime>(self) -> Boxed<T> {
        Boxed::at(self.ptr as *mut T)
    }
}

pub struct Boxed<T: HeapLifetime> {
    ptr: *mut T,
}

impl<T: HeapLifetime> Deref for Boxed<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T: HeapLifetime> DerefMut for Boxed<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}

impl<T: HeapLifetime> Boxed<T> {
    fn at(ptr: *mut T) -> Self {
        T::ctor(unsafe { &mut *ptr });
        let val = Boxed { ptr: ptr };

        val
    }
}

impl<T: HeapLifetime> Drop for Boxed<T> {
    fn drop(&mut self) {
        T::dtor(unsafe { &mut *self.ptr })
    }
}

pub trait HeapLifetime {
    fn ctor(&mut self);
    fn dtor(&mut self);
}
