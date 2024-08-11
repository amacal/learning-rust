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
            handle: self.handle(),
            token: None,
        }
    }
}

struct TimeoutFuture {
    handle: IORuntimeHandle,
    timespec: timespec,
    token: Option<IORingTaskToken>,
}

impl Future for TimeoutFuture {
    type Output = Result<(), Option<i32>>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling timeout; tid=%d\n", this.handle.task.tid());

        let op = IORingSubmitEntry::timeout(&this.timespec);
        let (token, poll) = match this.token.take() {
            None => match this.handle.submit(op) {
                None => (None, Poll::Ready(Err(None))),
                Some(token) => (Some(token), Poll::Pending),
            },
            Some(token) => match token.extract(&mut this.handle.ctx) {
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
