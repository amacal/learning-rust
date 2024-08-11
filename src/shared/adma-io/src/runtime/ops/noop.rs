use ::core::future::*;
use ::core::pin::*;
use ::core::task::*;

use super::*;
use crate::runtime::token::*;
use crate::trace::*;

struct NoopFuture {
    ops: IORuntimeOps,
    token: Option<IORingTaskToken>,
}

impl IORuntimeOps {
    pub fn noop(&self) -> impl Future<Output = Result<(), Option<i32>>> {
        NoopFuture {
            ops: self.duplicate(),
            token: None,
        }
    }
}

impl Future for NoopFuture {
    type Output = Result<(), Option<i32>>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling noop; tid=%d\n", this.ops.tid());

        let op = IORingSubmitEntry::Noop();
        let (token, poll) = match this.token.take() {
            None => match this.ops.submit(op) {
                None => (None, Poll::Ready(Err(None))),
                Some(token) => (Some(token), Poll::Pending),
            },
            Some(token) => match token.extract(&mut this.ops.ctx) {
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
