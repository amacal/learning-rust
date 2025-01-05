use std::arch::global_asm;

#[derive(Clone, Copy)]
pub enum AllocatorBytes {
    B4096 = 4096,
    B8192 = 8192,
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

    fn alloc_two_bits(date: *const u8, bitmap: *const u64) -> *mut u8;
    fn free_two_bits(data: *const u8, bitmap: *const u64, ptr: *const u8);
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

        find_two_bits:
            bsf rcx, rax
            jz no_slot_two_bits

            mov rdx, 0x03
            shl rdx, cl

            mov r8, rax
            and r8, rdx
            cmp r8, rdx
            jne next_two_bits

            btr [rsi], rcx
            add rcx, 0x01
            btr [rsi], rcx
            sub rcx, 0x01
            shl rcx, 0x0c
            lea rax, [rdi + rcx]
            ret

        next_two_bits:
            btr rax, rcx
            jmp find_two_bits

        free_two_bits:
            mov rcx, rdx
            sub rcx, rdi
            shr rcx, 0x0c
            mov rax, 0x03
            shl rax, cl
            or [rsi], rax
            ret

        no_slot_two_bits:
            ret
    "#
);

impl Allocator for &Naive64Pages {
    fn alloc(&self, size: AllocatorBytes) -> Option<*mut u8> {
        unsafe {
            let ptr = match size {
                AllocatorBytes::B4096 => alloc_one_bit(self.data.as_ptr(), &self.bitmap as *const u64),
                AllocatorBytes::B8192 => alloc_two_bits(self.data.as_ptr(), &self.bitmap as *const u64),
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
                AllocatorBytes::B8192 => free_two_bits(self.data.as_ptr(), &self.bitmap as *const u64, ptr),
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
}
