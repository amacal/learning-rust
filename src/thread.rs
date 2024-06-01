use ::core::arch;
use ::core::mem;

use crate::heap::*;
use crate::kernel::*;
use crate::syscall::*;
use crate::trace::*;
use crate::uring::*;

use crate::CallableTarget;

arch::global_asm!(
    "
    .global _start_thread;
    .global _stop_thread;

    _start_thread:
        push rdi;           // flags
        sub rsi, 16;        // stack
        mov [rsi], rcx;     // seed
        mov [rsi + 8], rdx; // func
        mov rax, 56;
        syscall;
        pop rdi;
        ret

    _stop_thread:
        mov rax, 11;
        syscall;
        mov rax, 60;
        syscall;
"
);

extern "C" {
    fn _start_thread(flags: u64, stack: *mut (), func: extern "C" fn(&WorkerArgs) -> !, seed: u64) -> isize;
    fn _stop_thread(heap_ptr: usize, heap_len: usize) -> !;
}

unsafe fn start_thread(heap: &Heap, func: extern "C" fn(&WorkerArgs) -> !, args: WorkerArgs) -> isize {
    // preparing flags to clone as thread
    let flags = CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND | CLONE_THREAD;

    // pointing at the end of the created stack
    let size = mem::size_of::<WorkerArgs>();
    let stack = (heap.ptr as *mut u8).add(heap.len - size);

    *(stack as *mut WorkerArgs) = args;

    // we don't care about handing negative results here
    let res = _start_thread(flags, stack as *mut (), func, stack as u64);
    trace1(b"starting thread; res=%d\n", res);

    return res;
}

pub struct Worker {
    incoming: u32,
    outgoing: u32,
}

#[repr(C)]
struct WorkerArgs {
    heap_ptr: usize,
    heap_len: usize,
    incoming: u32,
    outgoing: u32,
}

pub enum WorkerStart {
    Succeeded(Worker),
    StartFailed(isize),
    StackFailed(isize),
    PipesFailed(isize),
}

impl Worker {
    pub fn start() -> WorkerStart {
        let mut pipefd = [0; 4];
        let ptr = pipefd.as_mut_ptr();

        fn release_pipes(result: isize, pipefd: [u32; 4]) -> WorkerStart {
            for fd in pipefd {
                if fd > 0 {
                    sys_close(fd);
                }
            }

            WorkerStart::PipesFailed(result)
        }

        match sys_pipe2(unsafe { ptr.add(0) }, O_DIRECT) {
            result if result < 0 => return release_pipes(result, pipefd),
            _ => (),
        }

        match sys_pipe2(unsafe { ptr.add(2) }, O_DIRECT) {
            result if result < 0 => return release_pipes(result, pipefd),
            _ => (),
        }

        // we need to have a stack
        let mut heap = match mem_alloc(4096) {
            MemoryAllocation::Succeeded(heap) => heap,
            MemoryAllocation::Failed(err) => {
                release_pipes(err, pipefd);
                return WorkerStart::StackFailed(err);
            }
        };

        // a seed will be passed directly to newly created thread
        // and must contains incoming and outgoing pipes
        let args = WorkerArgs {
            heap_ptr: heap.ptr,
            heap_len: heap.len,
            incoming: pipefd[0],
            outgoing: pipefd[3],
        };

        // now we can start a thread
        let tid = match unsafe { start_thread(&heap, worker_callback, args) } {
            result if result > 0 => result as u32,
            result => {
                mem_free(&mut heap);
                release_pipes(result, pipefd);

                return WorkerStart::StartFailed(result);
            }
        };

        trace4(
            b"worker spawned; tid=%d, heap=%x, in=%d, out=%d\n",
            tid,
            heap.ptr,
            pipefd[3],
            pipefd[1],
        );

        let worker = Worker {
            incoming: pipefd[2],
            outgoing: pipefd[1],
        };

        WorkerStart::Succeeded(worker)
    }

    pub fn release(self) {
        sys_close(self.incoming);
        sys_close(self.outgoing);
    }

    pub fn execute(&mut self, callable: &CallableTarget<24>) -> IORingSubmitEntry<*const u8> {
        let heap = callable.heap();
        let ptr = heap.ptr;

        unsafe { *(ptr as *mut usize) = heap.ptr };
        unsafe { *(ptr as *mut usize).add(1) = heap.len };

        trace2(b"worker sends bytes; addr=%x, size=%d\n", heap.ptr, heap.len);
        let res = sys_write(self.outgoing, ptr as *mut (), 16);

        trace3(b"worker sends bytes; fd=%d, size=%d, res=%d\n", self.outgoing, 16, res);

        let slice = match heap.between(16, 17) {
            HeapSlicing::Succeeded(slice) => slice,
            HeapSlicing::InvalidParameters() => todo!(),
            HeapSlicing::OutOfRange() => todo!(),
        };

        IORingSubmitEntry::read(self.incoming, slice.ptr as *const u8, slice.len, 0)
    }
}

extern "C" fn worker_callback(args: &WorkerArgs) -> ! {
    let mut buffer: [u8; 16] = [0; 16];
    let ptr = buffer.as_mut_ptr() as *mut ();

    loop {
        let received = sys_read(args.incoming, ptr, buffer.len());
        trace2(b"worker received bytes; fd=%d, size=%d\n", args.incoming, received);

        if received == 0 {
            break;
        }

        let heap_ptr = unsafe { *(ptr as *const usize) };
        let heap_len = unsafe { *(ptr as *const usize).add(1) };

        trace2(b"worker received bytes; addr=%x, size=%d\n", heap_ptr, heap_len);

        let heap = Heap::at(heap_ptr, heap_len);
        let mut target: CallableTarget<24> = CallableTarget::from(heap);

        let res = target.call();
        trace0(b"worker called target; successfully\n");

        let res = sys_write(args.outgoing, ptr, 1);
        trace1(b"worker completed; res=%d\n", res);
    }

    let res = sys_close(args.incoming);
    trace2(b"terminating thread; in=%d, res=%d\n", args.incoming, res);

    let res = sys_close(args.outgoing);
    trace2(b"terminating thread; out=%d, res=%d\n", args.outgoing, res);

    trace2(b"terminating thread; heap=%x, len=%d\n", args.heap_ptr, args.heap_len);
    unsafe { _stop_thread(args.heap_ptr, args.heap_len) }
}
