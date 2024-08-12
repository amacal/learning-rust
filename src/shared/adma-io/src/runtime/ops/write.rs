use ::core::future::*;
use ::core::pin::*;
use ::core::task::*;

use super::*;
use crate::runtime::file::*;
use crate::runtime::mem::*;

impl IORuntimeOps {
    pub fn write<'a, TBuffer, TFileDescriptor>(
        &mut self,
        file: TFileDescriptor,
        buffer: &'a TBuffer,
    ) -> impl Future<Output = Result<u32, Option<i32>>> + 'a
    where
        TBuffer: IORingSubmitBuffer + Unpin + 'a,
        TFileDescriptor: AsFileDescriptor + AsWrittableFileDescriptor,
    {
        WriteAtOffset {
            fd: file.as_fd(),
            handle: self.handle(),
            buffer: buffer,
            offset: 0,
            token: None,
        }
    }
}

pub struct WriteAtOffset<'a, THandle, TBuffer>
where
    THandle: IORuntimeHandle + Unpin,
    TBuffer: IORingSubmitBuffer + Unpin + 'a,
{
    fd: u32,
    offset: u64,
    handle: THandle,
    buffer: &'a TBuffer,
    token: Option<IORingTaskToken>,
}

impl<'a, THandle, TBuffer> Future for WriteAtOffset<'a, THandle, TBuffer>
where
    THandle: IORuntimeHandle + Unpin,
    TBuffer: IORingSubmitBuffer + Unpin + 'a,
{
    type Output = Result<u32, Option<i32>>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace2(b"# polling file-write; tid=%d, fd=%d\n", this.handle.tid(), this.fd);

        let (buf, len) = this.buffer.extract();
        let op = IORingSubmitEntry::write(this.fd, buf, len, this.offset);

        let (token, poll) = match this.token.take() {
            None => match this.handle.submit(op) {
                None => (None, Poll::Ready(Err(None))),
                Some(token) => (Some(token), Poll::Pending),
            },
            Some(token) => match token.extract(&mut this.handle) {
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
