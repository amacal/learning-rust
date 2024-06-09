use ::core::mem;

use super::erase::*;
use super::refs::*;
use crate::heap::*;
use crate::syscall::*;
use crate::thread::*;
use crate::trace::*;
use crate::uring::*;
use crate::pipe::*;

const WORKERS_COUNT: usize = 1;

pub struct IORuntimePool {
    workers_completers: [Option<u64>; WORKERS_COUNT],
    workers_array: [Option<Worker>; WORKERS_COUNT],
    workers_slots: [usize; WORKERS_COUNT],
    workers_count: usize,
    queue_incoming: u32,
    queue_outgoing: u32,
    queue_counter: usize,
}

pub enum IORuntimePoolAllocation {
    Succeeded(Boxed<IORuntimePool>),
    AllocationFailed(isize),
    ThreadingFailed(isize),
    QueueFailed(isize),
}

impl IORuntimePool {
    pub fn allocate() -> IORuntimePoolAllocation {
        let queue = match PipeChannel::create() {
            Ok(value) => value,
            Err(err) => return IORuntimePoolAllocation::QueueFailed(err),
        };

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

        let (incoming, outgoing) = queue.extract();

        instance.queue_counter = 0;
        instance.workers_count = 0;

        instance.queue_incoming = incoming;
        instance.queue_outgoing = outgoing;

        IORuntimePoolAllocation::Succeeded(instance)
    }
}

impl HeapLifetime for IORuntimePool {
    fn ctor(&mut self) {}

    fn dtor(&mut self) {
        sys_close(self.queue_incoming);
        sys_close(self.queue_outgoing);

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
        callable: &CallableTarget,
    ) -> IORuntimePoolExecute {
        // acquire worker
        if let Some(slot) = self.workers_slots.get(self.workers_count) {
            let worker = match self.workers_array.get_mut(*slot) {
                Some(Some(worker)) => worker,
                _ => return IORuntimePoolExecute::InternallyFailed(),
            };

            // confirm queuing
            let op = IORingSubmitEntry::noop();
            match submitter.submit(completers[0].encode(), [op]) {
                IORingSubmit::Succeeded(_) => (),
                _ => return IORuntimePoolExecute::ScheduleFailed(),
            }

            // prepare execute op
            let op = match worker.execute(callable) {
                WorkerExecute::Succeeded(op) => op,
                _ => return IORuntimePoolExecute::InternallyFailed(),
            };

            // confirm execution
            match submitter.submit(completers[1].encode(), [op]) {
                IORingSubmit::Succeeded(_) => (),
                _ => return IORuntimePoolExecute::ExecutionFailed(),
            }

            // update internal counter and correlate worker with completer
            self.workers_count += 1;
            self.workers_completers[*slot] = Some(completers[1].encode());

            return IORuntimePoolExecute::Executed();
        }

        // append encoded completer to a callable header
        let (ptr, len) = unsafe {
            let (ptr, _) = callable.as_ptr();
            let encoded = (ptr + 16) as *mut u64;

            *encoded = completers[1].encode();
            (ptr as *const u8, 24)
        };

        // notify when queuing happened
        let op = IORingSubmitEntry::write(self.queue_outgoing, ptr, len, 0);
        match submitter.submit(completers[0].encode(), [op]) {
            IORingSubmit::Succeeded(_) => (),
            _ => return IORuntimePoolExecute::ScheduleFailed(),
        }

        return IORuntimePoolExecute::Queued();
    }
}

impl IORuntimePool {
    pub fn queue(&mut self, completer: &IORingCompleterRef) {
        self.queue_counter += 1;
        trace2(b"worker queued; cid=%d, size=%d\n", completer.cid(), self.queue_counter);
    }
}

pub enum IORuntimePoolSubmit {
    Succeeded(),
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

        if self.queue_counter > 0 {
            if let Some(slot) = self.workers_slots.get(self.workers_count) {
                trace1(b"worker would be available; size=%d\n", self.queue_counter);

                let mut buffer: [u8; 24] = [0; 24];
                let ptr = buffer.as_mut_ptr() as *mut ();

                let result = sys_read(self.queue_incoming, ptr, 24);
                trace1(b"worker would be available; res=%d\n", result);
                self.queue_counter -= 1;

                let ptr = ptr as *const usize;
                let len = unsafe { ptr.add(1) };
                let encoded = unsafe { ptr.add(2) as *const u64 };

                let heap = unsafe { Heap::at(*ptr, *len) };
                let callable: CallableTarget = CallableTarget::from(heap);

                let worker = match self.workers_array.get_mut(*slot) {
                    Some(Some(worker)) => worker,
                    _ => return IORuntimePoolSubmit::InternallyFailed(),
                };

                let op = match worker.execute(&callable) {
                    WorkerExecute::Succeeded(op) => op,
                    _ => return IORuntimePoolSubmit::InternallyFailed(),
                };

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
