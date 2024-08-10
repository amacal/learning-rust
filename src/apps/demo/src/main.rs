#![no_std]
#![no_main]

mod commands;
mod start;

use adma_io::proc::*;
use adma_io::runtime::*;
use adma_io::syscall::*;
use adma_io::trace::*;

use crate::commands::*;

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
        Some(b"cat") => runtime.run(|ops| CatCommand { args: args }.execute(ops)),
        Some(b"faster") => runtime.run(|ops| FasterCommand { args: args, delay: 4 }.execute(ops)),
        Some(b"hello") => runtime.run(|ops| HelloCommand { msg: b"Hello, World!\n" }.execute(ops)),
        Some(b"pipe") => runtime.run(|ops| PipeCommand { msg: b"Hello, World!\n" }.execute(ops)),
        Some(b"sha1sum") => runtime.run(|ops| Sha1Command { args: args }.execute(ops)),
        Some(b"spawn") => runtime.run(|ops| SpawnCommand { times: 30, delay: 3 }.execute(ops)),
        Some(b"sync") => runtime.run(|_| SyncCommand { msg: b"Hello, World!\n" }.execute()),
        Some(b"thread") => runtime.run(|ops| ThreadCommand { ios: 100, cpus: 100 }.execute(ops)),
        Some(b"tick") => runtime.run(|ops| TickCommand { ticks: 30, delay: 1 }.execute(ops)),
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
#[cfg(not(test))]
#[panic_handler]
fn panic(_panic: &::core::panic::PanicInfo<'_>) -> ! {
   sys_exit(-1)
}
