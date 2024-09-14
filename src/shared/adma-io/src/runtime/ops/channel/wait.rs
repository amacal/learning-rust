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
    pub fn channel_wait<'a, TPayload, TRx, TTx, TSx>(
        &'a self,
        channel: &'a mut TxChannel<Open, TPayload, TRx, TTx, TSx>,
    ) -> impl Future<Output = Result<(), Option<i32>>> + 'a
    where
        TPayload: Pinned,
        TRx: FileDescriptor + Readable + Copy,
        TTx: FileDescriptor,
        TSx: FileDescriptor,
    {
        async move {
            while channel.cnt < channel.total {
                let cnt = match wait_descriptor(self, channel.rx).await {
                    Ok(cnt) => cnt,
                    Err(errno) => return Err(errno),
                };

                channel.cnt += cnt;
                trace2(b"awaiting channel message; rx=%d, cnt=%d, completed\n", channel.rx.as_fd(), channel.cnt);
            }

            Ok(())
        }
    }
}
