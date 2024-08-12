use super::*;

impl Heap {
    pub fn allocate(len: usize) -> Result<Self, Option<i32>> {
        let prot = PROT_READ | PROT_WRITE;
        let flags = MAP_PRIVATE | MAP_ANONYMOUS;

        let req = len;
        let len = ((len + 4095) / 4096) * 4096;

        let addr = match sys_mmap(len, prot, flags) {
            value if value > 0 => Self::at(value as usize, len),
            value => match i32::try_from(value) {
                Ok(value) => return Err(Some(value)),
                Err(_) => return Err(None),
            },
        };

        trace2(b"allocating heap; addr=%x, size=%d\n", addr.ptr, req);
        Ok(addr)
    }

    pub fn free(self) -> Result<(), Option<i32>> {
        // tracing releases heap may help in any naive troubleshooting
        trace2(b"releasing heap; addr=%x, size=%d\n", self.ptr, self.len);

        // use syscall to free memory with error propagation
        match sys_munmap(self.ptr, self.len) {
            value if value == 0 => Ok(()),
            value if value > 0 => Err(None),
            value => match i32::try_from(value) {
                Ok(value) => Err(Some(value)),
                Err(_) => Err(None),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_one_page_rounded_up() {
        let heap = match Heap::allocate(128) {
            Ok(heap) => heap.as_ref(),
            Err(_) => return assert!(false),
        };

        assert_ne!(heap.ptr, 0);
        assert_eq!(heap.len, 4096);
    }

    #[test]
    fn allocate_one_page_exact() {
        let heap = match Heap::allocate(4096) {
            Ok(heap) => heap.as_ref(),
            Err(_) => return assert!(false),
        };

        assert_ne!(heap.ptr, 0);
        assert_eq!(heap.len, 4096);
    }

    #[test]
    fn free_allocated_page_success() {
        let heap = match Heap::allocate(128) {
            Ok(heap) => heap,
            Err(_) => return assert!(false),
        };

        if let Err(_) = heap.free() {
            assert!(false);
        }
    }

    #[test]
    fn free_allocated_page_failure_because_unaligned() {
        match Heap::at(0x0010, 4096).free() {
            Ok(()) => return assert!(false),
            Err(errno) => assert_eq!(errno, Some(-22)),
        }
    }
}
