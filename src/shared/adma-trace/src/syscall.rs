use ::core::arch::*;

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
