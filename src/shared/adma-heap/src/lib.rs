#![cfg_attr(not(feature = "std"), no_std)]

mod alloc;
mod boxed;
mod core;
mod droplet;
mod kernel;
mod pool;
mod slice;
mod smart;
mod syscall;
mod trace;
mod view;

use ::core::marker::*;
use ::core::mem;
use ::core::ops::*;

use crate::kernel::*;
use crate::syscall::*;
use crate::trace::*;

pub struct Heap {
    ptr: usize,
    len: usize,
}

pub struct HeapRef {
    ptr: usize,
    len: usize,
}

pub struct HeapSlice<'a> {
    src: &'a Heap,
    off: usize,
    len: usize,
}

enum DropletTarget<'a, T> {
    Referenced(&'a mut T),
    Owned(T),
}


pub struct Droplet<T> {
    target: T,
    destroy: fn(&mut T),
}

pub struct Smart<T> {
    ptr: usize,
    len: usize,
    _pd: PhantomData<T>,
}

impl<T> PartialEq for Smart<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr == other.ptr && self.len == other.len && self._pd == other._pd
    }
}

pub struct HeapPool<const T: usize> {
    slots: [Option<HeapRef>; T],
    index: usize,
}

impl Heap {
    pub fn boxed<T: HeapLifetime>(self) -> Boxed<T> {
        trace2(b"creating boxed; addr=%x, size=%d\n", self.ptr, self.len);
        Boxed::at(self.ptr, self.len, self.ptr as *mut T)
    }
}

pub struct View<T> {
    ptr: *mut T,
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

        trace2(b"releasing boxed; addr=%x, size=%d\n", ptr, self.len);

        T::dtor(unsafe { &mut *self.ptr });
        sys_munmap(self.root, self.len);
    }
}

pub trait HeapLifetime {
    fn ctor(&mut self);
    fn dtor(&mut self);
}
