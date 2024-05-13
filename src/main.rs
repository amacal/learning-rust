#![no_std]
#![no_main]
#![feature(waker_getters)]

mod commands;
mod heap;
mod kernel;
mod proc;
mod runtime;
mod syscall;
mod trace;
mod uring;

use crate::commands::*;
use crate::proc::*;
use crate::runtime::*;
use crate::syscall::*;
use crate::trace::*;
use crate::uring::*;

#[no_mangle]
extern "C" fn main(args: &'static ProcessArguments) -> ! {
    let (submitter, completer) = match IORing::init(32) {
        IORingInit::Succeeded(submitter, completer) => (submitter, completer),
        IORingInit::InvalidDescriptor(_) => fail(-2, b"I/O Ring: Invalid Descriptor.\n"),
        IORingInit::SetupFailed(_) => fail(-2, b"I/O Ring: Setup Failed.\n"),
        IORingInit::MappingFailed(_, _) => fail(-2, b"I/O Ring: Mapping Failed.\n"),
    };

    let mut runtime = match IORingRuntime::create(submitter, completer) {
        IORingRuntimeCreate::Succeeded(runtime) => runtime,
        IORingRuntimeCreate::HeapFailed(_) => fail(-2, b"I/O Runtime: Allocation Failed.\n"),
    };

    let commands: [&'static [u8]; 6] = [b"cat", b"faster", b"hello", b"spawn", b"sync", b"tick"];
    let result = match args.select(1, commands) {
        Some(b"cat") => runtime.run(CatCommand { args: args }.execute()),
        Some(b"faster") => runtime.run(FasterCommand { args: args, delay: 4 }.execute()),
        Some(b"hello") => runtime.run(HelloCommand { msg: b"Hello, World!\n" }.execute()),
        Some(b"spawn") => runtime.run(SpawnCommand { times: 30, delay: 3 }.execute()),
        Some(b"sync") => runtime.run(SyncCommand { msg: b"Hello, World!\n" }.execute()),
        Some(b"tick") => runtime.run(TickCommand { ticks: 2, delay: 1 }.execute()),
        _ => fail(-2, b"I/O Runtime: Unrecognized Command.\n"),
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
    sys_write(2, msg.as_ptr(), msg.len());
    sys_exit(status);
}

#[inline(never)]
#[panic_handler]
fn panic(_panic: &core::panic::PanicInfo<'_>) -> ! {
    sys_exit(-1)
}
