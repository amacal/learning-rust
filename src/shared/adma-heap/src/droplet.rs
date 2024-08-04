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

impl<T> DerefMut for Droplet<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.target
    }
}

impl Heap {
    pub fn droplet(self) -> Droplet<Heap> {
        fn destroy(mem: &mut Heap) {
            // tracing releases heap may help in any naive troubleshooting
            trace2(b"releasing heap droplet; addr=%x, size=%d\n", mem.ptr, mem.len);

            // use syscall to free memory without error propagation
            sys_munmap(mem.ptr, mem.len);
        }

        // tracing conversion heap to a droplet may help in any naive troubleshooting
        trace2(b"creating heap droplet; addr=%x, size=%d\n", self.ptr, self.len);
        Droplet::from(self, destroy)
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
