use ::core::mem;

use super::erase::*;
use super::refs::*;
use crate::heap::*;
use crate::kernel::*;
use crate::syscall::*;
use crate::thread::*;
use crate::trace::*;
use crate::uring::*;

const WORKERS_COUNT: usize = 8;

pub struct IORuntimePool {
    workers_completers: [Option<u64>; WORKERS_COUNT],
    workers_array: [Option<Worker>; WORKERS_COUNT],
    workers_slots: [usize; WORKERS_COUNT],
    workers_count: usize,
    incoming: u32,
    outgoing: u32,
    queued: usize,
}

pub enum IORuntimePoolAllocation {
    Succeeded(Boxed<IORuntimePool>),
    AllocationFailed(isize),
    ThreadingFailed(isize),
}

impl IORuntimePool {
    pub fn allocate() -> IORuntimePoolAllocation {
        let mut pipefd = [0; 2];
        let ptr = pipefd.as_mut_ptr();

        let flags = O_DIRECT;
        let result = sys_pipe2(ptr, flags);

        let mut instance: Boxed<IORuntimePool> = match mem_alloc(mem::size_of::<IORuntimePool>()) {
            MemoryAllocation::Succeeded(heap) => heap.boxed(),
            MemoryAllocation::Failed(err) => return IORuntimePoolAllocation::AllocationFailed(err),
        };

        for i in 0..WORKERS_COUNT {
            let worker = match Worker::start() {
                WorkerStart::Succeeded(worker) => worker,
                WorkerStart::StartFailed(err) => return IORuntimePoolAllocation::ThreadingFailed(err),
                WorkerStart::PipesFailed(err) => return IORuntimePoolAllocation::ThreadingFailed(err),
                WorkerStart::StackFailed(err) => return IORuntimePoolAllocation::AllocationFailed(err),
            };

            instance.workers_array[i] = Some(worker);
            instance.workers_slots[i] = i;
        }

        instance.queued = 0;
        instance.workers_count = 0;

        instance.incoming = pipefd[0];
        instance.outgoing = pipefd[1];

        IORuntimePoolAllocation::Succeeded(instance)
    }
}

impl HeapLifetime for IORuntimePool {
    fn ctor(&mut self) {}

    fn dtor(&mut self) {
        sys_close(self.incoming);
        sys_close(self.outgoing);

        for i in 0..WORKERS_COUNT {
            if let Some(worker) = self.workers_array[i].take() {
                worker.release()
            }
        }
    }
}

pub enum IORuntimePoolExecute {
    Queued(),
    Executed(),
    ScheduleFailed(),
    ExecutionFailed(),
    InternallyFailed(),
}

impl IORuntimePool {
    pub fn execute(
        &mut self,
        submitter: &mut IORingSubmitter,
        completers: [&IORingCompleterRef; 2],
        callable: &CallableTarget<24>,
    ) -> IORuntimePoolExecute {
        if let Some(slot) = self.workers_slots.get(self.workers_count) {
            let worker = match self.workers_array.get_mut(*slot) {
                Some(Some(worker)) => worker,
                _ => return IORuntimePoolExecute::InternallyFailed(),
            };

            let op = IORingSubmitEntry::noop();
            match submitter.submit(completers[0].encode(), [op]) {
                IORingSubmit::Succeeded(_) => (),
                IORingSubmit::SubmissionFailed(_) => return IORuntimePoolExecute::ScheduleFailed(),
                IORingSubmit::SubmissionMismatched(_) => return IORuntimePoolExecute::ScheduleFailed(),
            }

            let op = worker.execute(callable);
            match submitter.submit(completers[1].encode(), [op]) {
                IORingSubmit::Succeeded(_) => (),
                IORingSubmit::SubmissionFailed(_) => return IORuntimePoolExecute::ExecutionFailed(),
                IORingSubmit::SubmissionMismatched(_) => return IORuntimePoolExecute::ExecutionFailed(),
            }

            self.workers_count += 1;
            self.workers_completers[*slot] = Some(completers[1].encode());

            return IORuntimePoolExecute::Executed();
        }

        trace0(b"worker is not available\n");

        let slice = match callable.heap().between(0, 24) {
            HeapSlicing::Succeeded(slice) => slice,
            HeapSlicing::InvalidParameters() => return IORuntimePoolExecute::ScheduleFailed(),
            HeapSlicing::OutOfRange() => return IORuntimePoolExecute::ScheduleFailed(),
        };

        unsafe {
            let ptr = slice.ptr as *mut usize;
            let encoded = ptr.add(2) as *mut u64;

            *ptr.add(0) = slice.ptr;
            *ptr.add(1) = slice.len;
            *encoded = completers[1].encode();
        }

        let op = IORingSubmitEntry::write(self.outgoing, slice, 0);
        match submitter.submit(completers[0].encode(), [op]) {
            IORingSubmit::Succeeded(_) => (),
            IORingSubmit::SubmissionFailed(_) => return IORuntimePoolExecute::ScheduleFailed(),
            IORingSubmit::SubmissionMismatched(_) => return IORuntimePoolExecute::ScheduleFailed(),
        }

        return IORuntimePoolExecute::Queued();
    }
}

impl IORuntimePool {
    pub fn queue(&mut self, completer: &IORingCompleterRef) {
        self.queued += 1;
        trace2(b"worker queued; cid=%d, size=%d\n", completer.cid(), self.queued);
    }
}

pub enum IORuntimePoolSubmit {
    Succeeded(),
    ScheduleFailed(),
    ExecutionFailed(),
    InternallyFailed(),
}

impl IORuntimePool {
    pub fn submit(&mut self, submitter: &mut IORingSubmitter, completer: &IORingCompleterRef) -> IORuntimePoolSubmit {
        trace1(b"returning worker; cid=%d\n", completer.cid());

        for slot in 0..WORKERS_COUNT {
            match self.workers_completers.get_mut(slot) {
                Some(Some(value)) if *value == completer.encode() => (),
                _ => continue,
            }

            self.workers_count -= 1;
            self.workers_completers[slot] = None;
            self.workers_slots[self.workers_count] = slot;

            trace1(b"returned worker; idx=%d\n", slot);
            break;
        }

        if self.queued > 0 {
            if let Some(slot) = self.workers_slots.get(self.workers_count) {
                trace1(b"worker would be available; size=%d\n", self.queued);

                let mut buffer: [u8; 24] = [0; 24];
                let ptr = buffer.as_mut_ptr() as *mut ();

                let result = sys_read(self.incoming, ptr, 24);
                trace1(b"worker would be available; res=%d\n", result);
                self.queued -= 1;

                let ptr = ptr as *const usize;
                let len = unsafe { ptr.add(1) };
                let encoded = unsafe { ptr.add(2) as *const u64 };

                let heap = unsafe { Heap::at(*ptr, *len) };
                let callable: CallableTarget<24> = CallableTarget::from(heap);

                let worker = match self.workers_array.get_mut(*slot) {
                    Some(Some(worker)) => worker,
                    _ => return IORuntimePoolSubmit::InternallyFailed(),
                };

                let op = worker.execute(&callable);
                match submitter.submit(unsafe { *encoded }, [op]) {
                    IORingSubmit::Succeeded(_) => (),
                    IORingSubmit::SubmissionFailed(_) => return IORuntimePoolSubmit::ExecutionFailed(),
                    IORingSubmit::SubmissionMismatched(_) => return IORuntimePoolSubmit::ExecutionFailed(),
                }

                self.workers_completers[*slot] = unsafe { Some(*encoded) };
                self.workers_count += 1;
            }
        }

        IORuntimePoolSubmit::Succeeded()
    }
}
