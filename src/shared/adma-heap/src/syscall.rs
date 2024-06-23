use ::core::arch::*;

#[allow(dead_code)]
#[inline(never)]
pub fn sys_mmap(len: usize, prot: usize, flags: usize) -> isize {
    unsafe {
        let ret: isize;

        asm!(
            "syscall",
            in("rax") 9,
            in("rdi") 0,
            in("rsi") len,
            in("rdx") prot,
            in("r10") flags,
            in("r8") 0,
            in("r9") 0,
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
pub fn sys_munmap(addr: usize, len: usize) -> isize {
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
