use core::ffi::CStr;
use core::ptr::null_mut;

use crate::syscall::{sys_close, sys_mmap, sys_munmap, sys_open, sys_read, sys_write};

pub struct MemorySlice {
    ptr: *const u8,
    len: usize,
}

impl MemorySlice {
    pub fn from(data: &[u8]) -> Self {
        Self {
            ptr: data.as_ptr(),
            len: data.len(),
        }
    }
}

pub struct MemoryAddress {
    ptr: *mut u8,
    len: usize,
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

pub struct FileDescriptor {
    value: u32,
}

pub enum FileOpenining {
    Succeeded(FileDescriptor),
    Failed(isize),
}

pub fn file_open(pathname: &CStr) -> FileOpenining {
    match sys_open(pathname.to_bytes_with_nul().as_ptr(), 0, 0) {
        value if value < 0 => FileOpenining::Failed(value),
        value => FileOpenining::Succeeded(FileDescriptor {
            value: value as u32,
        }),
    }
}

pub fn stdout_open() -> FileDescriptor {
    FileDescriptor { value: 1 }
}

pub fn stderr_open() -> FileDescriptor {
    FileDescriptor { value: 2 }
}

pub enum FileReading {
    Succeeded(usize),
    EndOfFile(),
    Failed(isize),
}

pub fn file_read(file: &FileDescriptor, buffer: &mut MemoryAddress) -> FileReading {
    match sys_read(file.value, buffer.ptr, buffer.len) {
        value if value < 0 => FileReading::Failed(value),
        value if value == 0 => FileReading::EndOfFile(),
        value => FileReading::Succeeded(value as usize),
    }
}

pub enum FileWriting {
    Succeeded(usize),
    Failed(isize),
}

pub fn file_write(file: &mut FileDescriptor, buffer: &MemorySlice) -> FileWriting {
    match sys_write(file.value, buffer.ptr, buffer.len) {
        value if value < 0 => FileWriting::Failed(value),
        value => FileWriting::Succeeded(value as usize),
    }
}

pub enum FileClosing {
    Succeeded(),
    Failed(isize),
}

pub fn file_close(file: FileDescriptor) -> FileClosing {
    match sys_close(file.value) {
        value if value < 0 => FileClosing::Failed(value),
        _ => FileClosing::Succeeded(),
    }
}
