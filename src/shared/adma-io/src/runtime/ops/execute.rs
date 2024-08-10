use ::core::future::*;
use ::core::marker::*;
use ::core::pin::*;
use ::core::task::*;

use crate::runtime::callable::*;
use crate::runtime::ops::*;
use crate::runtime::token::*;
use crate::trace::*;

impl IORuntimeOps {
    pub fn execute<'a, TFnOnce, TResult, TError>(
        &mut self,
        target: TFnOnce,
    ) -> impl Future<Output = Result<Result<TResult, TError>, Option<i32>>> + 'a
    where
        TFnOnce: FnOnce() -> Result<TResult, TError> + Unpin + Send + 'a,
        TResult: Unpin + Send + 'a,
        TError: Unpin + Send + 'a,
    {
        ExecuteFuture::<'a, TFnOnce, TResult, TError> {
            queued: None,
            executed: None,
            phantom: PhantomData,
            ops: self.duplicate(),
            task: Some(CallableTarget::allocate(&mut self.ctx.heap, target)),
        }
    }
}

struct ExecuteFuture<'a, TFnOnce, TResult, TError>
where
    TFnOnce: FnOnce() -> Result<TResult, TError> + Unpin + Send + 'a,
    TResult: Unpin + Send,
    TError: Unpin + Send,
{
    ops: IORuntimeOps,
    task: Option<Result<CallableTarget, CallableError>>,
    queued: Option<IORingTaskToken>,
    executed: Option<IORingTaskToken>,
    phantom: PhantomData<&'a (TFnOnce, TResult, TError)>,
}

impl<'a, TFnOnce, TResult, TError> Future for ExecuteFuture<'a, TFnOnce, TResult, TError>
where
    TFnOnce: FnOnce() -> Result<TResult, TError> + Unpin + Send + 'a,
    TResult: Unpin + Send,
    TError: Unpin + Send,
{
    type Output = Result<Result<TResult, TError>, Option<i32>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling spawn-cpu; tid=%d\n", this.ops.tid());

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
                    Poll::Ready(Err(Some(result)))
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
                    Poll::Ready(Err(Some(result)))
                } else {
                    trace1(b"# polling spawn-cpu; stage=executed, res=%d\n", result);
                    match this.task.take() {
                        None | Some(Err(_)) => Poll::Ready(Err(None)),
                        Some(Ok(task)) => match task.result::<16, TFnOnce, TResult, TError>(&mut this.ops.ctx.heap) {
                            Ok(Some(value)) => Poll::Ready(Ok(value)),
                            Ok(None) | Err(_) => Poll::Ready(Err(None)),
                        },
                    }
                };
            }
        }

        if this.queued.is_some() || this.executed.is_some() {
            return Poll::Pending;
        }

        let task = match &this.task {
            Some(Ok(task)) => task,
            Some(Err(_)) | None => return Poll::Ready(Err(None)),
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
                Poll::Ready(Err(None))
            }
        }
    }
}

impl<'a, TFnOnce, TResult, TError> Drop for ExecuteFuture<'a, TFnOnce, TResult, TError>
where
    TFnOnce: FnOnce() -> Result<TResult, TError> + Unpin + Send + 'a,
    TResult: Unpin + Send + 'a,
    TError: Unpin + Send + 'a,
{
    fn drop(&mut self) {
        if let Some(Ok(task)) = self.task.take() {
            let (ptr, len) = task.as_ref().as_ptr();
            trace2(b"callable; releasing task, heap=%x, size=%d\n", ptr, len);
            task.release(&mut self.ops.ctx.heap);
        }
    }
}
