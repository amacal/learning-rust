use ::core::arch::*;
use crate::kernel::*;

#[allow(dead_code)]
#[inline(never)]
pub fn sys_close(fd: u32) -> isize {
    unsafe {
        let ret: isize;

        asm!(
            "syscall",
            in("rax") 3,
            in("rdi") fd,
            lateout("rcx") _,
            lateout("r11") _,
            lateout("rax") ret,
            options(nostack)
        );

        ret
    }
}

#[allow(dead_code)]
#[inline(never)]
pub fn sys_mmap(addr: *mut u8, len: usize, prot: usize, flags: usize, fd: usize, off: usize) -> isize {
    unsafe {
        let ret: isize;

        asm!(
            "syscall",
            in("rax") 9,
            in("rdi") addr,
            in("rsi") len,
            in("rdx") prot,
            in("r10") flags,
            in("r8") fd,
            in("r9") off,
            lateout("rcx") _,
            lateout("r11") _,
            lateout("rax") ret,
            options(nostack)
        );

        ret
    }
}

#[allow(dead_code)]
#[inline(never)]
pub fn sys_munmap(addr: *mut (), len: usize) -> isize {
    unsafe {
        let ret: isize;

        asm!(
            "syscall",
            in("rax") 11,
            in("rdi") addr,
            in("rsi") len,
            lateout("rcx") _,
            lateout("r11") _,
            lateout("rax") ret,
            options(nostack)
        );

        ret
    }
}

#[allow(dead_code)]
#[inline(never)]
pub fn sys_io_uring_setup(entries: u32, params: *mut io_uring_params) -> isize {
    unsafe {
        let ret;

        asm!(
            "syscall",
            in("rax") 425,
            in("rdi") entries,
            in("rsi") params,
            lateout("rcx") _,
            lateout("r11") _,
            lateout("rax") ret,
            options(nostack)
        );

        ret
    }
}

#[allow(dead_code)]
#[inline(never)]
pub fn sys_io_uring_enter(fd: u32, to_submit: u32, min_complete: u32, flags: u32, argp: *const u8, args: u32) -> isize {
    unsafe {
        let ret;

        asm!(
            "syscall",
            in("rax") 426,
            in("rdi") fd,
            in("rsi") to_submit,
            in("rdx") min_complete,
            in("r10") flags,
            in("r8") argp,
            in("r9") args,
            lateout("rcx") _,
            lateout("r11") _,
            lateout("rax") ret,
            options(nostack)
        );

        ret
    }
}
