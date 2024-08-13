use ::core::future::*;
use ::core::task::*;

use super::ops::*;
use super::pool::*;
use super::refs::*;
use super::registry::*;
use crate::heap::*;
use crate::trace::*;
use crate::uring::*;

pub struct IORingRuntime {
    iteration: usize,
    ctx: Smart<IORuntimeContext>,
    entries: [IORingCompleteEntry; 16],
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
            Ok(registry) => registry.droplet(),
            Err(_) => return IORingRuntimeAllocate::RegistryAllocationFailed(),
        };

        // I/O Ring needs initialization
        let ring = match IORing::init(32) {
            Ok(ring) => ring.droplet(),
            Err(_) => return IORingRuntimeAllocate::RingAllocationFailed(),
        };

        let threads = match IORuntimePool::allocate() {
            Ok(threads) => threads.droplet(),
            Err(_) => return IORingRuntimeAllocate::PoolAllocationFailed(),
        };

        let ctx = match IORuntimeContext::allocate(ring, threads, registry) {
            Some(ctx) => ctx,
            None => return IORingRuntimeAllocate::PoolAllocationFailed(),
        };

        // if everying is ready we just need to collect created components
        let runtime = Self {
            iteration: 0,
            ctx: ctx,
            entries: [IORingCompleteEntry::default(); 16],
        };

        IORingRuntimeAllocate::Succeeded(runtime)
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
        let tasks = self.ctx.registry.tasks();
        let completers = self.ctx.registry.completers();

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
            match self.ctx.receive(&mut self.entries) {
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

        match self.ctx.flush() {
            IORingSubmit::Succeeded(_) => (),
            _ => return IORingRuntimeTick::InternallyFailed(),
        }

        IORingRuntimeTick::Succeeded()
    }

    fn complete(&mut self, completer: &IORingCompleterRef, entry: &IORingCompleteEntry) -> IORingRuntimeTick {
        trace2(b"looking for completions; cidx=%d, cid=%d\n", completer.cidx(), completer.cid());

        // complete received completer idx, it will return idx, id, readiness and completers of the found task
        let (task, ready, mut cnt) = match self.ctx.registry.complete(completer, entry.res) {
            Ok((task, ready, cnt)) => (task, ready, cnt),
            Err(_) => return IORingRuntimeTick::InternallyFailed(),
        };

        if !ready {
            // when task is not yet ready we need to poll it again
            let (_, completions) = match self.ctx.poll(&task) {
                Some((cnt, Poll::Ready(val))) => (val, cnt),
                Some((_, Poll::Pending)) => return IORingRuntimeTick::Pending(task),
                None => return IORingRuntimeTick::InternallyFailed(),
            };

            // completions may have changed after polling
            cnt = completions;
        }

        if cnt > 0 {
            // completers indicate that task is draining
            return IORingRuntimeTick::Draining(task);
        }

        // no draining and readiness, so remove the task
        let result = match self.ctx.registry.remove_task(&task) {
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
    pub fn run<'a, TFuture, TFnOnce>(&mut self, callback: TFnOnce) -> IORingRuntimeRun
    where
        TFuture: Future<Output = Option<&'static [u8]>> + Send + 'a,
        TFnOnce: FnOnce(IORuntimeOps) -> TFuture + Unpin + Send + 'a,
    {
        let mut result: Option<&'static [u8]> = None;

        let spawned = match IORuntimeContext::spawn(&mut self.ctx, callback) {
            None => return IORingRuntimeRun::InternallyFailed(),
            Some((Some(task), _)) => Some(task),
            Some((_, res)) => {
                result = res;
                None
            }
        };

        match self.ctx.flush() {
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

        let callback = |ops: IORuntimeOps| async move {
            match ops.timeout(0, 1).await {
                Ok(()) => assert!(true),
                Err(_) => assert!(false),
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
