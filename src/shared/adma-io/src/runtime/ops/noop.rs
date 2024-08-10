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
    pub fn noop(&self) -> impl Future<Output = Result<(), ()>> {
        NoopFuture {
            ops: self.duplicate(),
            token: None,
        }
    }
}

impl Future for NoopFuture {
    type Output = Result<(), ()>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling noop; tid=%d\n", this.ops.tid());

        let (token, poll) = match this.token.take() {
            None => match this.ops.submit(IORingSubmitEntry::Noop()) {
                None => (None, Poll::Ready(Err(()))),
                Some(token) => (Some(token), Poll::Pending),
            },
            Some(token) => match token.extract_ctx(&mut this.ops.ctx) {
                Err(token) => (Some(token), Poll::Pending),
                Ok(_) => (None, Poll::Ready(Ok(()))),
            },
        };

        this.token = token;
        poll
    }
}
