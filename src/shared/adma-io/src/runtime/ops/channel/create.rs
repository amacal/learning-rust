use super::*;

impl IORuntimeOps {
    pub fn channel_create<TPayload: Pinned>(
        &self,
        size: usize,
    ) -> Result<
        (
            RxChannel<
                Open,
                TPayload,
                impl FileDescriptor + Readable + Closable + Copy,
                impl FileDescriptor + Writtable + Duplicable + Closable + Copy,
            >,
            TxChannel<
                Open,
                TPayload,
                impl FileDescriptor + Readable + Closable + Copy,
                impl FileDescriptor + Writtable + Closable + Copy,
                impl FileDescriptor + Readable + Closable + Copy,
            >,
        ),
        Option<i32>,
    > {
        let (rx_rx, tx_tx) = match self.pipe() {
            Ok((rx, tx)) => (rx, tx),
            Err(errno) => return Err(errno),
        };

        let (tx_rx, rx_tx) = match self.pipe() {
            Ok((rx, tx)) => (rx, tx),
            Err(errno) => return Err(errno),
        };

        let sx = match self.clone(rx_rx) {
            Ok(sx) => sx,
            Err(errno) => return Err(errno),
        };

        let rx = RxChannel::<Open, TPayload, _, _> {
            rx: rx_rx,
            rx_closed: false,
            rx_drained: false,
            tx: rx_tx,
            tx_closed: false,
            ops: self.duplicate(),
            _state: PhantomData,
            _payload: PhantomData,
        };

        let tx = TxChannel::<Open, TPayload, _, _, _> {
            total: size,
            cnt: size,
            ops: self.duplicate(),
            rx: tx_rx,
            rx_closed: false,
            tx: tx_tx,
            tx_closed: false,
            sx: sx,
            sx_closed: false,
            _state: PhantomData,
            _payload: PhantomData,
        };

        Ok((rx, tx))
    }
}
