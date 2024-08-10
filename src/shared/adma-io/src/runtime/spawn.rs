use ::core::future::Future;
use ::core::marker::PhantomData;
use ::core::pin::Pin;
use ::core::task::Context;
use ::core::task::Poll;

use super::callable::*;
use super::ops::*;
use super::pollable::*;
use super::refs::*;
use super::token::*;
use crate::heap::*;
use crate::trace::*;

pub struct Spawn {
    pub task: IORingTaskRef,
    pub callable: Option<PollableTarget>,
}

unsafe impl Send for Spawn {}

impl Future for Spawn {
    type Output = Result<(), ()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let callable = match self.callable.take() {
            Some(callable) => callable,
            None => return Poll::Ready(Err(())),
        };

        match IORingTaskToken::spawn(cx.waker(), self.task, callable) {
            true => {
                trace0(b"task=%d; spawned\n");
                Poll::Ready(Ok(()))
            }
            false => {
                trace0(b"task=%d; not spawned\n");
                Poll::Ready(Err(()))
            }
        }
    }
}

pub struct SpawnCPU<'a, F, R, E>
where
    F: Unpin,
    R: Unpin,
{
    pub ctx: Smart<IORuntimeContext>,
    pub task: Option<CallableTarget>,
    pub queued: Option<IORingTaskToken>,
    pub executed: Option<IORingTaskToken>,
    pub phantom: PhantomData<(&'a F, R, E)>,
}

pub enum SpawnCPUResult<R, E> {
    Succeeded(Option<Result<R, E>>),
    OperationFailed(),
    InternallyFailed(),
}

impl<'a, F, R, E> Future for SpawnCPU<'a, F, R, E>
where
    F: FnOnce() -> Result<R, E> + Unpin,
    R: Unpin,
    E: Unpin,
{
    type Output = SpawnCPUResult<R, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace0(b"# polling spawn-cpu\n");

        if let Some(token) = this.queued.take() {
            trace0(b"# polling spawn-cpu; extracting queued\n");
            let result = match token.extract(cx.waker()) {
                Ok(value) => Some(value),
                Err(token) => {
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
                Ok(value) => Some(value),
                Err(token) => {
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
                        Some(task) => match task.result::<16, F, R, E>(&mut this.ctx.heap) {
                            Ok(value) => SpawnCPUResult::Succeeded(value),
                            Err(_) => SpawnCPUResult::InternallyFailed(),
                        },
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

impl<'a, F, R, E> Drop for SpawnCPU<'a, F, R, E>
where
    F: Unpin,
    R: Unpin,
{
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            let (ptr, len) = task.as_ref().as_ptr();
            trace2(b"callable; releasing task, heap=%x, size=%d\n", ptr, len);
            task.release(&mut self.ctx.heap);
        }
    }
}
