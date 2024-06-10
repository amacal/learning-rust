use ::core::mem;
use ::core::ops::Deref;
use ::core::ops::DerefMut;
use ::core::ptr;

use crate::kernel::*;
use crate::syscall::*;
use crate::trace::*;

pub struct HeapSlice<'a> {
    src: &'a Heap,
    off: usize,
    len: usize,
}

impl<'a> HeapSlice<'a> {
    pub fn ptr(&self) -> usize {
        self.src.ptr + self.off
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

pub struct Heap {
    pub ptr: usize,
    pub len: usize,
}

impl Heap {
    pub fn at(ptr: usize, len: usize) -> Self {
        Self { ptr: ptr, len: len }
    }
}

pub enum HeapSlicing<'a> {
    Succeeded(HeapSlice<'a>),
    InvalidParameters(),
    OutOfRange(),
}

impl Heap {
    pub fn between<'a>(&'a self, start: usize, end: usize) -> HeapSlicing<'a> {
        if start > self.len || end > self.len {
            return HeapSlicing::OutOfRange();
        }

        if start > end {
            return HeapSlicing::InvalidParameters();
        }

        let slice = HeapSlice {
            src: self,
            off: start,
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
    let prot = PROT_READ | PROT_WRITE;
    let flags = MAP_PRIVATE | MAP_ANONYMOUS;

    let addr = ptr::null_mut();
    let addr = match sys_mmap(addr, len, prot, flags, 0, 0) {
        value if value <= 0 => return MemoryAllocation::Failed(value),
        value => Heap {
            ptr: value as usize,
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
    match sys_munmap(memory.ptr as *mut (), memory.len) {
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

    pub fn as_ptr(&self) -> (usize, usize) {
        (self.ptr, self.len)
    }

    pub fn boxed<T: HeapLifetime>(self) -> Boxed<T> {
        trace2(b"creating boxed; addr=%x, size=%d\n", self.ptr, self.len);
        Boxed::at(self.ptr, self.len, self.ptr as *mut T)
    }

    pub fn view<T>(&self) -> View<T> {
        trace2(b"creating view; addr=%x, size=%d\n", self.ptr, self.len);
        View::at(self.ptr as *mut T)
    }
}

pub struct View<T> {
    ptr: *mut T,
}

impl<T> View<T> {
    fn at(ptr: *mut T) -> Self {
        Self { ptr }
    }
}

impl<T> Deref for View<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T> DerefMut for View<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}

pub struct Boxed<T: HeapLifetime> {
    root: usize,
    len: usize,
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
    fn at(root: usize, len: usize, ptr: *mut T) -> Self {
        T::ctor(unsafe { &mut *ptr });
        let val = Boxed { root, ptr, len };

        val
    }
}

impl<T: HeapLifetime> Into<Heap> for Boxed<T> {
    fn into(self) -> Heap {
        let ptr = self.ptr as usize;
        let heap = Heap::at(self.root, self.len);

        trace2(b"forgetting boxed; addr=%x, size=%d\n", ptr, self.len);

        mem::forget(self);
        heap
    }
}

impl<T: HeapLifetime> Drop for Boxed<T> {
    fn drop(&mut self) {
        let ptr = self.ptr as usize;
        let mut heap = Heap::at(self.root, self.len);

        trace2(b"releasing boxed; addr=%x, size=%d\n", ptr, self.len);

        T::dtor(unsafe { &mut *self.ptr });
        mem_free(&mut heap);
    }
}

pub trait HeapLifetime {
    fn ctor(&mut self);
    fn dtor(&mut self);
}
