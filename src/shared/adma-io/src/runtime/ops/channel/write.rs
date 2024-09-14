use super::*;

pub trait ChannelWritable {
    type Source;
    type Target;
    type Result;
    type Receipt;

    fn source(&mut self) -> &mut Droplet<Self::Source>;
    fn execute(ops: &IORuntimeOps, target: &mut Droplet<Self::Source>) -> impl Future<Output = Self::Result>;
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
    pub fn channel_write<'a, TPayload, TRx, TTx, TSx>(
        &'a self,
        channel: &'a mut TxChannel<Open, TPayload, TRx, TTx, TSx>,
        data: TPayload,
    ) -> impl Future<Output = Result<(), Option<i32>>> + 'a
    where
        TPayload: Pinned,
        TRx: FileDescriptor + Readable + Copy,
        TTx: FileDescriptor + Writtable + Copy,
        TSx: FileDescriptor + Readable + Copy,
    {
        async move {
            while channel.cnt == 0 {
                let buffer: [u8; 1] = [0; 1];

                trace1(b"increasing channel slots; cnt=%d, reading\n", buffer[0]);
                match self.read(channel.rx, &buffer).await {
                    Ok(cnt) if cnt == 1 => (),
                    Ok(_) => return Err(None),
                    Err(errno) => return Err(errno),
                }

                if buffer[0] == 0 {
                    trace1(b"increasing channel slots; cnt=%d, unexpected\n", buffer[0]);
                    return Err(None);
                }

                channel.cnt += buffer[0] as usize;
                trace1(b"increasing channel slots; cnt=%d, completed\n", channel.cnt);
            }

            if let Err(errno) = write_descriptor(self, channel.tx, data).await {
                return Err(errno);
            }

            channel.cnt -= 1;
            trace1(b"writing channel message; cnt=%d, completed\n", channel.cnt);

            Ok(())
        }
    }
}
