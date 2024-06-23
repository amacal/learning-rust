use ::core::ops::*;

use super::*;

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
            trace2(b"releasing droplet; addr=%x, size=%d\n", mem.ptr, mem.len);
            sys_munmap(mem.ptr, mem.len);
        }

        trace2(b"creating droplet; addr=%x, size=%d\n", self.ptr, self.len);
        Droplet::from(self, free)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_and_drop() {
        let heap = match Heap::allocate(128) {
            Ok(heap) => heap.droplet(),
            Err(_) => return assert!(false),
        };

        drop(heap);
    }
}
