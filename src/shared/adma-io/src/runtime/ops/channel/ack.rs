use super::*;

impl IORuntimeOps {
    pub fn channel_ack<'a, TTx>(
        &'a self,
        mut receipt: Droplet<RxReceipt<Open, TTx>>,
    ) -> impl Future<Output = Result<(), Option<i32>>> + 'a
    where
        TTx: FileDescriptor + Writtable + Closable + Copy + 'a,
    {
        async move {
            let buffer: [u8; 1] = [1; 1];

            trace1(b"ack channel message; fd=%d, started\n", receipt.tx.as_fd());
            match self.write(receipt.tx, &buffer).await {
                Ok(cnt) if cnt == 1 => (),
                Ok(_) => return Err(None),
                Err(errno) => return Err(errno),
            }

            receipt.ack = true;
            trace1(b"ack channel message; fd=%d, completed\n", receipt.tx.as_fd());

            if let Err(errno) = self.close(receipt.tx).await {
                trace1(b"ack channel message; fd=%d, failed\n", receipt.tx.as_fd());
                return Err(errno);
            }

            receipt.closed = true;
            trace1(b"ack channel message; fd=%d, completed\n", receipt.tx.as_fd());

            Ok(())
        }
    }
}
