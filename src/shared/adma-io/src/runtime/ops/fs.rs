use ::core::future::*;
use ::core::task::*;
use ::core::pin::*;

use super::*;
use crate::runtime::file::*;
use crate::core::*;


impl IORuntimeOps {
    pub fn open_at<'a, TPath>(
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
                        Ok(fd) => (None, Poll::Ready(Ok(FileDescriptor::new(fd)))),
                        Err(_) => (None, Poll::Ready(Err(None))),
                    },
                },
            },
        };

        this.token = token;
        poll
    }
}
