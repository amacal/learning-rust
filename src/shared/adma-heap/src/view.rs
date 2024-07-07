
use super::*;

impl Heap {
    pub fn view<'a, T>(&self) -> View<T> where View<T>: 'a {
        // tracing conversion heap to a view may help in any naive troubleshooting
        trace2(b"creating view; addr=%x, size=%d\n", self.ptr, self.len);

        // returned view will support dereferencing
        View::at(self.ptr as *mut T)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Pair {
        first: u32,
        second: u32,
    }

    #[test]
    fn allocate_and_view() {
        let mut pair: View<Pair> = match Heap::allocate(128) {
            Ok(heap) => heap.view(),
            Err(_) => return assert!(false),
        };

        pair.first = 32;
        pair.second = 64;

        assert_eq!(pair.first, 32);
        assert_eq!(pair.second, 64);
    }
}
