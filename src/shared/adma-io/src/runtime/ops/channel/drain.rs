use super::*;

pub trait ChannelDrainable {
    type Source;
    type Target;
    type Result;

    fn source(self) -> Droplet<Self::Source>;
    fn execute(ops: &IORuntimeOps, target: Droplet<Self::Source>) -> impl Future<Output = Self::Result>;
}

impl IORuntimeOps {
    pub fn channel_drain<'a, TChannel: ChannelDrainable + 'a>(
        &'a self,
        target: TChannel,
    ) -> impl Future<Output = TChannel::Result> + 'a {
        TChannel::execute(self, target.source())
    }
}

pub fn drain_descriptor<TPayload: Pinned>(rx: impl FileDescriptor + Readable) -> Result<(), Option<i32>> {
    let fd = rx.as_fd();
    let buffer: [usize; 2] = [0; 2];

    let result = loop {
        // we expect that the read operation won't block due to O_NONBLOCK mode
        trace1(b"draining channel; fd=%d\n", fd);
        match sys_read(fd, buffer.as_ptr() as *const (), 16) {
            value if value == 16 => (),
            value if value == 0 || value == EAGAIN => break Ok(()),
            value if value > 0 => break Err(None),
            value => match i32::try_from(value) {
                Ok(value) => break Err(Some(value)),
                Err(_) => break Err(None),
            },
        }

        if buffer[0] == 0 {
            trace1(b"draining channel; fd=%d, breaking\n", fd);
            break Err(None);
        }

        let (ptr, len) = (buffer[0], buffer[1]);
        drop(TPayload::from(HeapRef::new(ptr, len)));
        trace2(b"draining channel; addr=%x, len=%d, dropped\n", ptr, len);
    };

    if let Err(None) = result {
        trace1(b"draining channel; fd=%d, failed\n", fd);
    }

    if let Err(Some(errno)) = result {
        trace2(b"draining channel; fd=%d, failed, res=%d\n", fd, errno);
    }

    trace1(b"draining channel; fd=%d, completed\n", fd);
    result
}

impl<TPayload, TRx, TTx> ChannelDrainable for Droplet<RxChannel<Open, TPayload, TRx, TTx>>
where
    TPayload: Pinned + Send + Unpin,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Duplicable + Copy + Send + Unpin,
{
    type Source = RxChannel<Open, TPayload, TRx, TTx>;
    type Target = RxChannel<Drained, TPayload, TRx, TTx>;
    type Result = Result<Droplet<Self::Target>, Option<i32>>;

    fn source(self) -> Droplet<Self::Source> {
        self
    }

    fn execute(
        _: &IORuntimeOps,
        mut target: Droplet<Self::Source>,
    ) -> impl Future<Output = Result<Droplet<Self::Target>, Option<i32>>> {
        async move {
            if target.rx_drained == false {
                if let Err(errno) = drain_descriptor::<TPayload>(target.rx) {
                    return Err(errno);
                } else {
                    target.rx_drained = true;
                }
            }

            Ok(RxChannel::transform(target))
        }
    }
}
