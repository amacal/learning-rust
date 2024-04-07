use core::ptr::null_mut;

use crate::syscall::*;

pub struct MemorySlice {
    pub ptr: *const u8,
    pub len: usize,
}

pub struct MemoryAddress {
    pub ptr: *mut u8,
    pub len: usize,
}

pub enum MemorySlicing {
    Succeeded(MemorySlice),
    InvalidParameters(),
    OutOfRange(),
}

impl MemoryAddress {
    pub fn between(&self, start: usize, end: usize) -> MemorySlicing {
        if start > self.len || end > self.len {
            return MemorySlicing::OutOfRange();
        }

        if start > end {
            return MemorySlicing::InvalidParameters();
        }

        let slice = MemorySlice {
            ptr: unsafe { self.ptr.offset(start as isize) },
            len: end - start,
        };

        MemorySlicing::Succeeded(slice)
    }
}

pub enum MemoryAllocation {
    Succeeded(MemoryAddress),
    Failed(isize),
}

pub fn mem_alloc(length: usize) -> MemoryAllocation {
    let address = match sys_mmap(null_mut(), length, 0x03, 0x22, 0, 0) {
        value if value <= 0 => return MemoryAllocation::Failed(value),
        value => MemoryAddress {
            ptr: value as *mut u8,
            len: length,
        },
    };

    MemoryAllocation::Succeeded(address)
}

pub enum MemoryDeallocation {
    Succeeded(),
    Failed(isize),
}

pub fn mem_free(memory: MemoryAddress) -> MemoryDeallocation {
    match sys_munmap(memory.ptr, memory.len) {
        value if value == 0 => MemoryDeallocation::Succeeded(),
        value => MemoryDeallocation::Failed(value),
    }
}
