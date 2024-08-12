use ::core::arch;
use ::core::mem;

use super::callable::*;
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
        push rdi;           // flags in parent, seed in child
        sub rsi, 24;        // stack initially aligned to 16 needs to be aligned to modulo 8
                            // so that it's aligned to 16 after calling ret
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
    fn _start_thread(flags: u64, stack: *mut (), func: extern "C" fn(&mut WorkerArgs) -> !, seed: usize) -> isize;
    fn _stop_thread(stack_ptr: usize, stack_len: usize) -> !;
}

unsafe fn start_thread(heap: &Heap, func: extern "C" fn(&mut WorkerArgs) -> !, args: WorkerArgs) -> isize {
    // preparing flags to clone as thread
    let flags = CLONE_VM | CLONE_FS | CLONE_FILES | CLONE_SIGHAND | CLONE_THREAD;

    // pointing at the end of the created stack
    let size = mem::size_of::<WorkerArgs>();
    let stack = (heap.as_ref().ptr() as *mut u8).add(heap.as_ref().len() - size);

    // copy worker args on new stack
    *(stack as *mut WorkerArgs) = args;

    // we don't care about handing negative results here
    let res = _start_thread(flags, stack as *mut (), func, stack as usize);
    trace3(b"starting thread; res=%d, args=%x, size=%d\n", res, stack as *mut WorkerArgs as *const u8, size);

    return res;
}

pub struct Worker {
    incoming: u32,
    outgoing: u32,
}

#[repr(C, align(16))]
pub struct WorkerArgs {
    stack_ptr: usize,
    stack_len: usize,
    incoming: u32,
    outgoing: u32,
}

pub enum WorkerStart {
    Succeeded(Worker),
    StartFailed(Option<i32>),
    StackFailed(Option<i32>),
    PipesFailed(Option<i32>),
}

impl Worker {
    pub fn start() -> WorkerStart {
        let mut pipefd = [0; 4];
        let ptr = pipefd.as_mut_ptr();

        fn release_pipes(result: Option<i32>, pipefd: [u32; 4]) -> WorkerStart {
            for fd in pipefd {
                if fd > 0 {
                    sys_close(fd);
                }
            }

            WorkerStart::PipesFailed(result)
        }

        match sys_pipe2(unsafe { ptr.add(0) }, O_DIRECT) {
            result if result < 0 => match i32::try_from(result) {
                Ok(value) => return release_pipes(Some(value), pipefd),
                Err(_) => return release_pipes(None, pipefd),
            },
            _ => (),
        }

        match sys_pipe2(unsafe { ptr.add(2) }, O_DIRECT) {
            result if result < 0 => match i32::try_from(result) {
                Ok(value) => return release_pipes(Some(value), pipefd),
                Err(_) => return release_pipes(None, pipefd),
            },
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
                if let Err(_) = heap.free() {
                    trace0(b"releasing stack for a thread failed\n");
                }

                let value = match i32::try_from(result) {
                    Ok(value) => Some(value),
                    Err(_) => None,
                };

                release_pipes(None, pipefd);
                return WorkerStart::StartFailed(value);
            }
        };

        trace4(b"worker spawned; tid=%d, heap=%x, in=%d, out=%d\n", tid, heap.as_ref().ptr(), pipefd[3], pipefd[1]);

        let mut buffer: [u8; 1] = [0; 1];
        sys_read(pipefd[2], buffer.as_mut_ptr() as *const (), 1);

        let worker = Worker {
            incoming: pipefd[2],
            outgoing: pipefd[1],
        };

        WorkerStart::Succeeded(worker)
    }

    pub fn release(&mut self) {
        // it will unblock sys_read in the worker
        sys_close(self.outgoing);

        // we need to understand what happened
        let mut buffer: [u8; 1] = [0; 1];
        let res = sys_read(self.incoming, buffer.as_mut_ptr() as *const (), 1);
        trace3(b"parent received notification; val=%d, fd=%d, res=%d\n", buffer[0], self.incoming, res);

        // to finally close the channel
        sys_close(self.incoming);
    }
}

pub enum WorkerExecute {
    Succeeded(IORingSubmitEntry),
    OutgoingPipeFailed(Option<i32>),
}

impl Worker {
    pub fn execute(&mut self, callable: &CallableTarget) -> WorkerExecute {
        let (ptr, len) = (callable.as_ref().ptr(), 16);

        // we expect here to not have any blocking operation because worker waits for it
        trace2(b"worker sends bytes; ptr=%x, len=%d\n", ptr, len);
        let res = sys_write(self.outgoing, ptr as *mut (), len);

        match res {
            value if value < 0 => match i32::try_from(value) {
                Ok(value) => return WorkerExecute::OutgoingPipeFailed(Some(value)),
                Err(_) => return WorkerExecute::OutgoingPipeFailed(None),
            },
            value if value as usize == len => {
                trace3(b"worker sends bytes; fd=%d, size=%d, res=%d\n", self.outgoing, len, res)
            }
            _ => return WorkerExecute::OutgoingPipeFailed(None),
        }

        // asynchronous operation has to be returned referencing callable's header
        WorkerExecute::Succeeded(IORingSubmitEntry::read(self.incoming, (ptr + 16) as *const u8, 1, 0))
    }
}

extern "C" fn worker_callback(args: &mut WorkerArgs) -> ! {
    let mut buffer: [usize; 2] = [0; 2];
    let ptr = buffer.as_mut_ptr() as *mut ();

    let res = sys_write(args.outgoing, [0x01u8].as_ptr() as *const (), 1);
    trace2(b"worker sent notification; val=1, fd=%d, res=%d\n", args.outgoing, res);

    loop {
        // read 16-bytes from the main thread
        let received = sys_read(args.incoming, ptr, 16);
        trace2(b"worker received bytes; fd=%d, size=%d\n", args.incoming, received);

        if received != 16 {
            trace0(b"worker leaves infinite loop...\n");
            break;
        }

        trace2(b"worker received bytes; addr=%x, size=%d\n", buffer[0], buffer[1]);

        let heap = Heap::at(buffer[0], buffer[1]);
        let mut target: CallableTarget = CallableTarget::from(heap);

        // calling the function behind the heap
        match target.call() {
            Ok(()) => trace0(b"worker called target; successfully\n"),
            Err(CallableError::CalledTwice) => trace0(b"worker called target; failed, called twice\n"),
            Err(_) => trace0(b"worker called target; failed\n"),
        }

        // reporting one byte
        let res = sys_write(args.outgoing, [0x03u8].as_ptr() as *const (), 1);
        trace2(b"worker sent notification; val=3, fd=%d, res=%d\n", args.outgoing, res);
    }

    let res = sys_write(args.outgoing, [0x02u8].as_ptr() as *const (), 1);
    trace2(b"worker sent notification; val=2, fd=%d, res=%d\n", args.outgoing, res);

    let res = sys_close(args.incoming);
    trace2(b"terminating thread; in=%d, res=%d\n", args.incoming, res);

    let res = sys_close(args.outgoing);
    trace2(b"terminating thread; out=%d, res=%d\n", args.outgoing, res);

    // releasing stack memory and exiting current thread
    trace2(b"terminating thread; heap=%x, len=%d\n", args.stack_ptr, args.stack_len);
    unsafe { _stop_thread(args.stack_ptr, args.stack_len) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_and_releases_worker() {
        let mut worker = match Worker::start() {
            WorkerStart::Succeeded(worker) => worker,
            _ => return assert!(false),
        };

        worker.release();
    }

    #[test]
    fn executes_callable() {
        fn release_worker(worker: &mut Worker) {
            worker.release();
        }

        let mut worker = match Worker::start() {
            WorkerStart::Succeeded(worker) => Droplet::from(worker, release_worker),
            _ => return assert!(false),
        };

        let mut pool = HeapPool::<1>::new();
        let target = || -> Result<u8, ()> { Ok(13) };

        let callable = match CallableTarget::allocate(&mut pool, target) {
            Ok(target) => target,
            Err(_) => return assert!(false),
        };

        let entry = match worker.execute(&callable) {
            WorkerExecute::Succeeded(entry) => entry,
            _ => return assert!(false),
        };

        let (ptr, read) = match entry {
            IORingSubmitEntry::Read(entry) => (entry.buf, entry),
            _ => return assert!(false),
        };

        unsafe {
            assert_eq!(read.fd, worker.incoming);
            assert_eq!(read.buf as usize / 4096, callable.as_ref().ptr() / 4096);
            assert_ne!(*ptr, 3);
        }

        let mut ring = match IORing::init(8) {
            Ok(ring) => ring.droplet(),
            _ => return assert!(false),
        };

        match ring.tx.submit(1, [IORingSubmitEntry::Read(read)]) {
            IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
            _ => return assert!(false),
        }

        match ring.tx.flush() {
            IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
            _ => return assert!(false),
        }

        let mut entries = [IORingCompleteEntry::default(); 1];
        match ring.rx.complete(&mut entries) {
            IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
            _ => return assert!(false),
        }

        unsafe {
            assert_eq!(entries[0].res, 1);
            assert_eq!(*ptr, 3);
        }
    }
}
