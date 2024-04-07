#![no_std]
#![no_main]

mod cat;
mod hello;
mod kernel;
mod linux;
mod syscall;
mod system;
mod uring;

use cat::{CatCommand, CatCommandExecute};
use hello::{HelloCommand, HelloCommandExecute};
use syscall::sys_write;

use crate::syscall::sys_exit;
use crate::system::ProcessArguments;

#[no_mangle]
extern "C" fn main(args: &ProcessArguments) -> ! {
    let src = match args.get(1) {
        None => fail(-2, b"Cannot find path in the args.\n"),
        Some(value) => value,
    };

    let hello = HelloCommand { msg: b"Hello, World!\n" };
    let cat = CatCommand { src: src };

    match cat.execute() {
        CatCommandExecute::Succeeded() => sys_exit(0),
        CatCommandExecute::Failed(msg) => fail(-1, msg),
    }
}

fn fail(status: i32, msg: &'static [u8]) -> ! {
    sys_write(2, msg.as_ptr(), msg.len());
    sys_exit(status);
}

#[panic_handler]
fn panic(_panic: &core::panic::PanicInfo<'_>) -> ! {
    sys_exit(-1)
}
