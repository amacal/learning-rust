use std::arch::asm;
use std::marker::PhantomData;
use std::ptr;

pub trait Guard<T, const SIZE: usize> {
    fn apply(off: usize) -> usize;

    fn deref_get(ptr: *const T, off: usize) -> T
    where
        T: Copy;

    fn deref_set(ptr: *mut T, off: usize, val: T);
}

pub struct GuardWrapping;
pub struct GuardDisabled;
pub struct GuardSegfault;

impl<T, const SIZE: usize> Guard<T, SIZE> for GuardWrapping {
    fn apply(off: usize) -> usize {
        off & (SIZE / size_of::<T>() - 1)
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

impl<T, const SIZE: usize> Guard<T, SIZE> for GuardDisabled {
    fn apply(off: usize) -> usize {
        off
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

impl<T, const SIZE: usize> Guard<T, SIZE> for GuardSegfault {
    fn apply(off: usize) -> usize {
        off
    }

    fn deref_get(ptr: *const T, off: usize) -> T
    where
        T: Copy,
    {
        unsafe {
            let src = if off < SIZE / size_of::<T>() {
                ptr.add(off)
            } else {
                ptr::null()
            };

            ptr::read_volatile(src)
        }
    }

    fn deref_set(ptr: *mut T, off: usize, val: T) {
        unsafe {
            let dst = if off < SIZE / size_of::<T>() {
                ptr.add(off)
            } else {
                ptr::null_mut()
            };

            ptr::write_volatile(dst, val);
        }
    }
}

pub struct Heap<T, const SIZE: usize, GUARD: Guard<T, SIZE>>(*mut T, PhantomData<GUARD>);

impl<T, const SIZE: usize, GUARD: Guard<T, SIZE>> Heap<T, SIZE, GUARD> {
    pub fn alloc() -> Self {
        let ptr = unsafe {
            let ret: isize;

            asm!(
                "syscall",
                in("rax") 9,
                in("rdi") 0,
                in("rsi") SIZE,
                in("rdx") 0x00000001 | 0x00000002,
                in("r10") 0x00000002 | 0x00000020,
                in("r8") 0,
                in("r9") 0,
                lateout("rcx") _,
                lateout("r11") _,
                lateout("rax") ret,
                options(nostack)
            );

            ret
        };

        Self(ptr as *mut T, PhantomData)
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
        GUARD::apply(off.wrapping_add(inc))
    }

    pub fn guard2(&self, off: usize, inc1: usize, inc2: usize) -> usize {
        GUARD::apply(off.wrapping_add(inc1).wrapping_add(inc2))
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

impl<const SIZE: usize, GUARD: Guard<u16, SIZE>> Heap<u16, SIZE, GUARD> {
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

impl<T, const SIZE: usize, GUARD: Guard<T, SIZE>> Drop for Heap<T, SIZE, GUARD> {
    fn drop(&mut self) {
        unsafe {
            asm!(
                "syscall",
                in("rax") 11,
                in("rdi") self.0,
                in("rsi") SIZE,
                lateout("rcx") _,
                lateout("r11") _,
                lateout("rax") _,
                options(nostack)
            );
        }
    }
}
