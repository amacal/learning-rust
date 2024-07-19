use ::core::future::Future;
use ::core::pin::Pin;
use ::core::task::Context;
use ::core::task::Poll;

use super::mem::*;
use super::ops::*;
use super::token::*;
use crate::trace::*;
use crate::uring::*;

impl IORuntimeOps {
    pub fn open_stdout(&self) -> StdOutDescriptor {
        StdOutDescriptor { value: 1 }
    }

    pub fn write_stdout<T>(&mut self, file: &StdOutDescriptor, buffer: T) -> StdOutWrite<T> {
        StdOutWrite {
            fd: file.value,
            ops: self.duplicate(),
            buffer: Some(buffer),
            token: None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct StdOutDescriptor {
    pub value: u32,
}

pub struct StdOutWrite<T> {
    fd: u32,
    ops: IORuntimeOps,
    buffer: Option<T>,
    token: Option<IORingTaskToken>,
}

pub enum StdOutWriteResult<T> {
    Succeeded(T, u32),
    OperationFailed(T, i32),
    InternallyFailed(),
}

impl<T: IORingSubmitBuffer + Unpin> Future for StdOutWrite<T> {
    type Output = StdOutWriteResult<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling stdout-write; fd=%d\n", this.fd);

        match this.token.take() {
            Some(token) => {
                let result = match token.extract(cx.waker()) {
                    IORingTaskTokenExtract::Succeeded(value) => value,
                    IORingTaskTokenExtract::Failed(token) => {
                        this.token = Some(token);
                        return Poll::Pending;
                    }
                };

                let buffer = match this.buffer.take() {
                    Some(value) => value,
                    None => return Poll::Ready(StdOutWriteResult::InternallyFailed()),
                };

                if result < 0 {
                    return Poll::Ready(StdOutWriteResult::OperationFailed(buffer, result));
                }

                Poll::Ready(StdOutWriteResult::Succeeded(buffer, result as u32))
            }

            None => {
                let (buf, len) = match &this.buffer {
                    Some(value) => value.extract(),
                    None => return Poll::Ready(StdOutWriteResult::InternallyFailed()),
                };

                let op = IORingSubmitEntry::write(this.fd, buf, len, 0);
                let token = match IORingTaskToken::submit(cx.waker(), op) {
                    Some(token) => token,
                    None => return Poll::Ready(StdOutWriteResult::InternallyFailed()),
                };

                this.token = Some(token);
                Poll::Pending
            }
        }
    }
}
