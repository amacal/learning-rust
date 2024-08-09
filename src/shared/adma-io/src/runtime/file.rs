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
    pub fn open_file<'a, TPath>(
        &mut self,
        path: &'a TPath,
    ) -> impl Future<Output = Result<FileDescriptor, Option<i32>>> + 'a
    where
        TPath: AsNullTerminatedRef,
    {
        FileOpen {
            path: path,
            token: None,
            ops: self.duplicate(),
        }
    }

    pub fn close_file(&mut self, descriptor: FileDescriptor) -> impl Future<Output = Result<(), Option<i32>>> {
        FileClose {
            token: None,
            ops: self.duplicate(),
            descriptor: descriptor,
        }
    }

    pub fn read_file<'a, TBuffer>(
        &mut self,
        file: &FileDescriptor,
        buffer: &'a TBuffer,
        offset: u64,
    ) -> impl Future<Output = Result<u32, Option<i32>>> + 'a
    where
        TBuffer: IORingSubmitBuffer + Unpin + 'a,
    {
        FileRead {
            fd: file.value,
            ops: self.duplicate(),
            buffer: buffer,
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
    ops: IORuntimeOps,
    token: Option<IORingTaskToken>,
}

impl<'a, TPath> Future for FileOpen<'a, TPath>
where
    TPath: AsNullTerminatedRef,
{
    type Output = Result<FileDescriptor, Option<i32>>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace2(b"# polling file-open; tid=%d, addr=%x\n", this.ops.tid(), this.path.as_ptr());

        let op = IORingSubmitEntry::open_at(this.path.as_ptr());
        let (token, poll) = match this.token.take() {
            None => match this.ops.submit(op) {
                None => (None, Poll::Ready(Err(None))),
                Some(token) => (Some(token), Poll::Pending),
            },
            Some(token) => match token.extract_ctx(&mut this.ops.ctx) {
                Err(token) => (Some(token), Poll::Pending),
                Ok(val) => match val {
                    val if val < 0 => (None, Poll::Ready(Err(Some(val)))),
                    val => match u32::try_from(val) {
                        Ok(fd) => (None, Poll::Ready(Ok(FileDescriptor { value: fd }))),
                        Err(_) => (None, Poll::Ready(Err(None))),
                    },
                },
            },
        };

        this.token = token;
        poll
    }
}

pub struct FileClose {
    ops: IORuntimeOps,
    descriptor: FileDescriptor,
    token: Option<IORingTaskToken>,
}

impl Future for FileClose {
    type Output = Result<(), Option<i32>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace2(b"# polling file-close; tid=%d, fd=%d\n", this.ops.tid(), this.descriptor.value);

        let op = IORingSubmitEntry::close(this.descriptor.value);
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

pub struct FileRead<'a, TBuffer> {
    fd: u32,
    offset: u64,
    ops: IORuntimeOps,
    buffer: &'a TBuffer,
    token: Option<IORingTaskToken>,
}

impl<'a, TBuffer> Future for FileRead<'a, TBuffer>
where
    TBuffer: IORingSubmitBuffer + Unpin + 'a,
{
    type Output = Result<u32, Option<i32>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace2(b"# polling file-read; tid=%d, fd=%d\n", this.ops.tid(), this.fd);

        let (buf, len) = this.buffer.extract();
        let op = IORingSubmitEntry::read(this.fd, buf, len, this.offset);

        let (token, poll) = match this.token.take() {
            None => match this.ops.submit(op) {
                None => (None, Poll::Ready(Err(None))),
                Some(token) => (Some(token), Poll::Pending),
            },
            Some(token) => match token.extract_ctx(&mut this.ops.ctx) {
                Err(token) => (Some(token), Poll::Pending),
                Ok(val) => match val {
                    val if val < 0 => (None, Poll::Ready(Err(Some(val)))),
                    val => match u32::try_from(val) {
                        Ok(cnt) => (None, Poll::Ready(Ok(cnt))),
                        Err(_) => (None, Poll::Ready(Err(None))),
                    },
                },
            },
        };

        this.token = token;
        poll
    }
}
