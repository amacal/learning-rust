use core::arch::asm;

pub fn sys_read(fd: u32, buf: *const u8, count: usize) -> isize {
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

pub fn sys_write(fd: u32, buf: *const u8, count: usize) -> isize {
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

pub fn sys_mmap(
    addr: *mut u8,
    length: usize,
    prot: usize,
    flags: usize,
    fd: usize,
    offset: usize,
) -> isize {
    unsafe {
        let ret: isize;

        asm!(
            "syscall",
            in("rax") 9,
            in("rdi") addr,
            in("rsi") length,
            in("rdx") prot,
            in("r10") flags,
            in("r8") fd,
            in("r9") offset,
            lateout("rcx") _,
            lateout("r11") _,
            lateout("rax") ret,
            options(nostack)
        );

        ret
    }
}

pub fn sys_munmap(addr: *mut u8, length: usize) -> isize {
    unsafe {
        let ret: isize;

        asm!(
            "syscall",
            in("rax") 11,
            in("rdi") addr,
            in("rsi") length,
            lateout("rcx") _,
            lateout("r11") _,
            lateout("rax") ret,
            options(nostack)
        );

        ret
    }
}

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
