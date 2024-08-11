use ::core::future::*;
use ::core::pin::*;
use ::core::task::*;

use super::*;
use crate::runtime::file::*;
use crate::runtime::mem::*;

impl IORuntimeOps {
    pub fn read<'a, TBuffer, TFileDescriptor>(
        &mut self,
        file: TFileDescriptor,
        buffer: &'a TBuffer,
    ) -> impl Future<Output = Result<u32, Option<i32>>> + 'a
    where
        TBuffer: IORingSubmitBuffer + Unpin + 'a,
        TFileDescriptor: AsFileDescriptor + AsReadableFileDescriptor,
    {
        ReadAtOffset {
            fd: file.as_fd(),
            handle: self.handle(),
            buffer: buffer,
            offset: 0,
            token: None,
        }
    }

    pub fn read_at_offset<'a, TBuffer, TFileDescriptor>(
        &mut self,
        file: TFileDescriptor,
        buffer: &'a TBuffer,
        offset: u64,
    ) -> impl Future<Output = Result<u32, Option<i32>>> + 'a
    where
        TBuffer: IORingSubmitBuffer + Unpin + 'a,
        TFileDescriptor: AsFileDescriptor + AsReadableAtOffsetFileDescriptor,
    {
        ReadAtOffset {
            fd: file.as_fd(),
            handle: self.handle(),
            buffer: buffer,
            offset: offset,
            token: None,
        }
    }
}

struct ReadAtOffset<'a, TBuffer> {
    fd: u32,
    offset: u64,
    handle: IORuntimeHandle,
    buffer: &'a TBuffer,
    token: Option<IORingTaskToken>,
}

impl<'a, TBuffer> Future for ReadAtOffset<'a, TBuffer>
where
    TBuffer: IORingSubmitBuffer + Unpin + 'a,
{
    type Output = Result<u32, Option<i32>>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace2(b"# polling file-read; tid=%d, fd=%d\n", this.handle.task.tid(), this.fd);

        let (buf, len) = this.buffer.extract();
        let op = IORingSubmitEntry::read(this.fd, buf, len, this.offset);

        let (token, poll) = match this.token.take() {
            None => match this.handle.submit(op) {
                None => (None, Poll::Ready(Err(None))),
                Some(token) => (Some(token), Poll::Pending),
            },
            Some(token) => match token.extract(&mut this.handle.ctx) {
                Ok((None, Some(token))) => (Some(token), Poll::Pending),
                Ok((Some(val), None)) if val < 0 => (None, Poll::Ready(Err(Some(val)))),
                Ok((Some(val), None)) => match u32::try_from(val) {
                    Ok(cnt) => (None, Poll::Ready(Ok(cnt))),
                    Err(_) => (None, Poll::Ready(Err(None))),
                },
                Ok(_) => (None, Poll::Ready(Err(None))),
                Err(err) => (None, Poll::Ready(Err(err))),
            },
        };

        this.token = token;
        poll
    }
}
