use ::core::marker::*;
use ::core::mem;
use ::core::ops::*;

use super::*;

impl<T> Smart<T> {
    fn new(ptr: usize, len: usize) -> Self {
        Self {
            ptr: ptr,
            len: len,
            _pd: PhantomData,
        }
    }

    pub fn allocate() -> Option<Smart<T>> {
        let len = mem::size_of::<SmartBox<T>>();
        let heap = match Heap::allocate(len) {
            Ok(heap) => heap,
            Err(_) => return None,
        };

        unsafe {
            (*(heap.ptr as *mut SmartBox<T>)).cnt = 1;
        }

        trace3(b"allocating smart; addr=%x, size=%d, cnt=%d\n", heap.ptr(), len, 1);
        Some(Smart::new(heap.ptr, heap.len))
    }
}

impl<T> Smart<T> {
    #[cfg(test)]
    fn counter(&self) -> usize {
        unsafe { (*(self.ptr as *mut SmartBox<T>)).cnt }
    }

    pub fn duplicate(&self) -> Smart<T> {
        let val = unsafe {
            (*(self.ptr as *mut SmartBox<T>)).cnt += 1;
            (*(self.ptr as *mut SmartBox<T>)).cnt
        };

        trace3(b"duplicating smart; addr=%x, size=%d, cnt=%d\n", self.ptr, self.len, val);

        Self {
            ptr: self.ptr,
            len: self.len,
            _pd: self._pd,
        }
    }
}

impl<T> Drop for Smart<T> {
    fn drop(&mut self) {
        let val = unsafe {
            (*(self.ptr as *mut SmartBox<T>)).cnt -= 1;
            (*(self.ptr as *mut SmartBox<T>)).cnt
        };

        trace3(b"dropping smart; addr=%x, size=%d, cnt=%d\n", self.ptr, self.len, val);

        if val == 0 {
            // in case of error we can only log it, no-way to propagate it to the caller
            if let Err(_) = Heap::at(self.ptr, self.len).free() {
                trace3(b"dropping smart; addr=%x, size=%d, cnt=%d, failed\n", self.ptr, self.len, val);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Pair {
        first: u32,
        second: u32,
    }

    #[test]
    fn allocate_one_page_rounded_up() {
        let heap = match Smart::<Pair>::allocate() {
            Some(value) => value,
            None => return assert!(false),
        };

        assert_ne!(heap.ptr, 0);
        assert_eq!(heap.len, 4096);
        assert_eq!(heap.counter(), 1);
    }

    #[test]
    fn access_created_pointer() {
        let mut heap = match Smart::<Pair>::allocate() {
            Some(value) => value,
            None => return assert!(false),
        };

        heap.first = 32;
        heap.second = 64;

        assert_eq!(heap.first, 32);
        assert_eq!(heap.second, 64);
    }

    #[test]
    fn duplicate_allocated_smart() {
        let (first, second) = match Smart::<Pair>::allocate() {
            Some(value) => (value.duplicate(), value),
            None => return assert!(false),
        };

        assert_ne!(second.ptr, 0);
        assert_eq!(first.ptr, second.ptr);

        assert_eq!(first.len, 4096);
        assert_eq!(second.len, 4096);

        assert_eq!(first.counter(), 2);
        assert_eq!(second.counter(), 2);
    }

    #[test]
    fn release_duplicated_smart() {
        let (first, second) = match Smart::<Pair>::allocate() {
            Some(value) => (value.duplicate(), value),
            None => return assert!(false),
        };

        drop(first);
        assert_eq!(second.counter(), 1);
    }
}
