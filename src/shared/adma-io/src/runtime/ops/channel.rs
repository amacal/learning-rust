use super::*;
use crate::kernel::*;
use crate::syscall::*;

pub struct RxChannel<TPayload, TRx, TTx>
where
    TPayload: Pinned,
    TRx: FileDescriptor,
    TTx: FileDescriptor,
{
    rx: TRx,
    rx_closed: bool,
    rx_drained: bool,
    tx: TTx,
    tx_closed: bool,
    ops: IORuntimeOps,
    __: PhantomData<TPayload>,
}

impl<TPayload, TRx, TTx> RxChannel<TPayload, TRx, TTx>
where
    TPayload: Pinned + Send + Unpin,
    TRx: FileDescriptor + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Closable + Copy + Send + Unpin,
{
    pub fn droplet(self) -> Droplet<Self> {
        fn drop_by_reference<TPayload, TRx, TTx>(target: &mut RxChannel<TPayload, TRx, TTx>)
        where
            TPayload: Pinned + Send + Unpin,
            TRx: FileDescriptor + Closable + Copy + Send + Unpin,
            TTx: FileDescriptor + Closable + Copy + Send + Unpin,
        {
            let rx = target.rx;
            let tx = target.tx;

            let rx_drained = target.rx_drained;
            let rx_closed = target.rx_closed;
            let tx_closed = target.tx_closed;

            // drops asynchronously all involved components
            let drop = move |ops: IORuntimeOps| async move {
                if rx_drained == false {
                    let fd = rx.as_fd();
                    let buffer: [usize; 2] = [0; 2];

                    let result = loop {
                        trace1(b"draining channel-rx droplet; fd=%d\n", fd);
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
                            trace1(b"draining channel-rx droplet; fd=%d, breaking\n", fd);
                            break Err(None);
                        }

                        let (ptr, len) = (buffer[0], buffer[1]);
                        drop(TPayload::from(HeapRef::new(ptr, len)));
                        trace2(b"draining channel-rx droplet; addr=%x, len=%d, dropped\n", ptr, len);
                    };

                    if let Err(Some(errno)) = result {
                        trace2(b"draining channel-rx droplet; fd=%d, failed, res=%d\n", fd, errno);
                    }

                    if let Err(None) = result {
                        trace1(b"draining channel-rx droplet; fd=%d, failed\n", fd);
                    }

                    trace1(b"draining channel-rx droplet; fd=%d, completed\n", fd);
                }

                if rx_closed == false {
                    trace1(b"releasing channel-rx droplet; rx=%d, closing\n", rx.as_fd());
                    match ops.close(rx).await {
                        Ok(_) => (),
                        Err(None) => trace1(b"releasing channel-rx droplet; rx=%d, failed\n", rx.as_fd()),
                        Err(Some(errno)) => trace2(b"releasing channel-rx droplet; rx=%d, err=%d\n", rx.as_fd(), errno),
                    }

                    trace1(b"releasing channel-rx droplet; rx=%d, completed\n", rx.as_fd());
                }

                if tx_closed == false {
                    trace1(b"releasing channel-rx droplet; tx=%d, closing\n", tx.as_fd());
                    match ops.close(rx).await {
                        Ok(_) => (),
                        Err(None) => trace1(b"releasing channel-rx droplet; tx=%d, failed\n", tx.as_fd()),
                        Err(Some(errno)) => trace2(b"releasing channel-rx droplet; tx=%d, err=%d\n", tx.as_fd(), errno),
                    }

                    trace1(b"releasing channel-rx droplet; tx=%d, completed\n", tx.as_fd());
                }

                None::<&'static [u8]>
            };

            if rx_drained == false || rx_closed == false || tx_closed == false {
                if let Err(_) = target.ops.spawn(drop) {
                    trace2(b"releasing channel-rx droplet; rx=%d, tx=%d, failed\n", rx.as_fd(), tx.as_fd());
                }
            }
        }

        trace1(b"creating channel-rx droplet; fd=%d\n", self.tx.as_fd());
        Droplet::from(self, drop_by_reference)
    }
}

pub struct TxChannel<TPayload, TRx, TTx>
where
    TPayload: Pinned,
    TRx: FileDescriptor,
    TTx: FileDescriptor,
{
    total: usize,
    cnt: usize,
    rx: TRx,
    tx: TTx,
    none: PhantomData<TPayload>,
}

pub struct RxReceipt<TTx>
where
    TTx: FileDescriptor + Copy,
{
    tx: TTx,
    ops: IORuntimeOps,
    ack: bool,
    closed: bool,
}

impl<TTx> RxReceipt<TTx>
where
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
{
    pub fn droplet(self) -> Droplet<Self> {
        fn drop_by_reference<TTx>(target: &mut RxReceipt<TTx>)
        where
            TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
        {
            let tx = target.tx;
            let fd = tx.as_fd();

            let ack = target.ack;
            let closed = target.closed;

            let drop = move |ops: IORuntimeOps| async move {
                if ack == false {
                    let buffer: [u8; 1] = [1; 1];

                    trace1(b"releasing channel receipt droplet; fd=%d, ack\n", fd);
                    match ops.write(tx, &buffer).await {
                        Ok(value) if value == 1 => (),
                        Ok(value) => trace2(b"releasing channel receipt droplet; fd=%d, ack, res=%d\n", fd, value),
                        Err(None) => trace1(b"releasing channel receipt droplet; fd=%d, ack, failed\n", fd),
                        Err(Some(errno)) => {
                            trace2(b"releasing channel receipt droplet; fd=%d, ack, err=%d\n", fd, errno)
                        }
                    }
                }

                if closed == false {
                    trace1(b"releasing channel receipt droplet; fd=%d, closing\n", fd);
                    match ops.close(tx).await {
                        Ok(_) => (),
                        Err(None) => trace1(b"releasing channel receipt droplet; fd=%d, failed\n", fd),
                        Err(Some(errno)) => trace2(b"releasing channel receipt droplet; fd=%d, err=%d\n", fd, errno),
                    }
                }

                trace1(b"releasing channel receipt droplet; fd=%d, completed\n", fd);
                None::<&'static [u8]>
            };

            if ack == false || closed == false {
                if let Err(_) = target.ops.spawn(drop) {
                    trace1(b"releasing channel receipt droplet; fd=%d, failed\n", fd);
                }
            }
        }

        trace1(b"creating channel receipt droplet; fd=%d\n", self.tx.as_fd());
        Droplet::from(self, drop_by_reference)
    }
}

impl IORuntimeOps {
    pub fn channel_create<TPayload: Pinned>(
        &self,
        size: usize,
    ) -> Result<
        (
            RxChannel<
                TPayload,
                impl FileDescriptor + Readable + Closable + Copy,
                impl FileDescriptor + Writtable + Duplicable + Closable + Copy,
            >,
            TxChannel<
                TPayload,
                impl FileDescriptor + Readable + Closable + Copy,
                impl FileDescriptor + Writtable + Closable + Copy,
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

        let rx = RxChannel::<TPayload, _, _> {
            rx: rx_rx,
            rx_closed: false,
            rx_drained: false,
            tx: rx_tx,
            tx_closed: false,
            ops: self.duplicate(),
            __: PhantomData,
        };

        let tx = TxChannel::<TPayload, _, _> {
            total: size,
            cnt: size,
            rx: tx_rx,
            tx: tx_tx,
            none: PhantomData,
        };

        Ok((rx, tx))
    }

    pub fn channel_read<'a, TPayload, TRx, TTx>(
        &'a self,
        rx: &'a mut RxChannel<TPayload, TRx, TTx>,
    ) -> impl Future<Output = Option<Result<(TPayload, RxReceipt<TTx>), Option<i32>>>> + 'a
    where
        TPayload: Pinned,
        TRx: FileDescriptor + Readable + Copy,
        TTx: FileDescriptor + Duplicable + Copy,
    {
        async fn execute<TPayload, TRx, TTx>(
            ops: &IORuntimeOps,
            this: &mut RxChannel<TPayload, TRx, TTx>,
        ) -> Option<Result<(TPayload, RxReceipt<TTx>), Option<i32>>>
        where
            TPayload: Pinned,
            TRx: FileDescriptor + Readable + Copy,
            TTx: FileDescriptor + Duplicable + Copy,
        {
            let buffer: [usize; 2] = [0; 2];

            trace1(b"reading channel message; fd=%d\n", this.rx.as_fd());
            match ops.read(this.rx, &buffer).await {
                Ok(cnt) if cnt == 16 => (),
                Ok(_) => return Some(Err(None)),
                Err(errno) => return Some(Err(errno)),
            }

            if buffer[0] == 0 {
                trace1(b"reading channel message; fd=%d, breaking\n", this.rx.as_fd());
                return None;
            }

            let (ptr, len) = (buffer[0], buffer[1]);
            let data = TPayload::from(HeapRef::new(ptr, len));

            let tx = match ops.clone(this.tx) {
                Ok(tx) => tx,
                Err(errno) => return Some(Err(errno)),
            };

            trace2(b"reading channel message; fd=%d, tx=%d, completed\n", this.rx.as_fd(), tx.as_fd());

            Some(Ok((
                data,
                RxReceipt {
                    ops: ops.duplicate(),
                    tx: tx,
                    ack: false,
                    closed: false,
                },
            )))
        }

        execute(self, rx)
    }

    pub fn channel_drain<'a, TPayload, TRx, TTx>(
        &'a self,
        rx: Droplet<RxChannel<TPayload, TRx, TTx>>,
    ) -> impl Future<Output = Result<(), Option<i32>>> + 'a
    where
        TPayload: Pinned + 'a,
        TRx: FileDescriptor + Readable + Closable + Copy + 'a,
        TTx: FileDescriptor + Closable + Copy + 'a,
    {
        async fn execute<TPayload, TRx, TTx>(
            ops: &IORuntimeOps,
            mut this: Droplet<RxChannel<TPayload, TRx, TTx>>,
        ) -> Result<(), Option<i32>>
        where
            TPayload: Pinned,
            TRx: FileDescriptor + Readable + Closable + Copy,
            TTx: FileDescriptor + Closable + Copy,
        {
            if this.rx_drained == false {
                let fd = this.rx.as_fd();
                let buffer: [usize; 2] = [0; 2];

                let result = loop {
                    trace1(b"draining channel-rx; fd=%d\n", fd);
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
                        trace1(b"draining channel-rx; fd=%d, breaking\n", fd);
                        break Err(None);
                    }

                    let (ptr, len) = (buffer[0], buffer[1]);
                    drop(TPayload::from(HeapRef::new(ptr, len)));
                    trace2(b"draining channel-rx; addr=%x, len=%d, dropped\n", ptr, len);
                };

                if let Err(Some(errno)) = result {
                    trace2(b"draining channel-rx; fd=%d, failed, res=%d\n", fd, errno);
                    return Err(Some(errno));
                }

                if let Err(None) = result {
                    trace1(b"draining channel-rx; fd=%d, failed\n", fd);
                    return Err(None);
                }

                this.rx_drained = true;
                trace1(b"draining channel-rx; fd=%d, completed\n", fd);
            }

            if this.rx_closed == false {
                trace1(b"closing channel-rx; rx=%d\n", this.rx.as_fd());
                if let Err(errno) = ops.close(this.rx).await {
                    return Err(errno);
                }

                this.rx_closed = true;
                trace1(b"closing channel-rx; rx=%d, completed\n", this.rx.as_fd());
            }

            if this.tx_closed == false {
                trace1(b"closing channel-rx; tx=%d\n", this.tx.as_fd());
                if let Err(errno) = ops.close(this.tx).await {
                    return Err(errno);
                }

                this.tx_closed = true;
                trace1(b"closing channel-rx; tx=%d, completed\n", this.tx.as_fd());
            }

            Ok(())
        }

        execute(self, rx)
    }

    pub fn channel_write<'a, TPayload, TRx, TTx>(
        &'a self,
        tx: &'a mut TxChannel<TPayload, TRx, TTx>,
        data: TPayload,
    ) -> impl Future<Output = Result<(), Option<i32>>> + 'a
    where
        TPayload: Pinned,
        TRx: FileDescriptor + Readable + Copy,
        TTx: FileDescriptor + Writtable + Copy,
    {
        async fn execute<TPayload, TRx, TTx>(
            ops: &IORuntimeOps,
            this: &mut TxChannel<TPayload, TRx, TTx>,
            data: TPayload,
        ) -> Result<(), Option<i32>>
        where
            TPayload: Pinned,
            TRx: FileDescriptor + Readable + Copy,
            TTx: FileDescriptor + Writtable + Copy,
        {
            while this.cnt == 0 {
                let buffer: [u8; 1] = [0; 1];

                trace1(b"increasing channel slots; cnt=%d, reading\n", buffer[0]);
                match ops.read(this.rx, &buffer).await {
                    Ok(cnt) if cnt == 1 => (),
                    Ok(_) => return Err(None),
                    Err(errno) => return Err(errno),
                }

                if buffer[0] == 0 {
                    trace1(b"increasing channel slots; cnt=%d, unexpected\n", buffer[0]);
                    return Err(None);
                }

                this.cnt += buffer[0] as usize;
                trace1(b"increasing channel slots; cnt=%d, completed\n", this.cnt);
            }

            let heap: HeapRef = data.into();
            let buffer: [usize; 2] = [heap.ptr(), heap.len()];

            trace1(b"writing channel message; cnt=%d\n", this.cnt);
            match ops.write(this.tx, &buffer).await {
                Ok(cnt) if cnt == 16 => (),
                Ok(_) => return Err(None),
                Err(errno) => return Err(errno),
            }

            this.cnt -= 1;
            trace1(b"writing channel message; cnt=%d, completed\n", this.cnt);

            Ok(())
        }

        execute(self, tx, data)
    }

    pub fn channel_wait<'a, TPayload, TRx, TTx>(
        &'a self,
        tx: &'a mut TxChannel<TPayload, TRx, TTx>,
    ) -> impl Future<Output = Result<(), Option<i32>>> + 'a
    where
        TPayload: Pinned,
        TRx: FileDescriptor + Readable + Copy,
        TTx: FileDescriptor,
    {
        async fn execute<TPayload, TRx, TTx>(
            ops: &IORuntimeOps,
            this: &mut TxChannel<TPayload, TRx, TTx>,
        ) -> Result<(), Option<i32>>
        where
            TPayload: Pinned,
            TRx: FileDescriptor + Readable + Copy,
            TTx: FileDescriptor,
        {
            while this.cnt < this.total {
                let buffer: [u8; 1] = [0; 1];

                trace1(b"awaiting channel message; cnt=%d\n", this.cnt);
                match ops.read(this.rx, &buffer).await {
                    Ok(cnt) if cnt == 1 => (),
                    Ok(_) => return Err(None),
                    Err(errno) => return Err(errno),
                }

                if buffer[0] == 0 {
                    trace1(b"awaiting channel message; cnt=%d, unexpected\n", this.cnt);
                    break;
                }

                this.cnt += buffer[0] as usize;
                trace1(b"awaiting channel message; cnt=%d, completed\n", this.cnt);
            }

            Ok(())
        }

        execute(self, tx)
    }

    pub fn channel_close<'a, TPayload, TRx, TTx>(
        &'a self,
        tx: TxChannel<TPayload, TRx, TTx>,
    ) -> impl Future<Output = Result<(), Option<i32>>> + 'a
    where
        TPayload: Pinned + 'a,
        TRx: FileDescriptor + Closable + Copy + 'a,
        TTx: FileDescriptor + Writtable + Closable + Copy + 'a,
    {
        async fn execute<TPayload, TRx, TTx>(
            ops: &IORuntimeOps,
            this: TxChannel<TPayload, TRx, TTx>,
        ) -> Result<(), Option<i32>>
        where
            TPayload: Pinned,
            TRx: FileDescriptor + Closable + Copy,
            TTx: FileDescriptor + Writtable + Closable + Copy,
        {
            trace2(b"closing tx channels; rx=%d, tx=%d\n", this.rx.as_fd(), this.tx.as_fd());
            if let Err(errno) = ops.close(this.rx).await {
                return Err(errno);
            }

            if let Err(errno) = ops.close(this.tx).await {
                return Err(errno);
            }

            trace2(b"closing tx channels; rx=%d, tx=%d, completed\n", this.rx.as_fd(), this.tx.as_fd());
            Ok(())
        }

        execute(self, tx)
    }

    pub fn channel_ack<'a, TTx>(
        &'a self,
        receipt: Droplet<RxReceipt<TTx>>,
    ) -> impl Future<Output = Result<(), Option<i32>>> + 'a
    where
        TTx: FileDescriptor + Writtable + Closable + Copy + 'a,
    {
        async fn execute<TTx>(ops: &IORuntimeOps, mut this: Droplet<RxReceipt<TTx>>) -> Result<(), Option<i32>>
        where
            TTx: FileDescriptor + Writtable + Closable + Copy,
        {
            let buffer: [u8; 1] = [1; 1];

            trace1(b"ack channel message; fd=%d, started\n", this.tx.as_fd());
            match ops.write(this.tx, &buffer).await {
                Ok(cnt) if cnt == 1 => (),
                Ok(_) => return Err(None),
                Err(errno) => return Err(errno),
            }

            this.ack = true;
            trace1(b"ack channel message; fd=%d, completed\n", this.tx.as_fd());

            if let Err(errno) = ops.close(this.tx).await {
                trace1(b"ack channel message; fd=%d, failed\n", this.tx.as_fd());
                return Err(errno);
            }

            this.closed = true;
            trace1(b"ack channel message; fd=%d, completed\n", this.tx.as_fd());

            Ok(())
        }

        execute(self, receipt)
    }
}
