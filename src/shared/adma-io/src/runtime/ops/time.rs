use ::core::future::Future;
use ::core::pin::Pin;
use ::core::task::Context;
use ::core::task::Poll;

use super::*;
use crate::runtime::token::*;
use crate::trace::*;
use crate::uring::*;

impl IORuntimeOps {
    pub fn timeout(&mut self, seconds: u32, nanos: u32) -> impl Future<Output = Result<(), Option<i32>>> {
        TimeoutFuture {
            timespec: timespec {
                tv_sec: seconds as i64,
                tv_nsec: nanos as i64,
            },
            ops: self.duplicate(),
            token: None,
        }
    }
}

struct TimeoutFuture {
    ops: IORuntimeOps,
    timespec: timespec,
    token: Option<IORingTaskToken>,
}

impl Future for TimeoutFuture {
    type Output = Result<(), Option<i32>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling noop; tid=%d\n", this.ops.tid());

        let op = IORingSubmitEntry::timeout(&this.timespec);
        let (token, poll) = match this.token.take() {
            None => match this.ops.submit(op) {
                None => (None, Poll::Ready(Err(None))),
                Some(token) => (Some(token), Poll::Pending),
            },
            Some(token) => match token.extract_ctx(&mut this.ops.ctx) {
                Err(token) => (Some(token), Poll::Pending),
                Ok(val) if val == -62 => (None, Poll::Ready(Ok(()))),
                Ok(val) => (None, Poll::Ready(Err(Some(val)))),
            },
        };

        this.token = token;
        poll
    }
}
