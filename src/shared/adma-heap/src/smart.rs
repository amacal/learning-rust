use ::core::marker::*;
use ::core::ops::*;
use ::core::mem;

use super::*;

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
        let heap = match Heap::allocate(len) {
            Ok(heap) => heap,
            Err(_) => return None,
        };

        unsafe {
            (*(heap.ptr as *mut SmartBox<T>)).cnt = 1;
        }

        Some(Smart::new(heap.ptr, heap.len))
    }
}

impl<T> Smart<T> {
    #[cfg(test)]
    fn counter(&self) -> usize {
        unsafe {
            (*(self.ptr as *mut SmartBox<T>)).cnt
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    struct Testing {
        val: usize,
    }

    #[test]
    fn allocate_one_page_rounded_up() {
        let heap = match Smart::<Testing>::allocate() {
            Some(value) => value,
            None => return assert!(false),
        };

        assert_ne!(heap.ptr, 0);
        assert_eq!(heap.len, 4096);
        assert_eq!(heap.counter(), 1);
    }

    #[test]
    fn duplicate_allocated_smart() {
        let (first, second) = match Smart::<Testing>::allocate() {
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
        let (first, second) = match Smart::<Testing>::allocate() {
            Some(value) => (value.duplicate(), value),
            None => return assert!(false),
        };

        drop(first);
        assert_eq!(second.counter(), 1);
    }
}
