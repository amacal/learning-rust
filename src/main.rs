#![no_std]
#![no_main]

mod syscalls;

use crate::syscalls::*;
use core::panic::PanicInfo;

#[no_mangle]
extern "C" fn _start() -> ! {
    let msg = b"Hello, world!\n";

    sys_write(1, msg.as_ptr(), msg.len());
    sys_exit(0);
}

#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    sys_exit(-1);
}
