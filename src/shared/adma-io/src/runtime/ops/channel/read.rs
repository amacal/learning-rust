use super::*;

pub trait ChannelReadable {
    type Source;
    type Target;
    type Result;
    type Receipt;

    fn source(&mut self) -> &mut Droplet<Self::Source>;
    fn execute(ops: &IORuntimeOps, target: &mut Droplet<Self::Source>) -> impl Future<Output = Self::Result>;
}

pub async fn read_descriptor<TPayload: Pinned>(
    ops: &IORuntimeOps,
    rx: impl FileDescriptor + Readable + Copy,
) -> Result<Option<TPayload>, Option<i32>> {
    let buffer: [usize; 2] = [0; 2];
    let fd = rx.as_fd();

    trace1(b"reading channel message; fd=%d\n", fd);
    match ops.read(rx, &buffer).await {
        Ok(cnt) if cnt == 16 => (),
        Ok(_) => return Err(None),
        Err(errno) => return Err(errno),
    }

    if buffer[0] == 0 {
        trace1(b"reading channel message; fd=%d, breaking\n", fd);
        return Ok(None);
    }

    trace3(b"reading channel message; fd=%d, completed, ptr=%x, len=%d\n", fd, buffer[0], buffer[1]);

    let (ptr, len) = (buffer[0], buffer[1]);
    Ok(Some(TPayload::from(HeapRef::new(ptr, len))))
}

impl IORuntimeOps {
    pub fn channel_read<'a, TChannel: ChannelReadable + 'a>(
        &'a self,
        target: &'a mut TChannel,
    ) -> impl Future<Output = TChannel::Result> + 'a {
        TChannel::execute(self, target.source())
    }
}

impl<TPayload, TRx, TTx> ChannelReadable for Droplet<RxChannel<Open, TPayload, TRx, TTx>>
where
    TPayload: Pinned + Send + Unpin,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Duplicable + Closable + Copy + Send + Unpin,
{
    type Source = RxChannel<Open, TPayload, TRx, TTx>;
    type Target = RxChannel<Drained, TPayload, TRx, TTx>;
    type Result = Option<Result<(TPayload, Self::Receipt), Option<i32>>>;
    type Receipt = RxReceipt<Open, TTx>;

    fn source(&mut self) -> &mut Droplet<Self::Source> {
        self
    }

    fn execute(ops: &IORuntimeOps, target: &mut Droplet<Self::Source>) -> impl Future<Output = Self::Result> {
        async move {
            let data = match read_descriptor(ops, target.rx).await {
                Ok(None) => return None,
                Ok(Some(data)) => data,
                Err(errno) => return Some(Err(errno)),
            };

            let tx = match ops.clone(target.tx) {
                Ok(tx) => tx,
                Err(errno) => return Some(Err(errno)),
            };

            trace2(b"reading channel message; fd=%d, receipted, tx=%d\n", target.rx.as_fd(), tx.as_fd());

            Some(Ok((
                data,
                RxReceipt {
                    ops: ops.duplicate(),
                    tx: tx,
                    ack: false,
                    closed: false,
                    _state: PhantomData,
                },
            )))
        }
    }
}
