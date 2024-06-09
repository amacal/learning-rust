#![no_std]
#![no_main]

#![feature(fn_traits)]
#![feature(waker_getters)]

mod commands;
mod core;
mod heap;
mod kernel;
mod pipe;
mod proc;
mod runtime;
mod sha1;
mod syscall;
mod thread;
mod trace;
mod uring;

use ::core::panic;

use crate::commands::*;
use crate::proc::*;
use crate::runtime::*;
use crate::syscall::*;
use crate::trace::*;

#[no_mangle]
extern "C" fn main(args: &'static ProcessArguments) -> ! {
    let mut runtime = match IORingRuntime::allocate() {
        IORingRuntimeAllocate::Succeeded(runtime) => runtime,
        IORingRuntimeAllocate::RingAllocationFailed() => fail(-2, b"I/O Runtime: Ring Allocation Failed.\n"),
        IORingRuntimeAllocate::RegistryAllocationFailed() => fail(-2, b"I/O Runtime: Registry Allocation Failed.\n"),
        IORingRuntimeAllocate::PoolAllocationFailed() => fail(-2, b"I/O Runtime: Pool Allocation Failed.\n"),
    };

    let commands: [&'static [u8]; 9] = [b"cat", b"faster", b"hello", b"pipe", b"sha1sum", b"spawn", b"sync", b"thread", b"tick"];

    let result = match args.select(1, commands) {
        Some(b"cat") => runtime.run(CatCommand { args: args }.execute()),
        Some(b"faster") => runtime.run(FasterCommand { args: args, delay: 4 }.execute()),
        Some(b"hello") => runtime.run(HelloCommand { msg: b"Hello, World!\n" }.execute()),
        Some(b"pipe") => runtime.run(PipeCommand { msg: b"Hello, World!\n" }.execute()),
        Some(b"sha1sum") => runtime.run(Sha1Command { args: args }.execute()),
        Some(b"spawn") => runtime.run(SpawnCommand { times: 30, delay: 3 }.execute()),
        Some(b"sync") => runtime.run(SyncCommand { msg: b"Hello, World!\n" }.execute()),
        Some(b"thread") => runtime.run(ThreadCommand { ios: 100, cpus: 1000 }.execute()),
        Some(b"tick") => runtime.run(TickCommand { ticks: 2, delay: 1 }.execute()),
        _ => fail(-2, b"I/O Runtime: Unrecognized command.\n"),
    };

    match result {
        IORingRuntimeRun::Completed(_) => (),
        _ => fail(-2, b"I/O Runtime: Run Failed.\n"),
    }

    match runtime.shutdown() {
        IORingRuntimeShutdown::Succeeded() => (),
        IORingRuntimeShutdown::ShutdownFailed() => fail(-2, b"I/O Runtime: Shutdown Failed.\n"),
        IORingRuntimeShutdown::ConsolidationFailed() => fail(-2, b"I/O Runtime: Consolidation Failed.\n"),
    }

    trace0(b"exit 0\n");
    sys_exit(0);
}

#[inline(never)]
fn fail(status: i32, msg: &'static [u8]) -> ! {
    sys_write(2, msg.as_ptr() as *const (), msg.len());
    sys_exit(status);
}

#[inline(never)]
#[panic_handler]
fn panic(_panic: &panic::PanicInfo<'_>) -> ! {
    sys_exit(-1)
}
