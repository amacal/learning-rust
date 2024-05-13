use ::core::future::Future;
use ::core::mem;
use ::core::pin::Pin;
use ::core::task::Context;
use ::core::task::Poll;

use super::pin::*;
use crate::trace::*;
use super::token::*;

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
        let mut task = None;
        mem::swap(&mut task, &mut self.task);

        let task = match task {
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
