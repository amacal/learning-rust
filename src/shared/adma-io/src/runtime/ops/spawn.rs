use ::core::future::*;
use ::core::marker::*;
use ::core::ops::*;
use ::core::pin::*;
use ::core::task::*;

use super::*;

impl IORuntimeOps {
    pub fn spawn<'a, TFnOnce, TFuture>(
        &mut self,
        call: TFnOnce,
    ) -> impl Future<Output = Result<(), Option<i32>>> + Send + 'a
    where
        TFuture: Future<Output = Option<&'static [u8]>> + Send + 'a,
        TFnOnce: FnOnce(IORuntimeOps) -> TFuture + Unpin + Send + 'a,
    {
        SpawnFuture {
            handle: self.handle(),
            call: Some(call),
        }
    }
}

struct SpawnFuture<TFnOnce, TFuture>
where
    TFuture: Future<Output = Option<&'static [u8]>> + Send,
    TFnOnce: FnOnce(IORuntimeOps) -> TFuture + Unpin + Send,
{
    handle: IORuntimeHandle,
    call: Option<TFnOnce>,
}

unsafe impl<TFnOnce, TFuture> Send for SpawnFuture<TFnOnce, TFuture>
where
    TFuture: Future<Output = Option<&'static [u8]>> + Send,
    TFnOnce: FnOnce(IORuntimeOps) -> TFuture + Unpin + Send,
{
}

impl<TFnOnce, TFuture> Future for SpawnFuture<TFnOnce, TFuture>
where
    TFuture: Future<Output = Option<&'static [u8]>> + Send,
    TFnOnce: FnOnce(IORuntimeOps) -> TFuture + Unpin + Send,
{
    type Output = Result<(), Option<i32>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling spawn; tid=%d\n", this.handle.task.tid());

        let callback = match this.call.take() {
            Some(callback) => callback,
            None => return Poll::Ready(Err(None)),
        };

        match IORuntimeContext::spawn(&mut this.handle.ctx, callback, cx) {
            Some(_) => (),
            None => return Poll::Ready(Err(None)),
        };

        Poll::Ready(Ok(()))
    }
}
