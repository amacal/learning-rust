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

impl FileDescriptor {
    pub fn new(value: u32) -> Self {
        Self { value: value }
    }
}

pub trait AsClosableFileDescriptor {
    fn as_file_descriptor(self) -> u32;
}

impl AsClosableFileDescriptor for FileDescriptor {
    fn as_file_descriptor(self) -> u32 {
        self.value
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

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
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
