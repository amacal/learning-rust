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
            ops: self.duplicate(),
            fd: descriptor.as_fd(),
        }
    }
}

struct CloseFuture {
    fd: u32,
    ops: IORuntimeOps,
    token: Option<IORingTaskToken>,
}

impl Future for CloseFuture {
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
