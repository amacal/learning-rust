use ::core::future::Future;
use ::core::pin::Pin;
use ::core::task::Context;
use ::core::task::Poll;

use super::token::*;
use crate::kernel::*;
use crate::trace::*;
use crate::uring::*;

pub fn timeout(seconds: u32) -> Timeout {
    Timeout {
        timespec: timespec {
            tv_sec: seconds as i64,
            tv_nsec: 0,
        },
        token: None,
    }
}

pub struct Timeout {
    timespec: timespec,
    token: Option<IORingTaskToken>,
}

#[allow(dead_code)]
pub enum TimeoutResult {
    Succeeded(),
    OperationFailed(i32),
    InternallyFailed(),
}

impl Future for Timeout {
    type Output = TimeoutResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling timeout; timespec=%x\n", &this.timespec as *const timespec as *const ());

        match this.token.take() {
            Some(token) => {
                let result = match token.extract(cx.waker()) {
                    IORingTaskTokenExtract::Succeeded(value) => value,
                    IORingTaskTokenExtract::Failed(token) => {
                        this.token = Some(token);
                        return Poll::Pending;
                    }
                };

                if result != -62 {
                    return Poll::Ready(TimeoutResult::OperationFailed(result));
                }

                return Poll::Ready(TimeoutResult::Succeeded());
            }

            None => {
                let timespec = &this.timespec as *const timespec;
                let op = IORingSubmitEntry::timeout(timespec);

                let token = match IORingTaskToken::submit(cx.waker(), op) {
                    Some(token) => token,
                    None => return Poll::Ready(TimeoutResult::InternallyFailed()),
                };

                this.token = Some(token);
            }
        }

        Poll::Pending
    }
}
