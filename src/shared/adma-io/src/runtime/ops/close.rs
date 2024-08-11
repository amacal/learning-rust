use ::core::future::*;
use ::core::pin::*;
use ::core::task::*;

use super::*;
use crate::runtime::file::*;

impl IORuntimeOps {
    pub fn close<TFileDescriptor>(
        &mut self,
        descriptor: TFileDescriptor,
    ) -> impl Future<Output = Result<(), Option<i32>>>
    where
        TFileDescriptor: AsFileDescriptor + AsClosableFileDescriptor,
    {
        CloseFuture {
            token: None,
            handle: self.handle(),
            fd: descriptor.as_fd(),
        }
    }
}

struct CloseFuture {
    fd: u32,
    handle: IORuntimeHandle,
    token: Option<IORingTaskToken>,
}

impl Future for CloseFuture {
    type Output = Result<(), Option<i32>>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace2(b"# polling file-close; tid=%d, fd=%d\n", this.handle.task.tid(), this.fd);

        let op = IORingSubmitEntry::close(this.fd);
        let (token, poll) = match this.token.take() {
            None => match this.handle.submit(op) {
                None => (None, Poll::Ready(Err(None))),
                Some(token) => (Some(token), Poll::Pending),
            },
            Some(token) => match token.extract(&mut this.handle.ctx) {
                Ok((None, Some(token))) => (Some(token), Poll::Pending),
                Ok((Some(val), None)) => match val {
                    val if val < 0 => (None, Poll::Ready(Err(Some(val)))),
                    _ => (None, Poll::Ready(Ok(()))),
                },
                Ok(_) => (None, Poll::Ready(Err(None))),
                Err(err) => (None, Poll::Ready(Err(err))),
            },
        };

        this.token = token;
        poll
    }
}
