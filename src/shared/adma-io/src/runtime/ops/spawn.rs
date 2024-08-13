use super::*;

impl IORuntimeOps {
    pub fn spawn<'a, TFnOnce, TFuture>(
        &self,
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

struct SpawnFuture<THandle, TFnOnce, TFuture>
where
    THandle: IORuntimeHandle + Unpin,
    TFuture: Future<Output = Option<&'static [u8]>> + Send,
    TFnOnce: FnOnce(IORuntimeOps) -> TFuture + Unpin + Send,
{
    handle: THandle,
    call: Option<TFnOnce>,
}

unsafe impl<THandle, TFnOnce, TFuture> Send for SpawnFuture<THandle, TFnOnce, TFuture>
where
    THandle: IORuntimeHandle + Unpin,
    TFuture: Future<Output = Option<&'static [u8]>> + Send,
    TFnOnce: FnOnce(IORuntimeOps) -> TFuture + Unpin + Send,
{
}

impl<THandle, TFnOnce, TFuture> Future for SpawnFuture<THandle, TFnOnce, TFuture>
where
    THandle: IORuntimeHandle + Unpin,
    TFuture: Future<Output = Option<&'static [u8]>> + Send,
    TFnOnce: FnOnce(IORuntimeOps) -> TFuture + Unpin + Send,
{
    type Output = Result<(), Option<i32>>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling spawn; tid=%d\n", this.handle.tid());

        let callback = match this.call.take() {
            Some(callback) => callback,
            None => return Poll::Ready(Err(None)),
        };

        match this.handle.spawn(callback) {
            Some(_) => (),
            None => return Poll::Ready(Err(None)),
        };

        Poll::Ready(Ok(()))
    }
}
