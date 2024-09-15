use super::wait::wait_descriptor;
use super::*;

pub trait ChannelWritable {
    type Source;
    type Target;
    type Result;
    type Payload;

    fn source(&mut self) -> &mut Droplet<Self::Source>;

    fn execute(
        ops: &IORuntimeOps,
        target: &mut Droplet<Self::Source>,
        data: Self::Payload,
    ) -> impl Future<Output = Self::Result>;
}

async fn write_descriptor<TPayload: Pinned>(
    ops: &IORuntimeOps,
    tx: impl FileDescriptor + Writtable + Copy,
    data: TPayload,
) -> Result<(), Option<i32>> {
    let heap: HeapRef = data.into();
    let buffer: [usize; 2] = [heap.ptr(), heap.len()];

    trace1(b"writing channel message; tx=%d\n", tx.as_fd());
    let result = match ops.write(tx, &buffer).await {
        Ok(cnt) if cnt == 16 => Ok(()),
        Ok(_) => Err(None),
        Err(errno) => Err(errno),
    };

    if let Err(errno) = result {
        trace2(b"draining channel; addr=%x, len=%d, dropping\n", heap.ptr(), heap.len());
        drop(TPayload::from(heap));

        trace1(b"writing channel message; tx=%d, failed\n", tx.as_fd());
        return Err(errno);
    }

    Ok(())
}

impl IORuntimeOps {
    pub fn channel_write<'a, TChannel: ChannelWritable>(
        &'a self,
        target: &'a mut TChannel,
        data: TChannel::Payload,
    ) -> impl Future<Output = TChannel::Result> + 'a {
        TChannel::execute(self, target.source(), data)
    }
}

impl<TPayload, TRx, TTx, TSx> ChannelWritable for Droplet<TxChannel<Open, TPayload, TRx, TTx, TSx>>
where
    TPayload: Pinned + Send + Unpin,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
    TSx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
{
    type Source = TxChannel<Open, TPayload, TRx, TTx, TSx>;
    type Target = TxChannel<Drained, TPayload, TRx, TTx, TSx>;
    type Result = Result<(), Option<i32>>;
    type Payload = TPayload;

    fn source(&mut self) -> &mut Droplet<Self::Source> {
        self
    }

    fn execute(
        ops: &IORuntimeOps,
        target: &mut Droplet<Self::Source>,
        data: TPayload,
    ) -> impl Future<Output = Self::Result> {
        async move {
            while target.cnt == 0 {
                let cnt = match wait_descriptor(ops, target.rx).await {
                    Ok(cnt) => cnt,
                    Err(errno) => return Err(errno),
                };

                target.cnt += cnt;
                trace2(b"awaiting channel message; rx=%d, cnt=%d, completed\n", target.rx.as_fd(), target.cnt);
            }

            if let Err(errno) = write_descriptor(ops, target.tx, data).await {
                return Err(errno);
            }

            target.cnt -= 1;
            trace2(b"writing channel message; tx=%d, cnt=%d, completed\n", target.tx.as_fd(), target.cnt);

            Ok(())
        }
    }
}
