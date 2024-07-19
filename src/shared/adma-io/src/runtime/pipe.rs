use ::core::future::Future;
use ::core::pin::Pin;
use ::core::task::Context;
use ::core::task::Poll;

use super::mem::*;
use super::token::*;
use crate::heap::*;
use crate::kernel::*;
use crate::syscall::*;
use crate::trace::*;
use crate::uring::*;
use super::ops::*;

pub enum CreatePipe {
    Succeeded((ReadPipeDescriptor, WritePipeDescriptor)),
    Failed(isize),
}

impl IORuntimeOps {
    pub fn create_pipe(&self) -> CreatePipe {
        let mut pipefd = [0; 2];
        let ptr = pipefd.as_mut_ptr();

        let flags = O_DIRECT;
        let result = sys_pipe2(ptr, flags);

        if result == 0 {
            CreatePipe::Succeeded((
                ReadPipeDescriptor { value: pipefd[0] },
                WritePipeDescriptor { value: pipefd[1] },
            ))
        } else {
            CreatePipe::Failed(result)
        }
    }

    pub fn close_pipe(&self, descriptor: impl PipeClosable) -> PipeClose {
        PipeClose {
            fd: descriptor.as_fd(),
            token: None,
        }
    }

    pub fn write_pipe<T>(&mut self, descriptor: &WritePipeDescriptor, buffer: T) -> PipeWrite<T> {
        PipeWrite {
            fd: descriptor.value,
            buffer: Some(buffer),
            token: None,
        }
    }

    pub fn read_pipe(&mut self, descriptor: &ReadPipeDescriptor, buffer: Droplet<Heap>) -> PipeRead {
        PipeRead {
            fd: descriptor.value,
            buffer: Some(buffer),
            token: None,
        }
    }
}

pub trait PipeClosable {
    fn as_fd(self) -> u32;
}

pub struct ReadPipeDescriptor {
    value: u32,
}

impl PipeClosable for ReadPipeDescriptor {
    fn as_fd(self) -> u32 {
        self.value
    }
}

pub struct WritePipeDescriptor {
    value: u32,
}

impl WritePipeDescriptor {
    pub fn at(fd: u32) -> Self {
        Self { value: fd }
    }
}

impl PipeClosable for WritePipeDescriptor {
    fn as_fd(self) -> u32 {
        self.value
    }
}

pub struct PipeWrite<T> {
    fd: u32,
    buffer: Option<T>,
    token: Option<IORingTaskToken>,
}

#[allow(dead_code)]
pub enum PipeWriteResult<T> {
    Succeeded(T, u32),
    OperationFailed(T, i32),
    InternallyFailed(),
}

impl<T: IORingSubmitBuffer + Unpin> Future for PipeWrite<T> {
    type Output = PipeWriteResult<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling pipe-write; fd=%d\n", this.fd);

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
                    None => return Poll::Ready(PipeWriteResult::InternallyFailed()),
                };

                if result < 0 {
                    return Poll::Ready(PipeWriteResult::OperationFailed(buffer, result));
                }

                Poll::Ready(PipeWriteResult::Succeeded(buffer, result as u32))
            }

            None => {
                let (buf, len) = match &this.buffer {
                    Some(value) => value.extract(),
                    None => return Poll::Ready(PipeWriteResult::InternallyFailed()),
                };

                let op = IORingSubmitEntry::write(this.fd, buf, len, 0);
                let token = match IORingTaskToken::submit(cx.waker(), op) {
                    Some(token) => token,
                    None => return Poll::Ready(PipeWriteResult::InternallyFailed()),
                };

                this.token = Some(token);
                Poll::Pending
            }
        }
    }
}

pub struct PipeRead {
    fd: u32,
    buffer: Option<Droplet<Heap>>,
    token: Option<IORingTaskToken>,
}

#[allow(dead_code)]
pub enum PipeReadResult {
    Succeeded(Droplet<Heap>, u32),
    OperationFailed(Droplet<Heap>, i32),
    InternallyFailed(),
}

impl Future for PipeRead {
    type Output = PipeReadResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling pipe-read; fd=%d\n", this.fd);

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
                    None => return Poll::Ready(PipeReadResult::InternallyFailed()),
                };

                if result < 0 {
                    return Poll::Ready(PipeReadResult::OperationFailed(buffer, result));
                }

                return Poll::Ready(PipeReadResult::Succeeded(buffer, result as u32));
            }

            None => {
                let (buf, len) = match &this.buffer {
                    Some(value) => value.extract(),
                    None => return Poll::Ready(PipeReadResult::InternallyFailed()),
                };

                let op = IORingSubmitEntry::read(this.fd, buf, len, 0);
                let token = match IORingTaskToken::submit(cx.waker(), op) {
                    Some(token) => token,
                    None => return Poll::Ready(PipeReadResult::InternallyFailed()),
                };

                this.token = Some(token);
            }
        }

        Poll::Pending
    }
}

pub struct PipeClose {
    fd: u32,
    token: Option<IORingTaskToken>,
}

#[allow(dead_code)]
pub enum PipeCloseResult {
    Succeeded(),
    OperationFailed(i32),
    InternallyFailed(),
}

impl Future for PipeClose {
    type Output = PipeCloseResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling pipe-close; fd=%d\n", this.fd);

        match this.token.take() {
            Some(token) => {
                let result = match token.extract(cx.waker()) {
                    IORingTaskTokenExtract::Succeeded(value) => value,
                    IORingTaskTokenExtract::Failed(token) => {
                        this.token = Some(token);
                        return Poll::Pending;
                    }
                };

                if result < 0 {
                    return Poll::Ready(PipeCloseResult::OperationFailed(result));
                }

                return Poll::Ready(PipeCloseResult::Succeeded());
            }

            None => {
                let op = IORingSubmitEntry::close(this.fd);
                let token = match IORingTaskToken::submit(cx.waker(), op) {
                    Some(token) => token,
                    None => return Poll::Ready(PipeCloseResult::InternallyFailed()),
                };

                this.token = Some(token);
            }
        }

        Poll::Pending
    }
}
