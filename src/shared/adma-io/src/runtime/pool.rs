use ::core::mem;

use super::erase::*;
use super::refs::*;
use super::thread::*;
use crate::heap::*;
use crate::pipe::*;
use crate::syscall::*;
use crate::trace::*;
use crate::uring::*;

pub struct IORuntimePool<const T: usize> {
    workers_completers: [Option<u64>; T],
    workers_array: [Option<Worker>; T],
    workers_slots: [usize; T],
    workers_count: usize,
    queue_incoming: u32,
    queue_outgoing: u32,
    queue_counter: usize,
}

pub enum IORuntimePoolAllocation<const T: usize> {
    Succeeded(Boxed<IORuntimePool<T>>),
    AllocationFailed(isize),
    ThreadingFailed(isize),
    QueueFailed(isize),
}

impl<const T: usize> IORuntimePool<T> {
    pub fn allocate() -> IORuntimePoolAllocation<T> {
        let queue = match PipeChannel::create() {
            Ok(value) => value,
            Err(err) => return IORuntimePoolAllocation::QueueFailed(err),
        };

        let mut instance: Boxed<IORuntimePool<T>> = match Heap::allocate(mem::size_of::<IORuntimePool<T>>()) {
            Ok(heap) => heap.boxed(),
            Err(err) => return IORuntimePoolAllocation::AllocationFailed(err),
        };

        for i in 0..T {
            let worker = match Worker::start() {
                WorkerStart::Succeeded(worker) => worker,
                WorkerStart::StartFailed(err) => return IORuntimePoolAllocation::ThreadingFailed(err),
                WorkerStart::PipesFailed(err) => return IORuntimePoolAllocation::ThreadingFailed(err),
                WorkerStart::StackFailed(err) => return IORuntimePoolAllocation::AllocationFailed(err),
            };

            instance.workers_array[i] = Some(worker);
            instance.workers_slots[i] = i;
        }

        // extract channel into its primitives pipes
        let (incoming, outgoing) = queue.extract();

        instance.queue_counter = 0;
        instance.workers_count = 0;

        instance.queue_incoming = incoming;
        instance.queue_outgoing = outgoing;

        IORuntimePoolAllocation::Succeeded(instance)
    }
}

impl<const T: usize> HeapLifetime for IORuntimePool<T> {
    fn ctor(&mut self) {}

    fn dtor(&mut self) {
        sys_close(self.queue_incoming);
        sys_close(self.queue_outgoing);

        for i in 0..T {
            if let Some(mut worker) = self.workers_array[i].take() {
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

impl<const T: usize> IORuntimePool<T> {
    pub fn execute(
        &mut self,
        submitter: &mut IORingSubmitter,
        completers: [&IORingCompleterRef; 2],
        callable: &CallableTarget,
    ) -> IORuntimePoolExecute {
        // acquire worker
        if let Some(slot) = self.workers_slots.get(self.workers_count) {
            trace1(b"acquired worker; slot=%d\n", *slot);

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
            let ptr = callable.as_ref().ptr();
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

impl<const T: usize> IORuntimePool<T> {
    pub fn enqueue(&mut self, completer: &IORingCompleterRef) {
        self.queue_counter += 1;
        trace1(b"callable queued; cid=%d\n", completer.cid());
    }
}

impl<const T: usize> IORuntimePool<T> {
    pub fn release_worker(&mut self, completer: &IORingCompleterRef) -> bool {
        trace1(b"releasing worker; cid=%d\n", completer.cid());

        for slot in 0..T {
            match self.workers_completers.get_mut(slot) {
                Some(Some(value)) if *value == completer.encode() => (),
                _ => continue,
            }

            self.workers_count -= 1;
            self.workers_completers[slot] = None;
            self.workers_slots[self.workers_count] = slot;

            trace2(b"releasing worker; cid=%d, idx=%d\n", completer.cid(), slot);
            return true;
        }

        false
    }
}

pub enum IORuntimePoolTrigger {
    Succeeded(bool),
    ExecutionFailed(),
    InternallyFailed(),
}

impl<const T: usize> IORuntimePool<T> {
    pub fn trigger(&mut self, submitter: &mut IORingSubmitter) -> IORuntimePoolTrigger {
        if self.queue_counter <= 0 {
            return IORuntimePoolTrigger::Succeeded(false);
        }

        if let Some(slot) = self.workers_slots.get(self.workers_count) {
            trace1(b"acquired worker; slot=%d\n", *slot);

            // worker still theoretically may fail
            let worker = match self.workers_array.get_mut(*slot) {
                Some(Some(worker)) => worker,
                _ => return IORuntimePoolTrigger::InternallyFailed(),
            };

            // buffer is needed to collect data from the pipe
            let mut buffer: [u8; 24] = [0; 24];
            let ptr = buffer.as_mut_ptr() as *mut ();

            // we expect to read ptr, len, encoded completer triple from a queue
            let result = sys_read(self.queue_incoming, ptr, 24);
            trace1(b"acquired callable; res=%d\n", result);

            if result != 24 {
                return IORuntimePoolTrigger::InternallyFailed();
            } else {
                self.queue_counter -= 1;
            }

            // decoding payload
            let ptr = ptr as *const usize;
            let len = unsafe { ptr.add(1) };
            let encoded = unsafe { ptr.add(2) as *const u64 };

            // rebuilding callable
            let heap = unsafe { Heap::at(*ptr, *len) };
            let callable: CallableTarget = CallableTarget::from(heap);

            // then we try to follow known path
            let op = match worker.execute(&callable) {
                WorkerExecute::Succeeded(op) => op,
                _ => return IORuntimePoolTrigger::InternallyFailed(),
            };

            // by registering it within I/O Ring
            match submitter.submit(unsafe { *encoded }, [op]) {
                IORingSubmit::Succeeded(_) => (),
                _ => return IORuntimePoolTrigger::ExecutionFailed(),
            }

            // not forgetting about maintaining the state
            self.workers_completers[*slot] = unsafe { Some(*encoded) };
            self.workers_count += 1;
        }

        IORuntimePoolTrigger::Succeeded(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_pool() {
        let pool = match IORuntimePool::<16>::allocate() {
            IORuntimePoolAllocation::Succeeded(pool) => pool,
            _ => return assert!(false),
        };

        assert_eq!(pool.queue_counter, 0);
        assert_eq!(pool.workers_count, 0);

        assert_ne!(pool.queue_incoming, 0);
        assert_ne!(pool.queue_outgoing, 0);

        drop(pool);
    }

    #[test]
    fn executes_callable_as_executed() {
        let mut heap = HeapPool::<1>::new();
        let target = || -> Result<u8, ()> { Ok(13) };

        fn execute<F>(heap: &mut HeapPool<1>, target: F)
        where
            F: FnOnce() -> Result<u8, ()> + Send,
        {
            let callable = match CallableTarget::allocate(heap, target) {
                CallableTargetAllocate::Succeeded(val) => val,
                CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
            };

            let (rx, mut tx) = match IORing::init(8) {
                IORingInit::Succeeded(tx, rx) => (rx, tx),
                _ => return assert!(false),
            };

            let mut pool = match IORuntimePool::<1>::allocate() {
                IORuntimePoolAllocation::Succeeded(pool) => pool,
                _ => return assert!(false),
            };

            let first = IORingCompleterRef::new(1, 2);
            let second = IORingCompleterRef::new(3, 4);

            match pool.execute(&mut tx, [&first, &second], &callable) {
                IORuntimePoolExecute::Executed() => assert!(true),
                _ => assert!(false),
            }

            match tx.flush() {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 2),
                _ => assert!(false),
            }

            let mut entries = [IORingCompleteEntry::default(); 1];
            match rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false)
            }

            assert_eq!(entries[0].res, 0);
            assert_eq!(entries[0].user_data, first.encode());

            let mut entries = [IORingCompleteEntry::default(); 1];
            match rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false)
            }

            assert_eq!(entries[0].res, 1);
            assert_eq!(entries[0].user_data, second.encode());

            match callable.result::<1, F, u8, ()>(heap) {
                Some(Ok(val)) => assert_eq!(val, 13),
                _ => assert!(false),
            }

            assert_eq!(pool.queue_counter, 0);
            assert_eq!(pool.workers_count, 1);

            drop(pool);
        }

        execute(&mut heap, target);
    }

    #[test]
    fn executes_callable_as_queued() {
        let mut heap = HeapPool::<1>::new();
        let target1 = || -> Result<u8, ()> { Ok(13) };
        let target2 = || -> Result<u8, ()> { Ok(17) };

        fn execute<F1, F2>(heap: &mut HeapPool<1>, target1: F1, target2: F2)
        where
            F1: FnOnce() -> Result<u8, ()> + Send,
            F2: FnOnce() -> Result<u8, ()> + Send,
        {
            let callable1 = match CallableTarget::allocate(heap, target1) {
                CallableTargetAllocate::Succeeded(val) => val,
                CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
            };

            let callable2 = match CallableTarget::allocate(heap, target2) {
                CallableTargetAllocate::Succeeded(val) => val,
                CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
            };

            let (rx, mut tx) = match IORing::init(8) {
                IORingInit::Succeeded(tx, rx) => (rx, tx),
                _ => return assert!(false),
            };

            let mut pool = match IORuntimePool::<1>::allocate() {
                IORuntimePoolAllocation::Succeeded(pool) => pool,
                _ => return assert!(false),
            };

            let first = IORingCompleterRef::new(1, 2);
            let second = IORingCompleterRef::new(3, 4);

            match pool.execute(&mut tx, [&first, &second], &callable1) {
                IORuntimePoolExecute::Executed() => assert!(true),
                _ => assert!(false),
            }

            match tx.flush() {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 2),
                _ => assert!(false),
            }

            let mut entries = [IORingCompleteEntry::default(); 1];
            match rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false)
            }

            assert_eq!(entries[0].res, 0);
            assert_eq!(entries[0].user_data, first.encode());

            let mut entries = [IORingCompleteEntry::default(); 1];
            match rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false)
            }

            assert_eq!(entries[0].res, 1);
            assert_eq!(entries[0].user_data, second.encode());

            match callable1.result::<1, F1, u8, ()>(heap) {
                Some(Ok(val)) => assert_eq!(val, 13),
                _ => assert!(false),
            }

            assert_eq!(pool.queue_counter, 0);
            assert_eq!(pool.workers_count, 1);

            match pool.execute(&mut tx, [&first, &second], &callable2) {
                IORuntimePoolExecute::Queued() => assert!(true),
                _ => assert!(false),
            }

            match tx.flush() {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            let mut entries = [IORingCompleteEntry::default(); 1];
            match rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false)
            }

            assert_eq!(entries[0].res, 24);
            assert_eq!(entries[0].user_data, first.encode());

            match callable2.result::<1, F2, u8, ()>(heap) {
                None => assert!(true),
                _ => assert!(false),
            }

            assert_eq!(pool.queue_counter, 0);
            assert_eq!(pool.workers_count, 1);

            drop(pool);
        }

        execute(&mut heap, target1, target2);
    }

    #[test]
    fn executes_callable_executed_second() {
        let mut heap = HeapPool::<1>::new();
        let target1 = || -> Result<u8, ()> { Ok(13) };
        let target2 = || -> Result<u8, ()> { Ok(17) };

        fn execute<F1, F2>(heap: &mut HeapPool<1>, target1: F1, target2: F2)
        where
            F1: FnOnce() -> Result<u8, ()> + Send,
            F2: FnOnce() -> Result<u8, ()> + Send,
        {
            let callable1 = match CallableTarget::allocate(heap, target1) {
                CallableTargetAllocate::Succeeded(val) => val,
                CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
            };

            let callable2 = match CallableTarget::allocate(heap, target2) {
                CallableTargetAllocate::Succeeded(val) => val,
                CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
            };

            let (rx, mut tx) = match IORing::init(8) {
                IORingInit::Succeeded(tx, rx) => (rx, tx),
                _ => return assert!(false),
            };

            let mut pool = match IORuntimePool::<1>::allocate() {
                IORuntimePoolAllocation::Succeeded(pool) => pool,
                _ => return assert!(false),
            };

            let first = IORingCompleterRef::new(1, 2);
            let second = IORingCompleterRef::new(3, 4);
            let third = IORingCompleterRef::new(5, 6);
            let fourth = IORingCompleterRef::new(7, 8);

            match pool.execute(&mut tx, [&first, &second], &callable1) {
                IORuntimePoolExecute::Executed() => assert!(true),
                _ => assert!(false),
            }

            match tx.flush() {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 2),
                _ => assert!(false),
            }

            let mut entries = [IORingCompleteEntry::default(); 1];
            match rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false)
            }

            assert_eq!(entries[0].res, 0);
            assert_eq!(entries[0].user_data, first.encode());

            let mut entries = [IORingCompleteEntry::default(); 1];
            match rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false)
            }

            assert_eq!(entries[0].res, 1);
            assert_eq!(entries[0].user_data, second.encode());

            match callable1.result::<1, F1, u8, ()>(heap) {
                Some(Ok(val)) => assert_eq!(val, 13),
                _ => assert!(false),
            }

            assert_eq!(pool.queue_counter, 0);
            assert_eq!(pool.workers_count, 1);

            match pool.execute(&mut tx, [&third, &fourth], &callable2) {
                IORuntimePoolExecute::Queued() => assert!(true),
                _ => assert!(false),
            }

            match tx.flush() {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            let mut entries = [IORingCompleteEntry::default(); 1];
            match rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false)
            }

            assert_eq!(entries[0].res, 24);
            assert_eq!(entries[0].user_data, third.encode());

            pool.enqueue(&third);
            assert_eq!(pool.queue_counter, 1);

            let res = pool.release_worker(&second);
            assert_eq!(res, true);

            match pool.trigger(&mut tx) {
                IORuntimePoolTrigger::Succeeded(val) => assert_eq!(val, true),
                _ => assert!(false),
            }

            match tx.flush() {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            let mut entries = [IORingCompleteEntry::default(); 1];
            match rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false)
            }

            assert_eq!(entries[0].res, 1);
            assert_eq!(entries[0].user_data, fourth.encode());

            match callable2.result::<1, F2, u8, ()>(heap) {
                Some(Ok(val)) => assert_eq!(val, 17),
                _ => assert!(false),
            }

            assert_eq!(pool.queue_counter, 0);
            assert_eq!(pool.workers_count, 1);

            drop(pool);
        }

        execute(&mut heap, target1, target2);
    }
}
