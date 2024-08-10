use ::core::future::*;
use ::core::marker::*;
use ::core::ops::*;
use ::core::pin::*;
use ::core::task::*;

use super::*;
use crate::runtime::callable::*;
use crate::runtime::file::*;
use crate::runtime::pollable::*;
use crate::runtime::spawn::*;

impl IORuntimeOps {
    pub fn spawn_io<'a, C, F>(&mut self, callback: C) -> Option<impl Future<Output = Result<(), ()>>>
    where
        F: Future<Output = Option<&'static [u8]>> + Send + 'a,
        C: FnOnce(IORuntimeOps) -> F + Unpin + Send + 'a,
    {
        let task = match self.ctx.registry.prepare_task() {
            Ok(val) => val,
            Err(_) => return None,
        };

        let ops = IORuntimeContext::ops(&mut self.ctx, task);
        let target = callback.call_once((ops,));

        Some(Spawn {
            task: task,
            callable: match PollableTarget::allocate(&mut self.ctx.heap_pool, target) {
                Some(task) => Some(task),
                None => None,
            },
        })
    }
}

impl IORuntimeOps {
    pub fn spawn_cpu<'a, F, R, E>(&mut self, target: F) -> Option<SpawnCPU<'a, F, R, E>>
    where
        F: FnOnce() -> Result<R, E> + Unpin + Send + 'a,
        R: Unpin + Send,
        E: Unpin + Send,
    {
        let task = match CallableTarget::allocate(&mut self.ctx.heap_pool, target) {
            Ok(target) => target,
            Err(_) => return None,
        };

        Some(SpawnCPU {
            ctx: self.ctx.duplicate(),
            queued: None,
            executed: None,
            phantom: PhantomData,
            task: Some(task),
        })
    }
}

impl IORuntimeOps {
    pub fn close(
        &mut self,
        descriptor: impl AsClosableFileDescriptor,
    ) -> impl Future<Output = Result<(), Option<i32>>> {
        FileClose {
            token: None,
            ops: self.duplicate(),
            fd: descriptor.as_file_descriptor(),
        }
    }
}

pub struct FileClose {
    fd: u32,
    ops: IORuntimeOps,
    token: Option<IORingTaskToken>,
}

impl Future for FileClose {
    type Output = Result<(), Option<i32>>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace2(b"# polling file-close; tid=%d, fd=%d\n", this.ops.tid(), this.fd);

        let op = IORingSubmitEntry::close(this.fd);
        let (token, poll) = match this.token.take() {
            None => match this.ops.submit(op) {
                None => (None, Poll::Ready(Err(None))),
                Some(token) => (Some(token), Poll::Pending),
            },
            Some(token) => match token.extract_ctx(&mut this.ops.ctx) {
                Err(token) => (Some(token), Poll::Pending),
                Ok(val) => match val {
                    val if val < 0 => (None, Poll::Ready(Err(Some(val)))),
                    _ => (None, Poll::Ready(Ok(()))),
                },
            },
        };

        this.token = token;
        poll
    }
}
