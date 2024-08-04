use ::core::future::Future;
use ::core::task::Context;
use ::core::task::Poll;
use ::core::task::Waker;

use super::callable::*;
use super::ops::*;
use super::pollable::*;
use super::pool::*;
use super::raw::*;
use super::refs::*;
use super::registry::*;
use crate::heap::*;
use crate::trace::*;
use crate::uring::*;

pub struct IORingRuntime {
    iteration: usize,
    registry: Boxed<IORingRegistry>,
    pool: Boxed<IORuntimePool<4>>,
    ops: IORuntimeOps,
    entries: [IORingCompleteEntry; 16],
}

pub struct IORingRuntimeContext {
    task: IORingTaskRef,
    runtime: *mut IORingRuntime,
}

pub enum IORingRuntimeAllocate {
    Succeeded(IORingRuntime),
    RingAllocationFailed(),
    PoolAllocationFailed(),
    RegistryAllocationFailed(),
}

impl IORingRuntime {
    pub fn allocate() -> IORingRuntimeAllocate {
        // registry needs to be alocated on the heap
        let registry = match IORingRegistry::allocate() {
            Ok(registry) => registry,
            Err(_) => return IORingRuntimeAllocate::RegistryAllocationFailed(),
        };

        let pool = match IORuntimePool::allocate() {
            IORuntimePoolAllocation::Succeeded(pool) => pool,
            IORuntimePoolAllocation::ThreadingFailed(_) => return IORingRuntimeAllocate::PoolAllocationFailed(),
            IORuntimePoolAllocation::AllocationFailed(_) => return IORingRuntimeAllocate::PoolAllocationFailed(),
            IORuntimePoolAllocation::QueueFailed(_) => return IORingRuntimeAllocate::PoolAllocationFailed(),
        };

        // I/O Ring needs initialization
        let ring = match IORing::init(32) {
            Ok(ring) => ring.droplet(),
            Err(_) => return IORingRuntimeAllocate::RingAllocationFailed(),
        };

        let ops = match IORuntimeOps::allocate(ring) {
            Some(ops) => ops,
            None => return IORingRuntimeAllocate::PoolAllocationFailed(),
        };

        // if everying is ready we just need to collect created components
        let runtime = Self {
            iteration: 0,
            registry: registry,
            pool: pool,
            ops: ops,
            entries: [IORingCompleteEntry::default(); 16],
        };

        IORingRuntimeAllocate::Succeeded(runtime)
    }
}

impl IORingRuntimeContext {
    pub fn from_waker<'a>(waker: &'a Waker) -> &'a mut IORingRuntimeContext {
        // reconstructing context requries assuming what is behind the pointer
        let ptr = waker.as_raw().data() as *mut IORingRuntimeContext;
        let context = unsafe { &mut *ptr };

        trace1(b"reconstructing context; addr=%x\n", ptr as *mut ());
        return context;
    }
}

impl IORingRuntime {
    fn poll(&mut self, task: IORingTaskRef) -> Result<(usize, Poll<Option<&'static [u8]>>), IORegistryError> {
        let runtime = self as *mut IORingRuntime;
        let context = IORingRuntimeContext { task, runtime };

        // waker contains always a mutable pointer to the runtime context
        let data = &context as *const IORingRuntimeContext;
        let waker = unsafe { Waker::from_raw(make_waker(data as *const ())) };

        trace2(b"# polling task; tid=%d, ctx=%x\n", context.task.tid(), data as *const ());
        let mut cx = Context::from_waker(&waker);

        // we always poll through registry to not expose details
        return self.registry.poll(&context.task, &mut cx);
    }
}

pub enum IORingRuntimeSpawn {
    Pending(IORingTaskRef),
    Draining(IORingTaskRef),
    Completed(Option<&'static [u8]>),
    InternallyFailed(),
    NotEnoughSlots(),
}

impl IORingRuntime {
    fn spawn(&mut self, task: PollableTarget) -> IORingRuntimeSpawn {
        // each future has to be put on the heap first
        trace0(b"appending task to registry\n");

        let task = match self.registry.append_task(task) {
            // and later to be appended to the registry
            Ok(val) => val,
            Err(IORegistryError::NotEnoughSlots) => return IORingRuntimeSpawn::NotEnoughSlots(),
            Err(_) => return IORingRuntimeSpawn::InternallyFailed(),
        };

        // to be initially polled
        let (result, completions) = match self.poll(task) {
            Ok((cnt, Poll::Ready(val))) => (val, cnt),
            Ok((_, Poll::Pending)) => return IORingRuntimeSpawn::Pending(task),
            Err(_) => return IORingRuntimeSpawn::InternallyFailed(),
        };

        if completions == 0 {
            // to be immediately removed if ready without hanging completers
            let result = match self.registry.remove_task(&task) {
                Ok(task) => task.release(),
                Err(_) => return IORingRuntimeSpawn::InternallyFailed(),
            };

            match result {
                None => trace1(b"task completed; tid=%d\n", task.tid()),
                Some(res) => trace2(b"task completed; tid=%d, res='%s'\n", task.tid(), res),
            }

            return IORingRuntimeSpawn::Completed(result);
        }

        match result {
            None => trace1(b"task draining; tid=%d\n", task.tid()),
            Some(res) => trace2(b"task draining; tid=%d, res='%s'\n", task.tid(), res),
        }

        // otherwise we left it in a draining mode
        IORingRuntimeSpawn::Draining(task)
    }
}

impl IORingRuntimeContext {
    pub fn spawn(&mut self, task: PollableTarget) -> IORingRuntimeSpawn {
        unsafe { (*self.runtime).spawn(task) }
    }
}

pub enum IORingRuntimeExecute {
    Queued(IORingCompleterRef, IORingCompleterRef),
    Executed(IORingCompleterRef, IORingCompleterRef),
    NotEnoughSlots(),
    InternallyFailed(),
}

impl IORingRuntime {
    fn execute(&mut self, task: &IORingTaskRef, callable: &CallableTarget) -> IORingRuntimeExecute {
        let queued = match self.registry.append_completer(task.clone()) {
            Ok(completer) => completer,
            Err(IORegistryError::NotEnoughSlots) => return IORingRuntimeExecute::NotEnoughSlots(),
            Err(_) => return IORingRuntimeExecute::InternallyFailed(),
        };

        let executed = match self.registry.append_completer(task.clone()) {
            Ok(completer) => completer,
            Err(IORegistryError::NotEnoughSlots) => return IORingRuntimeExecute::NotEnoughSlots(),
            Err(_) => return IORingRuntimeExecute::InternallyFailed(),
        };

        let mut slots: [Option<(u64, IORingSubmitEntry)>; 4] = [const { None }; 4];
        let cnt = match self.pool.execute(&mut slots, [&queued, &executed], callable) {
            Ok(Some(cnt)) => cnt,
            Ok(None) => 0,
            Err(_) => return IORingRuntimeExecute::InternallyFailed(),
        };

        // potentially received submits has to be processed
        for index in 0..cnt {
            let (user_data, entry) = unsafe {
                match slots.get_unchecked_mut(index).take() {
                    None => continue,
                    Some((user_data, entry)) => (user_data, entry),
                }
            };

            self.ops.submit(user_data, [entry]);
        }

        if cnt == 1 {
            IORingRuntimeExecute::Queued(queued, executed)
        } else {
            IORingRuntimeExecute::Executed(queued, executed)
        }
    }
}

impl IORingRuntimeContext {
    pub fn execute(&mut self, callable: &CallableTarget) -> IORingRuntimeExecute {
        unsafe { (*self.runtime).execute(&self.task, callable) }
    }
}

#[allow(dead_code)]
enum IORingRuntimeTick {
    Succeeded(),
    Empty(),
    Pending(IORingTaskRef),
    Draining(IORingTaskRef),
    Completed(IORingTaskRef, Option<&'static [u8]>),
    CompletionFailed(isize),
    InternallyFailed(),
}

impl IORingRuntime {
    fn tick(&mut self) -> IORingRuntimeTick {
        let tasks = self.registry.tasks();
        let completers = self.registry.completers();

        trace1(b"--------------------------------------------- %d\n", self.iteration);
        trace2(b"looking for completions; tasks=%d, completers=%d\n", tasks, completers);

        if tasks == 0 {
            // nothing to poll
            trace0(b"looking for completions; nothing to poll\n");
            return IORingRuntimeTick::Empty();
        }

        // increase iteration
        self.iteration += 1;

        // and wait for some event
        let cnt = loop {
            // sometimes we may end up in unexpected empty shot
            match self.ops.receive(&mut self.entries) {
                IORingComplete::UnexpectedEmpty(_) => continue,
                IORingComplete::Succeeded(cnt) => break cnt,
                IORingComplete::CompletionFailed(err) => return IORingRuntimeTick::CompletionFailed(err),
            }
        };

        for i in 0..cnt {
            let entry = self.entries[i];
            let completer = IORingCompleterRef::decode(entry.user_data);

            match self.complete(&completer, &entry) {
                IORingRuntimeTick::Succeeded() => (),
                IORingRuntimeTick::Empty() => (),
                IORingRuntimeTick::Pending(_) => (),
                IORingRuntimeTick::Draining(_) => (),
                IORingRuntimeTick::Completed(_, _) => (),
                val => return val,
            }
        }
        // user data contains encoded completion

        match self.ops.flush() {
            IORingSubmit::Succeeded(_) => (),
            _ => return IORingRuntimeTick::InternallyFailed(),
        }

        IORingRuntimeTick::Succeeded()
    }

    fn complete(&mut self, completer: &IORingCompleterRef, entry: &IORingCompleteEntry) -> IORingRuntimeTick {
        trace2(b"looking for completions; cidx=%d, cid=%d\n", completer.cidx(), completer.cid());

        // complete received completer idx, it will return idx, id, readiness and completers of the found task
        let (task, ready, mut cnt) = match self.registry.complete(completer, entry.res) {
            Ok((task, ready, cnt)) => (task, ready, cnt),
            Err(_) => return IORingRuntimeTick::InternallyFailed(),
        };

        if !ready {
            // when task is not yet ready we need to poll it again
            let (_, completions) = match self.poll(task) {
                Ok((cnt, Poll::Ready(val))) => (val, cnt),
                Ok((_, Poll::Pending)) => return IORingRuntimeTick::Pending(task),
                Err(_) => return IORingRuntimeTick::InternallyFailed(),
            };

            // completions may have changed after polling
            cnt = completions;
        }

        if cnt > 0 {
            // completers indicate that task is draining
            return IORingRuntimeTick::Draining(task);
        }

        // no draining and readiness, so remove the task
        let result = match self.registry.remove_task(&task) {
            Ok(task) => task.release(),
            Err(_) => return IORingRuntimeTick::InternallyFailed(),
        };

        // and return the task result
        return IORingRuntimeTick::Completed(task, result);
    }
}

#[allow(dead_code)]
pub enum IORingRuntimeRun {
    Completed(Option<&'static [u8]>),
    CompletionFailed(isize),
    AllocationFailed(isize),
    InternallyFailed(),
}

impl IORingRuntime {
    pub fn run<'a, F, C>(&mut self, callback: C) -> IORingRuntimeRun
    where
        F: Future<Output = Option<&'static [u8]>> + Send + 'a,
        C: FnOnce(IORuntimeOps) -> F + Unpin + Send + 'a,
    {
        let ops = self.ops.duplicate();
        let target = callback.call_once((ops,));

        trace0(b"allocating memory to pin a future\n");
        let pinned = match PollableTarget::allocate(&mut self.ops.ctx.heap_pool, target) {
            Some(pinned) => pinned,
            None => return IORingRuntimeRun::AllocationFailed(0),
        };

        // spawning may fail due to many reasons
        let mut result = None;
        trace0(b"spawning pinned future\n");

        let spawned = match self.spawn(pinned) {
            IORingRuntimeSpawn::InternallyFailed() => return IORingRuntimeRun::InternallyFailed(),
            IORingRuntimeSpawn::NotEnoughSlots() => return IORingRuntimeRun::InternallyFailed(),
            IORingRuntimeSpawn::Pending(task) => Some(task),
            IORingRuntimeSpawn::Draining(task) => Some(task),
            IORingRuntimeSpawn::Completed(res) => {
                result = res;
                None
            }
        };

        match self.ops.flush() {
            IORingSubmit::Succeeded(_) => (),
            _ => return IORingRuntimeRun::InternallyFailed(),
        }

        loop {
            match self.tick() {
                IORingRuntimeTick::Empty() => break,
                IORingRuntimeTick::Succeeded() => continue,
                IORingRuntimeTick::Pending(_) => continue,
                IORingRuntimeTick::Draining(_) => continue,
                IORingRuntimeTick::Completed(task, res) => {
                    if let Some(spawned) = spawned {
                        if task.tid() == spawned.tid() {
                            result = res;
                        }
                    }
                }
                IORingRuntimeTick::CompletionFailed(err) => return IORingRuntimeRun::CompletionFailed(err),
                IORingRuntimeTick::InternallyFailed() => return IORingRuntimeRun::InternallyFailed(),
            }
        }

        IORingRuntimeRun::Completed(result)
    }
}

pub enum IORingRuntimeExtract {
    Succeeded(i32),
    NotCompleted(),
    NotFound(),
    InternallyFailed(),
}

impl IORingRuntime {
    fn extract(&mut self, completer: &IORingCompleterRef) -> IORingRuntimeExtract {
        trace2(b"extracting completer; cidx=%d, cid=%d\n", completer.cidx(), completer.cid());

        let completion = match self.registry.remove_completer(completer) {
            Ok(completer) => completer,
            Err(IORegistryError::CompleterNotFound) => return IORingRuntimeExtract::NotFound(),
            Err(IORegistryError::CompleterNotReady) => {
                trace2(b"removing completer; cidx=%d, cid=%d, not ready\n", completer.cidx(), completer.cid());

                return IORingRuntimeExtract::NotCompleted();
            }
            Err(_) => return IORingRuntimeExtract::InternallyFailed(),
        };

        let value = match completion.result() {
            Some(value) => value,
            None => return IORingRuntimeExtract::NotCompleted(),
        };

        trace3(b"removing completer; cidx=%d, cid=%d, res=%d\n", completer.cidx(), completer.cid(), value);

        IORingRuntimeExtract::Succeeded(value)
    }
}

impl IORingRuntimeContext {
    pub fn extract(&mut self, completer: &IORingCompleterRef) -> IORingRuntimeExtract {
        unsafe { (*self.runtime).extract(completer) }
    }
}

impl IORingRuntime {
    fn enqueue(&mut self, completer: &IORingCompleterRef) {
        let mut slots: [Option<(u64, IORingSubmitEntry)>; 1] = [const { None }; 1];

        // first making a callable visible
        self.pool.enqueue(completer);

        // possibly it will be triggered now
        let cnt = match self.pool.trigger(&mut slots) {
            Ok(None) | Ok(Some(0)) => 0,
            Ok(Some(cnt)) => cnt,
            Err(_) => 0,
        };

        // potentially received submits has to be processed
        for index in 0..cnt {
            let (user_data, entry) = unsafe {
                match slots.get_unchecked_mut(index).take() {
                    None => continue,
                    Some((user_data, entry)) => (user_data, entry),
                }
            };

            self.ops.submit(user_data, [entry]);
        }
    }
}

impl IORingRuntimeContext {
    pub fn enqueue(&mut self, completer: &IORingCompleterRef) {
        unsafe { (*self.runtime).enqueue(completer) }
    }
}

impl IORingRuntime {
    fn trigger(&mut self, completer: &IORingCompleterRef) {
        let mut slots: [Option<(u64, IORingSubmitEntry)>; 1] = [const { None }; 1];

        // first release worker behind completer
        self.pool.release_worker(completer);

        // possibly it will be triggered now
        let cnt = match self.pool.trigger(&mut slots) {
            Ok(None) | Ok(Some(0)) => 0,
            Ok(Some(cnt)) => cnt,
            Err(_) => 0,
        };

        // potentially received submits has to be processed
        for index in 0..cnt {
            let (user_data, entry) = unsafe {
                match slots.get_unchecked_mut(index).take() {
                    None => continue,
                    Some((user_data, entry)) => (user_data, entry),
                }
            };

            self.ops.submit(user_data, [entry]);
        }
    }
}

impl IORingRuntimeContext {
    pub fn trigger(&mut self, completer: &IORingCompleterRef) {
        unsafe { (*self.runtime).trigger(completer) }
    }
}

#[allow(dead_code)]
pub enum IORingRuntimeSubmit {
    Succeeded(IORingCompleterRef),
    SubmissionFailed(Option<isize>),
    InternallyFailed(),
    NotEnoughSlots(),
}

impl IORingRuntime {
    fn submit(&mut self, task: &IORingTaskRef, entry: IORingSubmitEntry) -> IORingRuntimeSubmit {
        trace1(b"appending completer to registry; tid=%d\n", task.tid());
        let completer = match self.registry.append_completer(task.clone()) {
            Ok(completer) => completer,
            Err(IORegistryError::NotEnoughSlots) => return IORingRuntimeSubmit::NotEnoughSlots(),
            Err(_) => return IORingRuntimeSubmit::InternallyFailed(),
        };

        trace2(b"submitting op with uring; cidx=%d, cid=%d\n", completer.cidx(), completer.cid());

        let err = match self.ops.submit(completer.encode(), [entry]) {
            IORingSubmit::Succeeded(_) => {
                trace1(b"submitting op with uring; cidx=%d, succeeded\n", completer.cidx());
                return IORingRuntimeSubmit::Succeeded(completer);
            }
            IORingSubmit::SubmissionFailed(err) => {
                trace2(b"submitting op with uring; cidx=%d, err=%d\n", completer.cidx(), err);
                Some(err)
            }
            IORingSubmit::SubmissionMismatched(_) => {
                trace1(b"submitting op with uring; cidx=%d, failed\n", completer.cidx());
                None
            }
        };

        match self.registry.remove_completer(&completer) {
            Ok(_) => IORingRuntimeSubmit::SubmissionFailed(err),
            _ => IORingRuntimeSubmit::InternallyFailed(),
        }
    }
}

impl IORingRuntimeContext {
    pub fn submit(&mut self, entry: IORingSubmitEntry) -> IORingRuntimeSubmit {
        unsafe { (*self.runtime).submit(&self.task, entry) }
    }
}

pub enum IORingRuntimeShutdown {
    Succeeded(),
    ConsolidationFailed(),
    ShutdownFailed(),
}

impl IORingRuntime {
    pub fn shutdown(self) -> IORingRuntimeShutdown {
        IORingRuntimeShutdown::Succeeded()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::*;

    #[test]
    fn allocates_runtime() {
        let runtime = match IORingRuntime::allocate() {
            IORingRuntimeAllocate::Succeeded(runtime) => runtime,
            _ => return assert!(false),
        };

        match runtime.shutdown() {
            IORingRuntimeShutdown::Succeeded() => assert!(true),
            _ => assert!(false),
        }
    }

    #[test]
    fn runs_task_without_async_code() {
        let mut runtime = match IORingRuntime::allocate() {
            IORingRuntimeAllocate::Succeeded(runtime) => runtime,
            _ => return assert!(false),
        };

        let callback = |_| async { None::<&'static [u8]> };

        match runtime.run(callback) {
            IORingRuntimeRun::Completed(val) => assert!(val.is_none()),
            _ => assert!(false),
        };

        match runtime.shutdown() {
            IORingRuntimeShutdown::Succeeded() => assert!(true),
            _ => assert!(false),
        }
    }

    #[test]
    fn runs_task_with_async_code() {
        let mut runtime = match IORingRuntime::allocate() {
            IORingRuntimeAllocate::Succeeded(runtime) => runtime,
            _ => return assert!(false),
        };

        let callback = |mut ops: IORuntimeOps| async move {
            match ops.timeout(0, 1).await {
                TimeoutResult::Succeeded() => assert!(true),
                _ => assert!(false),
            }

            None::<&'static [u8]>
        };

        match runtime.run(callback) {
            IORingRuntimeRun::Completed(val) => assert!(val.is_none()),
            _ => assert!(false),
        };

        match runtime.shutdown() {
            IORingRuntimeShutdown::Succeeded() => assert!(true),
            _ => assert!(false),
        }
    }
}
