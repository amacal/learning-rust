use ::core::arch::*;

use crate::kernel::*;

#[allow(dead_code)]
#[inline(never)]
pub fn sys_read(fd: u32, buf: *const (), count: usize) -> isize {
    unsafe {
        let ret: isize;

        asm!(
            "syscall",
            in("rax") 0,
            in("rdi") fd,
            in("rsi") buf,
            in("rdx") count,
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
pub fn sys_write(fd: u32, buf: *const (), count: usize) -> isize {
    unsafe {
        let ret: isize;

        asm!(
            "syscall",
            in("rax") 1,
            in("rdi") fd,
            in("rsi") buf,
            in("rdx") count,
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
pub fn sys_open(pathname: *const u8, flags: i32, mode: u16) -> isize {
    unsafe {
        let ret: isize;

        asm!(
            "syscall",
            in("rax") 2,
            in("rdi") pathname,
            in("rsi") flags,
            in("rdx") mode,
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
pub fn sys_nanosleep(timespec: *const timespec) -> isize {
    unsafe {
        let ret: isize;

        asm!(
            "syscall",
            in("rax") 35,
            in("rdi") timespec,
            in("rsi") 0,
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
pub fn sys_exit(status: i32) -> ! {
    unsafe {
        asm!(
            "syscall",
            in("rax") 60,
            in("rdi") status,
            options(nostack, noreturn)
        )
    }
}

#[allow(dead_code)]
#[inline(never)]
pub fn sys_wait4(pid: u32, stats: *mut u32, options: u32, rusage: *mut ()) -> isize {
    unsafe {
        let ret: isize;

        asm!(
            "syscall",
            in("rax") 61,
            in("rdi") pid,
            in("rsi") stats,
            in("rdx") options,
            in("r10") rusage,
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
pub fn sys_fcntl(fd: u32, cmd: u32, arg: u64) -> isize {
    unsafe {
        let ret: isize;

        asm!(
            "syscall",
            in("rax") 72,
            in("rdi") fd,
            in("rsi") cmd,
            in("rdx") arg,
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
pub fn sys_waitid(which: i32, pid: u32, info: *mut siginfo, options: u32, rusage: *mut ()) -> isize {
    unsafe {
        let ret: isize;

        asm!(
            "syscall",
            in("rax") 247,
            in("rdi") which,
            in("rsi") pid,
            in("rdx") info,
            in("r10") options,
            in("r8") rusage,
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
pub fn sys_pipe2(pipefd: *mut u32, flags: u32) -> isize {
    unsafe {
        let ret: isize;

        asm!(
            "syscall",
            in("rax") 293,
            in("rdi") pipefd,
            in("rsi") flags,
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

#[allow(dead_code)]
#[inline(never)]
pub fn sys_pidfd_open(pid: u32, flags: u32) -> isize {
    unsafe {
        let ret;

        asm!(
            "syscall",
            in("rax") 434,
            in("rdi") pid,
            in("rsi") flags,
            lateout("rcx") _,
            lateout("r11") _,
            lateout("rax") ret,
            options(nostack)
        );

        ret
    }
}
