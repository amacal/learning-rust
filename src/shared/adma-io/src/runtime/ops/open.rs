use file::FileDescriptor;

use super::*;

impl IORuntimeOps {
    pub fn open_at<'a, TPath>(
        &mut self,
        path: &'a TPath,
    ) -> impl Future<Output = Result<impl FileDescriptor + ReadableAtOffset + Closable + Copy, Option<i32>>> + 'a
    where
        TPath: AsNullTerminatedRef,
    {
        OpenAtFuture {
            path: path,
            token: None,
            handle: self.handle(),
        }
    }
}

struct OpenAtFuture<'a, THandle, TPath>
where
    THandle: IORuntimeHandle + Unpin,
    TPath: AsNullTerminatedRef,
{
    path: &'a TPath,
    handle: THandle,
    token: Option<IORingTaskToken>,
}

impl<'a, THandle, TPath> Future for OpenAtFuture<'a, THandle, TPath>
where
    THandle: IORuntimeHandle + Unpin,
    TPath: AsNullTerminatedRef,
{
    type Output = Result<FileDescriptorValue, Option<i32>>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace2(b"# polling file-open; tid=%d, addr=%x\n", this.handle.tid(), this.path.as_ptr());

        let op = IORingSubmitEntry::open_at(this.path.as_ptr());
        let (token, poll) = match this.token.take() {
            None => match this.handle.submit(op) {
                None => (None, Poll::Ready(Err(None))),
                Some(token) => (Some(token), Poll::Pending),
            },
            Some(token) => match token.extract(&mut this.handle) {
                Ok((None, Some(token))) => (Some(token), Poll::Pending),
                Ok((Some(val), None)) if val < 0 => (None, Poll::Ready(Err(Some(val)))),
                Ok((Some(val), None)) => match u32::try_from(val) {
                    Ok(fd) => (None, Poll::Ready(Ok(FileDescriptorValue { fd }))),
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

#[derive(Clone, Copy)]
struct FileDescriptorValue {
    fd: u32,
}

impl FileDescriptor for FileDescriptorValue {
    fn as_fd(self) -> u32 {
        self.fd
    }
}

impl Closable for FileDescriptorValue {}
impl ReadableAtOffset for FileDescriptorValue {}
