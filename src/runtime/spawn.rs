use ::core::future::Future;
use ::core::marker::PhantomData;
use ::core::pin::Pin;
use ::core::task::Context;
use ::core::task::Poll;

use super::erase::*;
use super::pin::*;
use super::token::*;
use crate::trace::*;

pub fn spawn<F>(target: F) -> Spawn
where
    F: Future<Output = Option<&'static [u8]>>,
{
    Spawn {
        task: match IORingPin::allocate(target) {
            IORingPinAllocate::Succeeded(task) => Some(task),
            IORingPinAllocate::AllocationFailed(_) => None,
        },
    }
}

pub fn spawn_cpu<F, R>(target: F) -> SpawnCPU<F, R>
where
    F: FnOnce() -> Result<R, Option<&'static [u8]>> + Unpin,
    R: Unpin,
{
    SpawnCPU {
        queued: None,
        executed: None,
        phantom: PhantomData::default(),
        task: match CallableTarget::allocate(target) {
            CallableTargetAllocate::Succeeded(target) => Some(target),
            CallableTargetAllocate::AllocationFailed(_) => None,
        },
    }
}

pub struct Spawn {
    task: Option<IORingPin>,
}

pub enum SpawnResult {
    Succeeded(),
    OperationFailed(),
    InternallyFailed(),
}

impl Future for Spawn {
    type Output = SpawnResult;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let task = match self.task.take() {
            Some(task) => task,
            None => return Poll::Ready(SpawnResult::InternallyFailed()),
        };

        match IORingTaskToken::spawn(cx.waker(), task) {
            true => {
                trace0(b"task=%d; spawned\n");
                Poll::Ready(SpawnResult::Succeeded())
            }
            false => {
                trace0(b"task=%d; not spawned\n");
                Poll::Ready(SpawnResult::OperationFailed())
            }
        }
    }
}

pub struct SpawnCPU<F, R>
where
    F: Unpin,
    R: Unpin,
{
    task: Option<CallableTarget<24>>,
    queued: Option<IORingTaskToken>,
    executed: Option<IORingTaskToken>,
    phantom: PhantomData<(F, R)>,
}

pub enum SpawnCPUResult<R> {
    Succeeded(Option<R>),
    OperationFailed(),
    InternallyFailed(),
}

impl<F, R> Future for SpawnCPU<F, R>
where
    F: FnOnce() -> Result<R, Option<&'static [u8]>> + Unpin,
    R: Unpin,
{
    type Output = SpawnCPUResult<R>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace0(b"# polling spawn-cpu\n");

        if let Some(token) = this.queued.take() {
            trace0(b"# polling spawn-cpu; extracting queued\n");
            let result = match token.extract(cx.waker()) {
                IORingTaskTokenExtract::Succeeded(value) => Some(value),
                IORingTaskTokenExtract::Failed(token) => {
                    this.queued = Some(token);
                    None
                }
            };

            if let Some(result) = result {
                return if result < 0 {
                    Poll::Ready(SpawnCPUResult::OperationFailed())
                } else {
                    trace1(b"# polling spawn-cpu; stage=queued, res=%d\n", result);
                    Poll::Pending
                };
            }
        }

        if let Some(token) = this.executed.take() {
            trace0(b"# polling spawn-cpu; extracting executed\n");
            let result = match token.extract(cx.waker()) {
                IORingTaskTokenExtract::Succeeded(value) => Some(value),
                IORingTaskTokenExtract::Failed(token) => {
                    this.executed = Some(token);
                    return Poll::Pending;
                }
            };

            if let Some(result) = result {
                return if result < 0 {
                    Poll::Ready(SpawnCPUResult::OperationFailed())
                } else {
                    trace1(b"# polling spawn-cpu; stage=executed, res=%d\n", result);
                    let result = match this.task.take() {
                        None => SpawnCPUResult::InternallyFailed(),
                        Some(task) => SpawnCPUResult::Succeeded(task.result::<F, R>()),
                    };

                    Poll::Ready(result)
                };
            }
        }

        if this.queued.is_some() || this.executed.is_some() {
            return Poll::Pending;
        }

        let task = match &this.task {
            Some(task) => task,
            None => return Poll::Ready(SpawnCPUResult::InternallyFailed()),
        };

        match IORingTaskToken::execute(cx.waker(), task) {
            Some((queued, executed)) => {
                trace2(b"callable; scheduled, qid=%d, eid=%d\n", queued.cid(), executed.cid());
                this.queued = Some(queued);
                this.executed = Some(executed);
                Poll::Pending
            }
            None => {
                trace0(b"callable not scheduled\n");
                Poll::Ready(SpawnCPUResult::OperationFailed())
            }
        }
    }
}

impl<F, R> Drop for SpawnCPU<F, R>
where
    F: Unpin,
    R: Unpin,
{
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            let (ptr, len) = (task.heap().ptr, task.heap().len);
            trace2(b"callable; releasing task, heap=%x, size=%d\n", ptr, len);
            task.release();
        }
    }
}
