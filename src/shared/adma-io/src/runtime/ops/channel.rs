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
    tx: TTx,
    none: PhantomData<TPayload>,
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
    cls: bool,
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
            let cls = target.cls;

            let drop = move |ops: IORuntimeOps| async move {
                if ack == false {
                    let buffer: [u8; 1] = [1; 1];

                    trace1(b"releasing channel receipt droplet; fd=%d, ack\n", fd);
                    match ops.write(tx, &buffer).await {
                        Ok(value) if value == 1 => (),
                        Ok(value) => trace2(b"releasing channel receipt droplet; fd=%d, ack, res=%d\n", fd, value),
                        Err(None) => trace1(b"releasing channel receipt droplet; fd=%d, ack, failed\n", fd),
                        Err(Some(errno)) => trace2(b"releasing channel receipt droplet; fd=%d, ack, err=%d\n", fd, errno),
                    }
                }

                if cls == false {
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

            if ack == false || cls == false {
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
            tx: rx_tx,
            none: PhantomData,
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
                    cls: false,
                },
            )))
        }

        execute(self, rx)
    }

    pub fn channel_drain<'a, TPayload, TRx, TTx>(
        &'a self,
        rx: RxChannel<TPayload, TRx, TTx>,
    ) -> impl Future<Output = Result<(), Option<i32>>> + 'a
    where
        TPayload: Pinned + 'a,
        TRx: FileDescriptor + Readable + Closable + Copy + 'a,
        TTx: FileDescriptor + Closable + Copy + 'a,
    {
        async fn execute<TPayload, TRx, TTx>(
            ops: &IORuntimeOps,
            this: RxChannel<TPayload, TRx, TTx>,
        ) -> Result<(), Option<i32>>
        where
            TPayload: Pinned,
            TRx: FileDescriptor + Readable + Closable + Copy,
            TTx: FileDescriptor + Closable + Copy,
        {
            let fd = this.rx.as_fd();
            let buffer: [usize; 2] = [0; 2];

            if let Err(errno) = ops.noop().await {
                return Err(errno);
            }

            loop {
                trace1(b"draining channel; fd=%d\n", fd);
                match sys_read(fd, buffer.as_ptr() as *const (), 16) {
                    value if value == 16 => (),
                    value if value == EAGAIN => break,
                    value if value >= 0 => return Err(None),
                    value => match i32::try_from(value) {
                        Ok(value) => return Err(Some(value)),
                        Err(_) => return Err(None),
                    },
                }

                if buffer[0] == 0 {
                    trace1(b"draining channel; fd=%d, breaking\n", fd);
                    return Err(None);
                }

                let (ptr, len) = (buffer[0], buffer[1]);
                drop(TPayload::from(HeapRef::new(ptr, len)));
                trace2(b"payload dropped; addr=%x, len=%d\n", ptr, len);
            }

            trace2(b"closing rx channels; rx=%d, tx=%d\n", this.rx.as_fd(), this.tx.as_fd());
            if let Err(errno) = ops.close(this.rx).await {
                return Err(errno);
            }

            if let Err(errno) = ops.close(this.tx).await {
                return Err(errno);
            }

            trace1(b"draining channel; fd=%d, completed\n", fd);

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

            this.cls = true;
            trace1(b"ack channel message; fd=%d, completed\n", this.tx.as_fd());

            Ok(())
        }

        execute(self, receipt)
    }
}
