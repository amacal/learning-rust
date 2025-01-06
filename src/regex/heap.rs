use std::marker::PhantomData;
use std::ptr;

use super::alloc::Allocator;
use super::alloc::AllocatorBytes;

pub trait AllocatorSize {
    fn measure() -> usize;
    fn encode() -> AllocatorBytes;
}

pub struct B4096 {}
pub struct B8192 {}
pub struct B16384 {}

impl AllocatorSize for B4096 {
    fn measure() -> usize {
        4096
    }

    fn encode() -> AllocatorBytes {
        AllocatorBytes::B4096
    }
}

impl AllocatorSize for B8192 {
    fn measure() -> usize {
        8192
    }

    fn encode() -> AllocatorBytes {
        AllocatorBytes::B8192
    }
}

impl AllocatorSize for B16384 {
    fn measure() -> usize {
        16384
    }

    fn encode() -> AllocatorBytes {
        AllocatorBytes::B16384
    }
}

pub trait Guard<T, SIZE: AllocatorSize> {
    fn apply<U: Into<usize>>(off: U) -> usize;

    fn deref_get(ptr: *const T, off: usize) -> T
    where
        T: Copy;

    fn deref_set(ptr: *mut T, off: usize, val: T);
}

pub struct GuardWrapping;
pub struct GuardDisabled;
pub struct GuardSegfault;

impl<T, SIZE: AllocatorSize> Guard<T, SIZE> for GuardWrapping {
    fn apply<U: Into<usize>>(off: U) -> usize {
        off.into() & (SIZE::measure() / size_of::<T>() - 1)
    }

    fn deref_get(ptr: *const T, off: usize) -> T
    where
        T: Copy,
    {
        unsafe { *ptr.add(off) }
    }

    fn deref_set(ptr: *mut T, off: usize, val: T) {
        unsafe { *ptr.add(off) = val }
    }
}

impl<T, SIZE: AllocatorSize> Guard<T, SIZE> for GuardDisabled {
    fn apply<U: Into<usize>>(off: U) -> usize {
        off.into()
    }

    fn deref_get(ptr: *const T, off: usize) -> T
    where
        T: Copy,
    {
        unsafe { *ptr.add(off) }
    }

    fn deref_set(ptr: *mut T, off: usize, val: T) {
        unsafe { *ptr.add(off) = val }
    }
}

impl<T, SIZE: AllocatorSize> Guard<T, SIZE> for GuardSegfault {
    fn apply<U: Into<usize>>(off: U) -> usize {
        off.into()
    }

    fn deref_get(ptr: *const T, off: usize) -> T
    where
        T: Copy,
    {
        unsafe {
            let src = if off < SIZE::measure() / size_of::<T>() { ptr.add(off) } else { ptr::null() };

            ptr::read_volatile(src)
        }
    }

    fn deref_set(ptr: *mut T, off: usize, val: T) {
        unsafe {
            let dst = if off < SIZE::measure() / size_of::<T>() { ptr.add(off) } else { ptr::null_mut() };

            ptr::write_volatile(dst, val);
        }
    }
}

pub struct Heap<T, ALLOCATOR: Allocator, SIZE: AllocatorSize, GUARD: Guard<T, SIZE>>(*mut T, ALLOCATOR, PhantomData<SIZE>, PhantomData<GUARD>);

impl<T, ALLOCATOR: Allocator, SIZE: AllocatorSize, GUARD: Guard<T, SIZE>> Heap<T, ALLOCATOR, SIZE, GUARD> {
    pub fn alloc(allocator: ALLOCATOR) -> Option<Self> {
        let ptr = allocator.alloc(SIZE::encode())?;
        Some(Self(ptr as *mut T, allocator, PhantomData, PhantomData))
    }

    pub fn as_bytes(&self, len: usize) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.0, len) }
    }

    fn deref_get(&self, off: usize) -> T
    where
        T: Copy,
    {
        GUARD::deref_get(self.0, off)
    }

    fn deref_set(&mut self, off: usize, val: T) {
        GUARD::deref_set(self.0, off, val);
    }

    pub fn guard0(&self, off: usize) -> usize {
        GUARD::apply(off)
    }

    pub fn guard1(&self, off: usize, inc: usize) -> usize {
        GUARD::apply(off + inc)
    }

    pub fn guard2(&self, off: usize, inc1: usize, inc2: usize) -> usize {
        GUARD::apply(off + inc1 + inc2)
    }

    pub fn get0<U>(&self, off: U) -> T
    where
        T: Copy,
        U: Into<usize>,
    {
        self.deref_get(self.guard0(off.into()))
    }

    pub fn set0<U>(&mut self, val: T, off: U)
    where
        T: Copy,
        U: Into<usize>,
    {
        self.deref_set(self.guard0(off.into()), val)
    }

    pub fn get1<U>(&self, off: U, inc: U) -> T
    where
        T: Copy,
        U: Into<usize>,
    {
        self.deref_get(self.guard1(off.into(), inc.into()))
    }

    pub fn set1<U>(&mut self, val: T, off: U, inc: U)
    where
        T: Copy,
        U: Into<usize>,
    {
        self.deref_set(self.guard1(off.into(), inc.into()), val);
    }

    pub fn get2<U>(&self, off: U, inc1: U, inc2: U) -> T
    where
        T: Copy,
        U: Into<usize>,
    {
        self.deref_get(self.guard2(off.into(), inc1.into(), inc2.into()))
    }

    pub fn set2<U>(&mut self, val: T, off: U, inc1: U, inc2: U)
    where
        T: Copy,
        U: Into<usize>,
    {
        self.deref_set(self.guard2(off.into(), inc1.into(), inc2.into()), val);
    }
}

impl<ALLOCATOR: Allocator, SIZE: AllocatorSize, GUARD: Guard<u16, SIZE>> Heap<u16, ALLOCATOR, SIZE, GUARD> {
    pub fn as_string(&self, len: usize) -> String {
        let bytes = self.as_bytes(len);
        let mut out = String::from("[");

        for (i, b) in bytes.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&format!("0x{:02x}", b));
        }

        out.push(']');
        out
    }
}

impl<T, ALLOCATOR: Allocator, SIZE: AllocatorSize, GUARD: Guard<T, SIZE>> Drop for Heap<T, ALLOCATOR, SIZE, GUARD> {
    fn drop(&mut self) {
        self.1.free(self.0 as *mut u8, SIZE::encode());
    }
}
