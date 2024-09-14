use super::*;

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
                let buffer: [u8; 1] = [0; 1];

                trace1(b"awaiting channel message; cnt=%d\n", channel.cnt);
                let result = match self.read(channel.rx, &buffer).await {
                    Ok(cnt) if cnt == 1 => Ok(()), // one is expected payload
                    Ok(cnt) if cnt == 0 => Ok(()), // zero represents closed pipe
                    Ok(_) => Err(None),
                    Err(errno) => Err(errno),
                };

                if let Err(errno) = result {
                    trace1(b"awaiting channel message; cnt=%d, failed\n", channel.cnt);
                    return Err(errno);
                }

                if buffer[0] == 0 {
                    trace1(b"awaiting channel message; cnt=%d, terminated\n", channel.cnt);
                    break;
                }

                channel.cnt += buffer[0] as usize;
                trace1(b"awaiting channel message; cnt=%d, completed\n", channel.cnt);
            }

            Ok(())
        }
    }
}
