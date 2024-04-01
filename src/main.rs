#![no_std]
#![no_main]

mod linux;
mod syscall;

use core::arch::global_asm;
use core::ffi::CStr;

use crate::linux::{file_close, FileClosing};
use crate::linux::{file_open, FileOpenining};
use crate::linux::{file_read, FileReading};
use crate::linux::{file_write, FileWriting};
use crate::linux::{mem_alloc, mem_free, MemoryAllocation, MemoryDeallocation};
use crate::linux::{stderr_open, stdout_open};
use crate::linux::{MemorySlice, MemorySlicing};
use crate::syscall::sys_exit;

global_asm! {
    ".global _start",
    "_start:",
    "mov rdi, [rsp]",
    "lea rsi, [rsp + 8]",
    "push rsi",
    "push rdi",
    "mov rdi, rsp",
    "call main"
}

#[repr(C)]
pub struct ProcessArguments {
    argc: usize,
    argv: *const *const u8,
}

impl ProcessArguments {
    pub fn len(&self) -> usize {
        self.argc
    }

    pub fn get(&self, index: usize) -> Option<&CStr> {
        if index >= self.argc {
            return None
        }

        unsafe {
            Some(CStr::from_ptr(*self.argv.add(index) as *const i8))
        }
    }
}

#[no_mangle]
extern "C" fn main(args: &ProcessArguments) -> ! {
    let buffer_size = 16 * 4096;
    let mut target = stdout_open();

    let mut buffer = match mem_alloc(buffer_size) {
        MemoryAllocation::Failed(_) => fail(-2, b"Cannot allocate memory.\n"),
        MemoryAllocation::Succeeded(value) => value,
    };

    let pathname = match args.get(1) {
        None => fail(-2, b"Cannot find path in the first argument.\n"),
        Some(value) => value
    };

    let source = match file_open(pathname) {
        FileOpenining::Failed(_) => fail(-2, b"Cannot open source file.\n"),
        FileOpenining::Succeeded(value) => value,
    };

    loop {
        let read = match file_read(&source, &mut buffer) {
            FileReading::Failed(_) => fail(-2, b"Cannot read from source file.\n"),
            FileReading::Succeeded(value) => value,
            FileReading::EndOfFile() => break,
        };

        let mut index = 0;
        while index < read {
            let write = match buffer.between(index, read) {
                MemorySlicing::Succeeded(value) => value,
                MemorySlicing::InvalidParameters() => fail(-2, b"Cannot slice buffer.\n"),
                MemorySlicing::OutOfRange() => fail(-2, b"Cannot slice buffer.d\n"),
            };

            index += match file_write(&mut target, &write) {
                FileWriting::Failed(_) => fail(-2, b"Cannot write to stdout.\n"),
                FileWriting::Succeeded(value) => value,
            };
        }
    }

    if let MemoryDeallocation::Failed(_) = mem_free(buffer) {
        fail(-2, b"Cannot free memory.\n");
    }

    if let FileClosing::Failed(_) = file_close(source) {
        fail(-2, b"Cannot close source file descriptor.\n")
    }

    sys_exit(0);
}

fn fail(status: i32, msg: &[u8]) -> ! {
    file_write(&mut stderr_open(), &MemorySlice::from(msg));
    sys_exit(status);
}

#[panic_handler]
fn panic(_panic: &core::panic::PanicInfo<'_>) -> ! {
    sys_exit(-1)
}
