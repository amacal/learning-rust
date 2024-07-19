use ::core::future::Future;
use ::core::pin::Pin;
use ::core::task::Context;
use ::core::task::Poll;

use super::mem::*;
use super::ops::*;
use super::token::*;
use crate::core::*;
use crate::trace::*;
use crate::uring::*;

impl IORuntimeOps {
    pub fn open_file<'a, TPath>(&mut self, path: &'a TPath) -> FileOpen<'a, TPath>
    where
        TPath: AsNullTerminatedRef,
    {
        FileOpen {
            path: path,
            token: None,
        }
    }

    pub fn close_file(&mut self, descriptor: FileDescriptor) -> FileClose {
        FileClose {
            descriptor: descriptor,
            token: None,
        }
    }

    pub fn read_file<TBuffer>(&mut self, file: &FileDescriptor, buffer: TBuffer, offset: u64) -> FileRead<TBuffer> {
        FileRead {
            fd: file.value,
            buffer: Some(buffer),
            offset: offset,
            token: None,
        }
    }
}

pub struct FileDescriptor {
    value: u32,
}

pub struct FileOpen<'a, TPath>
where
    TPath: AsNullTerminatedRef,
{
    path: &'a TPath,
    token: Option<IORingTaskToken>,
}

#[allow(dead_code)]
pub enum FileOpenResult {
    Succeeded(FileDescriptor),
    OperationFailed(i32),
    InternallyFailed(),
}

impl<'a, TPath> Future for FileOpen<'a, TPath>
where
    TPath: AsNullTerminatedRef,
{
    type Output = FileOpenResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling file-open; addr=%x\n", this.path.as_ptr());

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
                    return Poll::Ready(FileOpenResult::OperationFailed(result));
                }

                return Poll::Ready(FileOpenResult::Succeeded(FileDescriptor { value: result as u32 }));
            }

            None => {
                let op = IORingSubmitEntry::open_at(this.path.as_ptr());
                let token = match IORingTaskToken::submit(cx.waker(), op) {
                    Some(token) => token,
                    None => return Poll::Ready(FileOpenResult::InternallyFailed()),
                };

                this.token = Some(token);
            }
        }

        Poll::Pending
    }
}

pub struct FileClose {
    descriptor: FileDescriptor,
    token: Option<IORingTaskToken>,
}

#[allow(dead_code)]
pub enum FileCloseResult {
    Succeeded(),
    OperationFailed(i32),
    InternallyFailed(),
}

impl Future for FileClose {
    type Output = FileCloseResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling file-close; fd=%d\n", this.descriptor.value);

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
                    return Poll::Ready(FileCloseResult::OperationFailed(result));
                }

                return Poll::Ready(FileCloseResult::Succeeded());
            }

            None => {
                let op = IORingSubmitEntry::close(this.descriptor.value);
                let token = match IORingTaskToken::submit(cx.waker(), op) {
                    Some(token) => token,
                    None => return Poll::Ready(FileCloseResult::InternallyFailed()),
                };

                this.token = Some(token);
            }
        }

        Poll::Pending
    }
}

pub struct FileRead<TBuffer> {
    fd: u32,
    offset: u64,
    buffer: Option<TBuffer>,
    token: Option<IORingTaskToken>,
}

#[allow(dead_code)]
pub enum FileReadResult<TBuffer> {
    Succeeded(TBuffer, u32),
    OperationFailed(TBuffer, i32),
    InternallyFailed(),
}

impl<TBuffer> Future for FileRead<TBuffer>
where
    TBuffer: IORingSubmitBuffer + Unpin,
{
    type Output = FileReadResult<TBuffer>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling file-read; fd=%d\n", this.fd);

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
                    None => return Poll::Ready(FileReadResult::InternallyFailed()),
                };

                if result < 0 {
                    return Poll::Ready(FileReadResult::OperationFailed(buffer, result));
                }

                return Poll::Ready(FileReadResult::Succeeded(buffer, result as u32));
            }

            None => {
                let (buf, len) = match &this.buffer {
                    Some(value) => value.extract(),
                    None => return Poll::Ready(FileReadResult::InternallyFailed()),
                };

                let op = IORingSubmitEntry::read(this.fd, buf, len, this.offset);
                let token = match IORingTaskToken::submit(cx.waker(), op) {
                    Some(token) => token,
                    None => return Poll::Ready(FileReadResult::InternallyFailed()),
                };

                this.token = Some(token);
            }
        }

        Poll::Pending
    }
}
