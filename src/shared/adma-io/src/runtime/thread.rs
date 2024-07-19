use ::core::arch;
use ::core::mem;

use super::erase::*;
use crate::heap::*;
use crate::kernel::*;
use crate::syscall::*;
use crate::trace::*;
use crate::uring::*;

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
    fn _stop_thread(stack_ptr: usize, stack_len: usize) -> !;
}

unsafe fn start_thread(heap: &Heap, func: extern "C" fn(&WorkerArgs) -> !, args: WorkerArgs) -> isize {
    // preparing flags to clone as thread
    let flags = CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND | CLONE_THREAD;

    // pointing at the end of the created stack
    let size = mem::size_of::<WorkerArgs>();
    let stack = (heap.as_ref().ptr() as *mut u8).add(heap.as_ref().len() - size);

    // copy worker args on new stack
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
    stack_ptr: usize,
    stack_len: usize,
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
        let heap = match Heap::allocate(4096) {
            Ok(heap) => heap,
            Err(err) => {
                release_pipes(err, pipefd);
                return WorkerStart::StackFailed(err);
            }
        };

        // args will be passed directly to newly created thread
        // and must contain incoming and outgoing pipes
        let args = WorkerArgs {
            stack_ptr: heap.as_ref().ptr(),
            stack_len: heap.as_ref().len(),
            incoming: pipefd[0],
            outgoing: pipefd[3],
        };

        // now we can start a thread
        let tid = match unsafe { start_thread(&heap, worker_callback, args) } {
            result if result > 0 => result as u32,
            result => {
                heap.free();
                release_pipes(result, pipefd);

                return WorkerStart::StartFailed(result);
            }
        };

        trace4(
            b"worker spawned; tid=%d, heap=%x, in=%d, out=%d\n",
            tid,
            heap.as_ref().ptr(),
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
}

pub enum WorkerExecute {
    Succeeded(IORingSubmitEntry),
    OutgoingPipeFailed(isize),
}

impl Worker {
    pub fn execute(&mut self, callable: &CallableTarget) -> WorkerExecute {
        let (ptr, len) = (callable.as_ref().ptr(), 16);

        // we expect here to not have any blocking operation because worker waits for it
        trace2(b"worker sends bytes; ptr=%x, len=%d\n", ptr, len);
        let res = sys_write(self.outgoing, ptr as *mut (), len);

        // we sends exactly 16 bytes, containing (ptr, len) of the heap
        trace3(b"worker sends bytes; fd=%d, size=%d, res=%d\n", self.outgoing, len, res);
        if res != len as isize {
            return WorkerExecute::OutgoingPipeFailed(res);
        }

        // asynchronous operation has to be returned referencing callable's header
        WorkerExecute::Succeeded(IORingSubmitEntry::read(self.incoming, (ptr + 16) as *const u8, 1, 0))
    }
}

extern "C" fn worker_callback(args: &WorkerArgs) -> ! {
    let mut buffer: [usize; 2] = [0; 2];
    let ptr = buffer.as_mut_ptr() as *mut ();

    loop {
        // read 16-bytes from the main thread
        let received = sys_read(args.incoming, ptr, 16);
        trace2(b"worker received bytes; fd=%d, size=%d\n", args.incoming, received);

        if received != 16 {
            break;
        }

        trace2(b"worker received bytes; addr=%x, size=%d\n", buffer[0], buffer[1]);

        let heap = Heap::at(buffer[0], buffer[1]);
        let mut target: CallableTarget = CallableTarget::from(heap);

        // calling the function behind the heap
        match target.call() {
            None => trace0(b"worker called target; successfully\n"),
            Some(err) => trace1(b"worker called target; %s\n", err),
        }

        // reporting one byte
        let res = sys_write(args.outgoing, ptr, 1);
        trace1(b"worker completed; res=%d\n", res);
    }

    let res = sys_close(args.incoming);
    trace2(b"terminating thread; in=%d, res=%d\n", args.incoming, res);

    let res = sys_close(args.outgoing);
    trace2(b"terminating thread; out=%d, res=%d\n", args.outgoing, res);

    // releasing stack memory and exiting current thread
    trace2(b"terminating thread; heap=%x, len=%d\n", args.stack_ptr, args.stack_len);
    unsafe { _stop_thread(args.stack_ptr, args.stack_len) }
}
