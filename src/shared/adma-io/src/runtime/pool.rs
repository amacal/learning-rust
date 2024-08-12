use super::callable::*;
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

impl<const T: usize> IORuntimePool<T> {
    pub fn allocate() -> Result<IORuntimePool<T>, Option<i32>> {
        let queue = match PipeChannel::create() {
            Ok(value) => value,
            Err(_) => return Err(None),
        };

        let mut workers_slots: [usize; T] = [0; T];
        let mut workers_array: [Option<Worker>; T] = [const { None }; T];

        for i in 0..T {
            let worker = match Worker::start() {
                WorkerStart::Succeeded(worker) => worker,
                WorkerStart::StartFailed(errno) => return Err(errno),
                WorkerStart::PipesFailed(errno) => return Err(errno),
                WorkerStart::StackFailed(errno) => return Err(errno),
            };

            workers_array[i] = Some(worker);
            workers_slots[i] = i;
        }

        let (incoming, outgoing) = queue.extract();
        let instance = IORuntimePool{
            queue_counter: 0,
            queue_incoming: incoming,
            queue_outgoing: outgoing,
            workers_count: 0,
            workers_slots: workers_slots,
            workers_array: workers_array,
            workers_completers: [const { None }; T],
        };

        Ok(instance)
    }

    fn drop_ref(&mut self) {
        trace1(b"releasing threads droplet; size=%d\n", T);

        sys_close(self.queue_incoming);
        sys_close(self.queue_outgoing);

        for i in 0..T {
            if let Some(mut worker) = self.workers_array[i].take() {
                worker.release()
            }
        }
    }

    pub fn droplet(self) -> Droplet<Self> {
        trace1(b"creating threads droplet; size=%d\n", T);
        Droplet::from(self, Self::drop_ref)

    }
}

impl<const T: usize> IORuntimePool<T> {
    pub fn execute(
        &mut self,
        slots: &mut [Option<(u64, IORingSubmitEntry)>; 4],
        completers: [&IORingCompleterRef; 2],
        callable: &CallableTarget,
    ) -> Result<usize, Option<i32>> {
        // acquire worker
        if let Some(slot) = self.workers_slots.get(self.workers_count) {
            trace1(b"acquired worker; slot=%d\n", *slot);

            let worker = match self.workers_array.get_mut(*slot) {
                Some(Some(worker)) => worker,
                _ => return Err(None),
            };

            // confirm queuing
            let op = IORingSubmitEntry::noop();
            slots[0] = Some((completers[0].encode(), op));

            // prepare execute op
            let op = match worker.execute(callable) {
                WorkerExecute::Succeeded(op) => op,
                WorkerExecute::OutgoingPipeFailed(errno) => return Err(errno),
            };

            // confirm execution
            slots[1] = Some((completers[1].encode(), op));

            // update internal counter and correlate worker with completer
            self.workers_count += 1;
            self.workers_completers[*slot] = Some(completers[1].encode());

            return Ok(2);
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
        slots[0] = Some((completers[0].encode(), op));

        return Ok(1);
    }
}

impl<const T: usize> IORuntimePool<T> {
    pub fn enqueue(&mut self, completer: &IORingCompleterRef) {
        self.queue_counter += 1;
        trace1(b"callable queued; cid=%d\n", completer.cid());
    }
}

impl<const T: usize> IORuntimePool<T> {
    pub fn release(&mut self, completer: &IORingCompleterRef) -> bool {
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

impl<const T: usize> IORuntimePool<T> {
    pub fn trigger(&mut self, slots: &mut [Option<(u64, IORingSubmitEntry)>; 1]) -> Result<Option<usize>, Option<i32>> {
        if self.queue_counter <= 0 {
            return Ok(None);
        }

        if let Some(slot) = self.workers_slots.get(self.workers_count) {
            trace1(b"acquired worker; slot=%d\n", *slot);

            // worker still theoretically may fail
            let worker = match self.workers_array.get_mut(*slot) {
                Some(Some(worker)) => worker,
                Some(None) | None => return Err(None),
            };

            // buffer is needed to collect data from the pipe
            let mut buffer: [u8; 24] = [0; 24];
            let ptr = buffer.as_mut_ptr() as *mut ();

            // we expect to read ptr, len, encoded completer triple from a queue
            trace0(b"acquiring callable\n");
            match sys_read(self.queue_incoming, ptr, 24) {
                value if value == 24 => self.queue_counter -= 1,
                value if value >= 0 => return Err(None),
                value => match i32::try_from(value) {
                    Ok(value) => return Err(Some(value)),
                    Err(_) => return Err(None),
                },
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
                _ => return Err(None),
            };

            // by registering it within I/O Ring
            slots[0] = Some((unsafe { *encoded }, op));

            // not forgetting about maintaining the state
            self.workers_completers[*slot] = unsafe { Some(*encoded) };
            self.workers_count += 1;

            return Ok(Some(1));
        }

        Ok(Some(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_pool() {
        let pool = match IORuntimePool::<16>::allocate() {
            Ok(pool) => pool,
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
                Ok(target) => target,
                Err(_) => return assert!(false),
            };

            let mut ring = match IORing::init(8) {
                Ok(ring) => ring.droplet(),
                _ => return assert!(false),
            };

            let mut pool = match IORuntimePool::<1>::allocate() {
                Ok(pool) => pool,
                _ => return assert!(false),
            };

            let first = IORingCompleterRef::new(1, 2);
            let second = IORingCompleterRef::new(3, 4);

            let mut slots: [Option<(u64, IORingSubmitEntry)>; 4] = [const { None }; 4];
            match pool.execute(&mut slots, [&first, &second], &callable) {
                Ok(cnt) => assert_eq!(cnt, 2),
                _ => assert!(false),
            }

            let (user_data, entry) = match slots[0].take() {
                Some((user_data, entry)) => (user_data, entry),
                _ => return assert!(false),
            };

            assert_eq!(user_data, first.encode());
            match ring.tx.submit(user_data, [entry]) {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            let (user_data, entry) = match slots[1].take() {
                Some((user_data, entry)) => (user_data, entry),
                _ => return assert!(false),
            };

            assert_eq!(user_data, second.encode());
            match ring.tx.submit(user_data, [entry]) {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            match ring.tx.flush() {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 2),
                _ => assert!(false),
            }

            let mut entries = [IORingCompleteEntry::default(); 1];
            match ring.rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false),
            }

            assert_eq!(entries[0].res, 0);
            assert_eq!(entries[0].user_data, first.encode());

            let mut entries = [IORingCompleteEntry::default(); 1];
            match ring.rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false),
            }

            assert_eq!(entries[0].res, 1);
            assert_eq!(entries[0].user_data, second.encode());

            match callable.result::<1, F, u8, ()>(heap) {
                Ok(Some(Ok(val))) => assert_eq!(val, 13),
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
                Ok(target) => target,
                Err(_) => return assert!(false),
            };

            let callable2 = match CallableTarget::allocate(heap, target2) {
                Ok(target) => target,
                Err(_) => return assert!(false),
            };

            let mut ring = match IORing::init(8) {
                Ok(ring) => ring.droplet(),
                _ => return assert!(false),
            };

            let mut pool = match IORuntimePool::<1>::allocate() {
                Ok(pool) => pool,
                _ => return assert!(false),
            };

            let first = IORingCompleterRef::new(1, 2);
            let second = IORingCompleterRef::new(3, 4);

            let mut slots: [Option<(u64, IORingSubmitEntry)>; 4] = [const { None }; 4];
            match pool.execute(&mut slots, [&first, &second], &callable1) {
                Ok(cnt) => assert_eq!(cnt, 2),
                _ => assert!(false),
            }

            let (user_data, entry) = match slots[0].take() {
                Some((user_data, entry)) => (user_data, entry),
                _ => return assert!(false),
            };

            assert_eq!(user_data, first.encode());
            match ring.tx.submit(user_data, [entry]) {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            let (user_data, entry) = match slots[1].take() {
                Some((user_data, entry)) => (user_data, entry),
                _ => return assert!(false),
            };

            assert_eq!(user_data, second.encode());
            match ring.tx.submit(user_data, [entry]) {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            match ring.tx.flush() {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 2),
                _ => assert!(false),
            }

            let mut entries = [IORingCompleteEntry::default(); 1];
            match ring.rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false),
            }

            assert_eq!(entries[0].res, 0);
            assert_eq!(entries[0].user_data, first.encode());

            let mut entries = [IORingCompleteEntry::default(); 1];
            match ring.rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false),
            }

            assert_eq!(entries[0].res, 1);
            assert_eq!(entries[0].user_data, second.encode());

            match callable1.result::<1, F1, u8, ()>(heap) {
                Ok(Some(Ok(val))) => assert_eq!(val, 13),
                _ => assert!(false),
            }

            assert_eq!(pool.queue_counter, 0);
            assert_eq!(pool.workers_count, 1);

            let mut slots: [Option<(u64, IORingSubmitEntry)>; 4] = [const { None }; 4];
            match pool.execute(&mut slots, [&first, &second], &callable2) {
                Ok(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            let (user_data, entry) = match slots[0].take() {
                Some((user_data, entry)) => (user_data, entry),
                _ => return assert!(false),
            };

            assert_eq!(user_data, first.encode());
            assert!(slots[1].is_none());

            match ring.tx.submit(user_data, [entry]) {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            match ring.tx.flush() {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            let mut entries = [IORingCompleteEntry::default(); 1];
            match ring.rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false),
            }

            assert_eq!(entries[0].res, 24);
            assert_eq!(entries[0].user_data, first.encode());

            match callable2.result::<1, F2, u8, ()>(heap) {
                Ok(None) => assert!(true),
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
                Ok(target) => target,
                Err(_) => return assert!(false),
            };

            let callable2 = match CallableTarget::allocate(heap, target2) {
                Ok(target) => target,
                Err(_) => return assert!(false),
            };

            let mut ring = match IORing::init(8) {
                Ok(ring) => ring.droplet(),
                _ => return assert!(false),
            };

            let mut pool = match IORuntimePool::<1>::allocate() {
                Ok(pool) => pool,
                _ => return assert!(false),
            };

            let first = IORingCompleterRef::new(1, 2);
            let second = IORingCompleterRef::new(3, 4);
            let third = IORingCompleterRef::new(5, 6);
            let fourth = IORingCompleterRef::new(7, 8);

            let mut slots: [Option<(u64, IORingSubmitEntry)>; 4] = [const { None }; 4];
            match pool.execute(&mut slots, [&first, &second], &callable1) {
                Ok(cnt) => assert_eq!(cnt, 2),
                _ => assert!(false),
            }

            let (user_data, entry) = match slots[0].take() {
                Some((user_data, entry)) => (user_data, entry),
                _ => return assert!(false),
            };

            assert_eq!(user_data, first.encode());
            match ring.tx.submit(user_data, [entry]) {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            let (user_data, entry) = match slots[1].take() {
                Some((user_data, entry)) => (user_data, entry),
                _ => return assert!(false),
            };

            assert_eq!(user_data, second.encode());
            match ring.tx.submit(user_data, [entry]) {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            match ring.tx.flush() {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 2),
                _ => assert!(false),
            }

            let mut entries = [IORingCompleteEntry::default(); 1];
            match ring.rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false),
            }

            assert_eq!(entries[0].res, 0);
            assert_eq!(entries[0].user_data, first.encode());

            let mut entries = [IORingCompleteEntry::default(); 1];
            match ring.rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false),
            }

            assert_eq!(entries[0].res, 1);
            assert_eq!(entries[0].user_data, second.encode());

            match callable1.result::<1, F1, u8, ()>(heap) {
                Ok(Some(Ok(val))) => assert_eq!(val, 13),
                _ => assert!(false),
            }

            assert_eq!(pool.queue_counter, 0);
            assert_eq!(pool.workers_count, 1);

            let mut slots: [Option<(u64, IORingSubmitEntry)>; 4] = [const { None }; 4];
            match pool.execute(&mut slots, [&third, &fourth], &callable2) {
                Ok(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            let (user_data, entry) = match slots[0].take() {
                Some((user_data, entry)) => (user_data, entry),
                _ => return assert!(false),
            };

            assert_eq!(user_data, third.encode());
            assert!(slots[1].is_none());

            match ring.tx.submit(user_data, [entry]) {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            match ring.tx.flush() {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            let mut entries = [IORingCompleteEntry::default(); 1];
            match ring.rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false),
            }

            assert_eq!(entries[0].res, 24);
            assert_eq!(entries[0].user_data, third.encode());

            pool.enqueue(&third);
            assert_eq!(pool.queue_counter, 1);

            let res = pool.release(&second);
            assert_eq!(res, true);

            let mut slots: [Option<(u64, IORingSubmitEntry)>; 1] = [const { None }; 1];

            match pool.trigger(&mut slots) {
                Ok(Some(cnt)) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            let (user_data, entry) = match slots.get_mut(0) {
                Some(entry) => match entry.take() {
                    Some((user_data, entry)) => (user_data, entry),
                    _ => return assert!(false),
                },
                _ => return assert!(false),
            };

            assert_eq!(user_data, fourth.encode());
            match ring.tx.submit(user_data, [entry]) {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            match ring.tx.flush() {
                IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => assert!(false),
            }

            let mut entries = [IORingCompleteEntry::default(); 1];
            match ring.rx.complete(&mut entries) {
                IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
                _ => return assert!(false),
            }

            assert_eq!(entries[0].res, 1);
            assert_eq!(entries[0].user_data, fourth.encode());

            match callable2.result::<1, F2, u8, ()>(heap) {
                Ok(Some(Ok(val))) => assert_eq!(val, 17),
                _ => assert!(false),
            }

            assert_eq!(pool.queue_counter, 0);
            assert_eq!(pool.workers_count, 1);

            drop(pool);
        }

        execute(&mut heap, target1, target2);
    }
}
