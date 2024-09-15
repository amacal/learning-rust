use super::*;

pub trait ChannelWaitable {
    type Source;
    type Target;
    type Result;

    fn source(&mut self) -> &mut Droplet<Self::Source>;
    fn execute(ops: &IORuntimeOps, target: &mut Droplet<Self::Source>) -> impl Future<Output = Self::Result>;
}

pub async fn wait_descriptor(
    ops: &IORuntimeOps,
    rx: impl FileDescriptor + Readable + Copy,
) -> Result<usize, Option<i32>> {
    let buffer: [u8; 1] = [0; 1];
    let fd = rx.as_fd();

    trace1(b"awaiting channel message; rx=%d\n", fd);
    let result = match ops.read(rx, &buffer).await {
        Ok(cnt) if cnt == 1 => Ok(()), // one is expected payload
        Ok(cnt) if cnt == 0 => Ok(()), // zero represents closed pipe
        Ok(_) => Err(None),
        Err(errno) => Err(errno),
    };

    if let Err(errno) = result {
        trace1(b"awaiting channel message; rx=%d, failed\n", fd);
        return Err(errno);
    }

    if buffer[0] == 0 {
        trace1(b"awaiting channel message; rx=%d, terminated\n", fd);
    }

    Ok(buffer[0] as usize)
}

impl IORuntimeOps {
    pub fn channel_wait<'a, TChannel: ChannelWaitable>(
        &'a self,
        target: &'a mut TChannel,
    ) -> impl Future<Output = TChannel::Result> + 'a {
        TChannel::execute(self, target.source())
    }
}

impl<TPayload, TRx, TTx, TSx> ChannelWaitable for Droplet<TxChannel<Open, TPayload, TRx, TTx, TSx>>
where
    TPayload: Pinned + Send + Unpin,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
    TSx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
{
    type Source = TxChannel<Open, TPayload, TRx, TTx, TSx>;
    type Target = TxChannel<Drained, TPayload, TRx, TTx, TSx>;
    type Result = Result<(), Option<i32>>;

    fn source(&mut self) -> &mut Droplet<Self::Source> {
        self
    }

    fn execute(ops: &IORuntimeOps, target: &mut Droplet<Self::Source>) -> impl Future<Output = Self::Result> {
        async move {
            while target.cnt < target.total {
                let cnt = match wait_descriptor(ops, target.rx).await {
                    Ok(cnt) => cnt,
                    Err(errno) => return Err(errno),
                };

                target.cnt += cnt;
                trace2(b"awaiting channel message; rx=%d, cnt=%d, completed\n", target.rx.as_fd(), target.cnt);
            }

            Ok(())
        }
    }
}
