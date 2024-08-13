use super::*;

impl IORuntimeOps {
    pub fn timeout(&self, seconds: u32, nanos: u32) -> impl Future<Output = Result<(), Option<i32>>> {
        TimeoutFuture {
            timespec: timespec {
                tv_sec: seconds as i64,
                tv_nsec: nanos as i64,
            },
            handle: self.handle(),
            token: None,
        }
    }
}

struct TimeoutFuture<THandle>
where
    THandle: IORuntimeHandle + Unpin,
{
    handle: THandle,
    timespec: timespec,
    token: Option<IORingTaskToken>,
}

impl<THandle> Future for TimeoutFuture<THandle>
where
    THandle: IORuntimeHandle + Unpin,
{
    type Output = Result<(), Option<i32>>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling timeout; tid=%d\n", this.handle.tid());

        let op = IORingSubmitEntry::timeout(&this.timespec);
        let (token, poll) = match this.token.take() {
            None => match this.handle.submit(op) {
                None => (None, Poll::Ready(Err(None))),
                Some(token) => (Some(token), Poll::Pending),
            },
            Some(token) => match token.extract(&mut this.handle) {
                Ok((None, Some(token))) => (Some(token), Poll::Pending),
                Ok((Some(val), None)) if val == -62 => (None, Poll::Ready(Ok(()))),
                Ok((Some(val), None)) => (None, Poll::Ready(Err(Some(val)))),
                Ok(_) => (None, Poll::Ready(Err(None))),
                Err(err) => (None, Poll::Ready(Err(err))),
            },
        };

        this.token = token;
        poll
    }
}
