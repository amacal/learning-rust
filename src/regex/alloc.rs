use std::arch::global_asm;

#[derive(Clone, Copy)]
pub enum AllocatorBytes {
    B4096 = 4096,
    B8192 = 8192,
    B16384 = 16384,
}

pub trait Allocator {
    fn alloc(&self, size: AllocatorBytes) -> Option<*mut u8>;
    fn free(&self, ptr: *mut u8, size: AllocatorBytes);
}

#[repr(C, align(4096))]
pub struct Naive64Pages {
    data: [u8; 64 * 4096],
    bitmap: u64,
}

impl Naive64Pages {
    pub fn new() -> Self {
        Naive64Pages { data: [0; 64 * 4096], bitmap: 0xffff_ffff_ffff_ffff }
    }
}

extern "C" {
    fn alloc_one_bit(data: *const u8, bitmap: *const u64) -> *mut u8;
    fn free_one_bit(data: *const u8, bitmap: *const u64, ptr: *const u8);

    fn alloc_two_bits(date: *const u8, bitmap: *const u64, mask: u8) -> *mut u8;
    fn free_two_bits(data: *const u8, bitmap: *const u64, mask: u8, ptr: *const u8);

    fn alloc_n_bits(date: *const u8, bitmap: *const u64, mask: u8) -> *mut u8;
    fn free_n_bits(data: *const u8, bitmap: *const u64, mask: u8, ptr: *const u8);
}

global_asm!(
    r#"
        .global alloc_one_bit
        .global free_one_bit

        alloc_one_bit:
            bsf rdx, [rsi]
            jz no_slot_one_bit
            btr [rsi], rdx
            shl rdx, 0x0c
            lea rax, [rdi + rdx]
            ret

        free_one_bit:
            sub rdx, rdi
            shr rdx, 0x0c
            bts [rsi], rdx
            ret

        no_slot_one_bit:
            xor rax, rax
            ret
    "#
);

global_asm!(
    r#"
        .global alloc_two_bits
        .global free_two_bits

        alloc_two_bits:
            mov rax, [rsi]
            mov r9, rax
            shl r9, 0x01
            and rax, r9

            bsf rcx, rax
            jz no_slot_two_bits

            sub rcx, 0x01
            mov r9, rdx
            shl r9, cl

            not r9
            and [rsi], r9

            shl rcx, 0x0c
            lea rax, [rdi + rcx]
            ret

        free_two_bits:
            sub rcx, rdi
            shr rcx, 0x0c
            shl rdx, cl
            or [rsi], rdx
            ret

        no_slot_two_overflow:
            mov rax, 0x00

        no_slot_two_bits:
            ret
    "#
);

global_asm!(
    r#"
        .global alloc_n_bits
        .global free_n_bits

        alloc_n_bits:
            mov rax, [rsi]

        find_n_bits:
            bsf rcx, rax
            jz no_slot_n_bits

            mov r9, rdx
            shl r9, cl
            jc no_slot_n_overflow

            mov r8, rax
            and r8, r9
            cmp r8, r9
            jne next_n_bits

            not r9
            and [rsi], r9

            shl rcx, 0x0c
            lea rax, [rdi + rcx]
            ret

        next_n_bits:
            btr rax, rcx
            jmp find_n_bits

        free_n_bits:
            sub rcx, rdi
            shr rcx, 0x0c
            shl rdx, cl
            or [rsi], rdx
            ret

        no_slot_n_overflow:
            mov rax, 0x00

        no_slot_n_bits:
            ret
    "#
);

impl Allocator for &Naive64Pages {
    fn alloc(&self, size: AllocatorBytes) -> Option<*mut u8> {
        unsafe {
            let ptr = match size {
                AllocatorBytes::B4096 => alloc_one_bit(self.data.as_ptr(), &self.bitmap as *const u64),
                AllocatorBytes::B8192 => alloc_two_bits(self.data.as_ptr(), &self.bitmap as *const u64,0b11),
                AllocatorBytes::B16384 => alloc_n_bits(self.data.as_ptr(), &self.bitmap as *const u64,0b1111),
            };

            if ptr == core::ptr::null_mut() {
                return None;
            }

            Some(ptr)
        }
    }

    fn free(&self, ptr: *mut u8, size: AllocatorBytes) {
        unsafe {
            match size {
                AllocatorBytes::B4096 => free_one_bit(self.data.as_ptr(), &self.bitmap as *const u64, ptr),
                AllocatorBytes::B8192 => free_two_bits(self.data.as_ptr(), &self.bitmap as *const u64, 0b11, ptr),
                AllocatorBytes::B16384 => free_n_bits(self.data.as_ptr(), &self.bitmap as *const u64, 0b1111, ptr),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_one_page() {
        let pages = Naive64Pages::new();
        let allocator = &pages;
        let size = AllocatorBytes::B4096;

        let ptr = allocator.alloc(size);
        let data = pages.data.as_ptr() as usize;

        assert_eq!(ptr.unwrap() as usize, data);
    }

    #[test]
    fn allocates_one_page_65_times() {
        let pages = Naive64Pages::new();
        let allocator = &pages;
        let size = AllocatorBytes::B4096;

        for i in 0..64 {
            let ptr = allocator.alloc(size);
            let data = pages.data.as_ptr() as usize;

            assert_eq!(ptr.unwrap() as usize, data + i as usize * 4096);
        }

        let ptr = allocator.alloc(size);
        assert!(ptr.is_none());
    }

    #[test]
    fn allocates_one_page_same_100_times() {
        let pages = Naive64Pages::new();
        let allocator = &pages;
        let size = AllocatorBytes::B4096;

        let ptr1 = allocator.alloc(size);
        allocator.free(ptr1.unwrap(), size);

        for _ in 0..100 {
            let ptr2 = allocator.alloc(size);
            allocator.free(ptr2.unwrap(), size);

            assert_eq!(ptr1.unwrap(), ptr2.unwrap())
        }
    }

    #[test]
    fn allocates_two_pages() {
        let pages = Naive64Pages::new();
        let allocator = &pages;
        let size = AllocatorBytes::B8192;

        let ptr = allocator.alloc(size);
        let data = pages.data.as_ptr() as usize;

        assert_eq!(ptr.unwrap() as usize, data);
    }

    #[test]
    fn allocates_two_pages_33_times() {
        let pages = Naive64Pages::new();
        let allocator = &pages;
        let size = AllocatorBytes::B8192;

        for i in 0..32 {
            let ptr = allocator.alloc(size);
            let data = pages.data.as_ptr() as usize;

            println!("{:08x}", data);
            println!("{:08x}", ptr.unwrap() as usize);
            println!("{:08x}", data + i as usize * 8192);

            assert_eq!(ptr.unwrap() as usize, data + i as usize * 8192);
        }

        let ptr = allocator.alloc(size);
        assert!(ptr.is_none());
    }

    #[test]
    fn allocates_two_pages_same_100_times() {
        let pages = Naive64Pages::new();
        let allocator = &pages;
        let size = AllocatorBytes::B8192;

        let ptr1 = allocator.alloc(size);
        allocator.free(ptr1.unwrap(), size);

        for _ in 0..100 {
            let ptr2 = allocator.alloc(size);
            allocator.free(ptr2.unwrap(), size);

            assert_eq!(ptr1.unwrap(), ptr2.unwrap())
        }
    }

    #[test]
    fn allocates_two_pages_fragmented() {
        let pages = Naive64Pages::new();
        let allocator = &pages;
        let mut ptrs: [Option<*mut u8>; 8] = [const { None }; 8];

        let b4096 = AllocatorBytes::B4096;
        let b8192 = AllocatorBytes::B8192;

        for i in 0..ptrs.len() {
            ptrs[i] = allocator.alloc(b4096);
        }

        for i in 0..ptrs.len() {
            if i % 2 == 1 {
                allocator.free(ptrs[i].unwrap(), b4096);
            }
        }

        let ptr = allocator.alloc(b8192);
        let data = pages.data.as_ptr() as usize;

        assert_eq!(ptr.unwrap() as usize, data + 7 * 4096);
    }

    #[test]
    fn allocates_two_pages_fragmented_hole() {
        let pages = Naive64Pages::new();
        let allocator = &pages;
        let mut ptrs: [Option<*mut u8>; 8] = [const { None }; 8];

        let b4096 = AllocatorBytes::B4096;
        let b8192 = AllocatorBytes::B8192;

        for i in 0..ptrs.len() {
            ptrs[i] = allocator.alloc(b4096);
        }

        for i in 0..ptrs.len() {
            if i == 5 || i == 6 {
                allocator.free(ptrs[i].unwrap(), b4096);
            }
        }

        let ptr = allocator.alloc(b8192);
        let data = pages.data.as_ptr() as usize;

        assert_eq!(ptr.unwrap() as usize, data + 5 * 4096);

        let ptr = allocator.alloc(b8192);
        let data = pages.data.as_ptr() as usize;

        assert_eq!(ptr.unwrap() as usize, data + 8 * 4096);
    }

    #[test]
    fn allocates_two_pages_fragmented_edge() {
        let pages = Naive64Pages::new();
        let allocator = &pages;

        let b4096 = AllocatorBytes::B4096;
        let b8192 = AllocatorBytes::B8192;

        for _ in 0..63 {
            let _ = allocator.alloc(b4096);
        }

        assert!(allocator.alloc(b8192).is_none());
    }

    #[test]
    fn allocates_two_pages_fragmented_edge_almost() {
        let pages = Naive64Pages::new();
        let allocator = &pages;

        let b4096 = AllocatorBytes::B4096;
        let b8192 = AllocatorBytes::B8192;

        for _ in 0..62 {
            let _ = allocator.alloc(b4096);
        }

        let ptr = allocator.alloc(b8192);
        let data = pages.data.as_ptr() as usize;

        assert_eq!(ptr.unwrap() as usize, data + 62 * 4096);
    }

    #[test]
    fn allocates_four_pages() {
        let pages = Naive64Pages::new();
        let allocator = &pages;
        let size = AllocatorBytes::B16384;

        let ptr = allocator.alloc(size);
        let data = pages.data.as_ptr() as usize;

        assert_eq!(ptr.unwrap() as usize, data);
    }

    #[test]
    fn allocates_four_pages_17_times() {
        let pages = Naive64Pages::new();
        let allocator = &pages;
        let size = AllocatorBytes::B16384;

        for i in 0..16 {
            let ptr = allocator.alloc(size);
            let data = pages.data.as_ptr() as usize;

            assert_eq!(ptr.unwrap() as usize, data + i as usize * 16384);
        }

        let ptr = allocator.alloc(size);
        assert!(ptr.is_none());
    }

    #[test]
    fn allocates_four_pages_same_100_times() {
        let pages = Naive64Pages::new();
        let allocator = &pages;
        let size = AllocatorBytes::B16384;

        let ptr1 = allocator.alloc(size);
        allocator.free(ptr1.unwrap(), size);

        for _ in 0..100 {
            let ptr2 = allocator.alloc(size);
            allocator.free(ptr2.unwrap(), size);

            assert_eq!(ptr1.unwrap(), ptr2.unwrap())
        }
    }

    #[test]
    fn allocates_four_pages_fragmented() {
        let pages = Naive64Pages::new();
        let allocator = &pages;
        let mut ptrs: [Option<*mut u8>; 8] = [const { None }; 8];

        let b4096 = AllocatorBytes::B4096;
        let b16384 = AllocatorBytes::B16384;

        for i in 0..ptrs.len() {
            ptrs[i] = allocator.alloc(b4096);
        }

        for i in 0..ptrs.len() {
            if i % 2 == 1 {
                allocator.free(ptrs[i].unwrap(), b4096);
            }
        }

        let ptr = allocator.alloc(b16384);
        let data = pages.data.as_ptr() as usize;

        assert_eq!(ptr.unwrap() as usize, data + 7 * 4096);
    }

    #[test]
    fn allocates_four_pages_fragmented_hole() {
        let pages = Naive64Pages::new();
        let allocator = &pages;
        let mut ptrs: [Option<*mut u8>; 8] = [const { None }; 8];

        let b4096 = AllocatorBytes::B4096;
        let b16384 = AllocatorBytes::B16384;

        for i in 0..ptrs.len() {
            ptrs[i] = allocator.alloc(b4096);
        }

        for i in 0..ptrs.len() {
            if i == 3 || i == 4 || i == 5 || i == 6 {
                allocator.free(ptrs[i].unwrap(), b4096);
            }
        }

        let ptr = allocator.alloc(b16384);
        let data = pages.data.as_ptr() as usize;

        assert_eq!(ptr.unwrap() as usize, data + 3 * 4096);

        let ptr = allocator.alloc(b16384);
        let data = pages.data.as_ptr() as usize;

        assert_eq!(ptr.unwrap() as usize, data + 8 * 4096);
    }

    #[test]
    fn allocates_four_pages_fragmented_edge() {
        let pages = Naive64Pages::new();
        let allocator = &pages;

        let b4096 = AllocatorBytes::B4096;
        let b16384 = AllocatorBytes::B16384;

        for _ in 0..61 {
            let _ = allocator.alloc(b4096);
        }

        assert!(allocator.alloc(b16384).is_none());
    }

    #[test]
    fn allocates_four_pages_fragmented_edge_almost() {
        let pages = Naive64Pages::new();
        let allocator = &pages;

        let b4096 = AllocatorBytes::B4096;
        let b16384 = AllocatorBytes::B16384;

        for _ in 0..60 {
            let _ = allocator.alloc(b4096);
        }

        let ptr = allocator.alloc(b16384);
        let data = pages.data.as_ptr() as usize;

        assert_eq!(ptr.unwrap() as usize, data + 60 * 4096);
    }
}
