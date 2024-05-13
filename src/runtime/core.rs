use ::core::future::Future;
use ::core::task::Context;
use ::core::task::Waker;

use super::pin::*;
use super::raw::*;
use super::refs::*;
use super::registry::*;
use crate::heap::*;
use crate::trace::*;
use crate::uring::*;

pub struct IORingRuntime {
    iteration: usize,
    submitter: IORingSubmitter,
    completer: IORingCompleter,
    registry: Boxed<IORingRegistry>,
}

pub struct IORingRuntimeContext {
    task: IORingTaskRef,
    runtime: *mut IORingRuntime,
}

#[allow(dead_code)]
pub enum IORingRuntimeCreate {
    Succeeded(IORingRuntime),
    HeapFailed(isize),
}

impl IORingRuntime {
    pub fn create(submitter: IORingSubmitter, completer: IORingCompleter) -> IORingRuntimeCreate {
        // registry needs to be alocated on the heap
        let registry = match IORingRegistry::allocate() {
            IORingRegistryAllocation::Succeeded(registry) => registry,
            IORingRegistryAllocation::AllocationFailed(err) => return IORingRuntimeCreate::HeapFailed(err),
        };

        let runtime = Self {
            iteration: 0,
            submitter: submitter,
            completer: completer,
            registry: registry,
        };

        IORingRuntimeCreate::Succeeded(runtime)
    }
}

impl IORingRuntime {
    pub fn from_waker<'a>(waker: &'a Waker) -> &'a mut IORingRuntimeContext {
        // reconstructing context requries assuming what is behind the pointer
        let ptr = waker.as_raw().data() as *mut IORingRuntimeContext;
        let context = unsafe { &mut *ptr };

        trace1(b"reconstructing context; addr=%x\n", ptr);
        return context;
    }

    fn poll(&mut self, task: IORingTaskRef) -> IORingRegistryPoll {
        let runtime = self as *mut IORingRuntime;
        let context = IORingRuntimeContext { task, runtime };

        // waker contains always a mutable pointer to the runtime context
        let data = &context as *const IORingRuntimeContext;
        let waker = unsafe { Waker::from_raw(make_waker(data as *const ())) };

        trace2(b"# polling task; tid=%d, ctx=%x\n", context.task.tid(), data);
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
    fn spawn(&mut self, pinned: IORingPin) -> IORingRuntimeSpawn {
        // each future has to be put on the heap first
        trace0(b"appending task to registry\n");

        let task = match self.registry.append_task(pinned) {
            // and later to be appended to the registry
            IORingRegistryAppend::Succeeded(val) => val,
            IORingRegistryAppend::NotEnoughSlots() => return IORingRuntimeSpawn::NotEnoughSlots(),
            IORingRegistryAppend::InternallyFailed() => return IORingRuntimeSpawn::InternallyFailed(),
        };

        // to be initially polled
        let (val, completions) = match self.poll(task) {
            IORingRegistryPoll::Ready(cnt, val) => (val, cnt),
            IORingRegistryPoll::NotFound() => return IORingRuntimeSpawn::InternallyFailed(),
            IORingRegistryPoll::Pending(_) => return IORingRuntimeSpawn::Pending(task),
        };

        if completions == 0 {
            // to be immediately removed if ready without hanging completers
            return match self.registry.remove_task(&task) {
                IORingRegistryRemove::Succeeded(_) => IORingRuntimeSpawn::Completed(val),
                IORingRegistryRemove::NotFound() => IORingRuntimeSpawn::InternallyFailed(),
                IORingRegistryRemove::NotReady() => IORingRuntimeSpawn::InternallyFailed(),
            };
        }

        // otherwise we left it in a draining mode
        IORingRuntimeSpawn::Draining(task)
    }
}

impl IORingRuntimeContext {
    pub fn spawn(&mut self, pinned: IORingPin) -> IORingRuntimeSpawn {
        unsafe { (*self.runtime).spawn(pinned) }
    }
}

#[allow(dead_code)]
enum IORingRuntimeTick {
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
        let entry = loop {
            // sometimes we may end up in unexpected empty shot
            match self.completer.complete() {
                IORingComplete::UnexpectedEmpty(_) => continue,
                IORingComplete::Succeeded(entry) => break entry,
                IORingComplete::CompletionFailed(err) => return IORingRuntimeTick::CompletionFailed(err),
            }
        };

        // user data contains encoded completion
        let completer = IORingCompleterRef::decode(entry.user_data);

        trace2(
            b"looking for completions; cidx=%d, cid=%d\n",
            completer.cidx(),
            completer.cid(),
        );

        // complete received completer idx, it will return idx, id, readiness and completers of the found task
        let (task, ready, mut cnt) = match self.registry.complete(completer, entry.res) {
            IORingRegistryComplete::Succeeded(task, ready, cnt) => (task, ready, cnt),
            IORingRegistryComplete::NotFound() => return IORingRuntimeTick::InternallyFailed(),
            IORingRegistryComplete::Inconsistent() => return IORingRuntimeTick::InternallyFailed(),
        };

        if !ready {
            // when task is not yet ready we need to poll it again
            let (_, completions) = match self.poll(task) {
                IORingRegistryPoll::Ready(cnt, val) => (val, cnt),
                IORingRegistryPoll::Pending(_) => return IORingRuntimeTick::Pending(task),
                IORingRegistryPoll::NotFound() => return IORingRuntimeTick::InternallyFailed(),
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
            IORingRegistryRemove::Succeeded(task) => task.release(),
            IORingRegistryRemove::NotFound() => return IORingRuntimeTick::InternallyFailed(),
            IORingRegistryRemove::NotReady() => return IORingRuntimeTick::InternallyFailed(),
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
    pub fn run<F>(&mut self, future: F) -> IORingRuntimeRun
    where
        F: Future<Output = Option<&'static [u8]>>,
    {
        trace0(b"allocating memory to pin a future\n");
        let pinned = match IORingPin::allocate(future) {
            IORingPinAllocate::Succeeded(pinned) => pinned,
            IORingPinAllocate::AllocationFailed(err) => return IORingRuntimeRun::AllocationFailed(err),
        };

        // spawning may fail due to many reasons
        let mut result = None;
        trace0(b"spawning pinned future\n");

        let spawned = match self.spawn(pinned) {
            IORingRuntimeSpawn::Completed(res) => return IORingRuntimeRun::Completed(res),
            IORingRuntimeSpawn::InternallyFailed() => return IORingRuntimeRun::InternallyFailed(),
            IORingRuntimeSpawn::NotEnoughSlots() => return IORingRuntimeRun::InternallyFailed(),
            IORingRuntimeSpawn::Pending(task) => task,
            IORingRuntimeSpawn::Draining(task) => task,
        };

        loop {
            match self.tick() {
                IORingRuntimeTick::Empty() => break,
                IORingRuntimeTick::Pending(_) => continue,
                IORingRuntimeTick::Draining(_) => continue,
                IORingRuntimeTick::Completed(task, res) => {
                    if task.tid() == spawned.tid() {
                        result = res;
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
}

impl IORingRuntime {
    fn extract(&mut self, completer: &IORingCompleterRef) -> IORingRuntimeExtract {
        trace2(
            b"extracting completer; cidx=%d, cid=%d\n",
            completer.cidx(),
            completer.cid(),
        );

        let completion = match self.registry.remove_completer(completer) {
            IORingRegistryRemove::Succeeded(completer) => completer,
            IORingRegistryRemove::NotFound() => return IORingRuntimeExtract::NotFound(),
            IORingRegistryRemove::NotReady() => {
                trace2(
                    b"removing completer; cidx=%d, cid=%d, not ready\n",
                    completer.cidx(),
                    completer.cid(),
                );

                return IORingRuntimeExtract::NotCompleted();
            }
        };

        let value = match completion.result() {
            Some(value) => value,
            None => return IORingRuntimeExtract::NotCompleted(),
        };

        trace3(
            b"removing completer; cidx=%d, cid=%d, res=%d\n",
            completer.cidx(),
            completer.cid(),
            value,
        );

        IORingRuntimeExtract::Succeeded(value)
    }
}

impl IORingRuntimeContext {
    pub fn extract(&mut self, completer: &IORingCompleterRef) -> IORingRuntimeExtract {
        unsafe { (*self.runtime).extract(completer) }
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
    fn submit<T>(&mut self, task: &IORingTaskRef, entry: IORingSubmitEntry<T>) -> IORingRuntimeSubmit
    where
        T: IORingSubmitBuffer,
    {
        trace1(b"appending completer to registry; tid=%d\n", task.tid());
        let completer = match self.registry.append_completer(task.clone()) {
            IORingRegistryAppend::Succeeded(completer) => completer,
            IORingRegistryAppend::NotEnoughSlots() => return IORingRuntimeSubmit::NotEnoughSlots(),
            IORingRegistryAppend::InternallyFailed() => return IORingRuntimeSubmit::InternallyFailed(),
        };

        trace2(
            b"submitting op with uring; cidx=%d, cid=%d\n",
            completer.cidx(),
            completer.cid(),
        );

        let err = match self.submitter.submit(completer.encode(), [entry]) {
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
            IORingRegistryRemove::Succeeded(_) => IORingRuntimeSubmit::SubmissionFailed(err),
            _ => IORingRuntimeSubmit::InternallyFailed(),
        }
    }
}

impl IORingRuntimeContext {
    pub fn submit<T>(&mut self, entry: IORingSubmitEntry<T>) -> IORingRuntimeSubmit
    where
        T: IORingSubmitBuffer,
    {
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
        // we need to consolidate the ring first
        let ring = match IORing::join(self.submitter, self.completer) {
            IORingJoin::Succeeded(ring) => ring,
            IORingJoin::MismatchedDescriptor(_, _) => return IORingRuntimeShutdown::ConsolidationFailed(),
        };

        // to call final shutdown
        match ring.shutdown() {
            IORingShutdown::Succeeded() => IORingRuntimeShutdown::Succeeded(),
            IORingShutdown::Failed() => IORingRuntimeShutdown::ShutdownFailed(),
        }
    }
}
