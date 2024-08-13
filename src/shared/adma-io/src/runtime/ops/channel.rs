use super::*;

pub trait RxChannelFactory<TPayload: Pinned> {
    fn create(
        self,
        ops: &IORuntimeOps,
    ) -> RxChannel<
        TPayload,
        impl FileDescriptor + Readable + Closable + Copy + Send + Unpin,
        impl FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
    >;
}

pub trait TxChannelFactory<TPayload: Pinned> {
    fn create(
        self,
        ops: &IORuntimeOps,
    ) -> TxChannel<
        TPayload,
        impl FileDescriptor + Readable + Closable + Copy + Send + Unpin,
        impl FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
    >;
}

impl IORuntimeOps {
    pub fn channel<TPayload: Pinned>(
        &self,
        size: usize,
    ) -> Result<(impl RxChannelFactory<TPayload>, impl TxChannelFactory<TPayload>), Option<i32>> {
        let (rx_rx, tx_tx) = match self.pipe() {
            Ok((rx, tx)) => (rx, tx),
            Err(errno) => return Err(errno),
        };

        let (tx_rx, rx_tx) = match self.pipe() {
            Ok((rx, tx)) => (rx, tx),
            Err(errno) => return Err(errno),
        };

        let rx = RxChannelPrototype::<TPayload, _, _> {
            rx: rx_rx,
            tx: rx_tx,
            _p: PhantomData,
        };

        let tx = TxChannelPrototype::<TPayload, _, _> {
            cnt: size,
            rx: tx_rx,
            tx: tx_tx,
            _p: PhantomData,
        };

        Ok((rx, tx))
    }
}

struct RxChannelPrototype<TPayload, TRx, TTx>
where
    TPayload: Pinned,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
{
    rx: TRx,
    tx: TTx,
    _p: PhantomData<TPayload>,
}

// unsafe impl<TPayload, TRx, TTx> Send for RxChannelPrototype<TPayload, TRx, TTx>
// where
//     TPayload: Pinned,
//     TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
//     TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
// {
// }

impl<TPayload, TRx, TTx> RxChannelFactory<TPayload> for RxChannelPrototype<TPayload, TRx, TTx>
where
    TPayload: Pinned,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
{
    fn create(
        self,
        ops: &IORuntimeOps,
    ) -> RxChannel<
        TPayload,
        impl FileDescriptor + Readable + Closable + Copy + Send + Unpin,
        impl FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
    > {
        RxChannel {
            ops: ops.duplicate(),
            rx: self.rx,
            tx: self.tx,
            _p: PhantomData,
        }
    }
}

struct TxChannelPrototype<TPayload, TRx, TTx>
where
    TPayload: Pinned,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
{
    cnt: usize,
    rx: TRx,
    tx: TTx,
    _p: PhantomData<TPayload>,
}

// unsafe impl<TPayload, TRx, TTx> Send for TxChannelPrototype<TPayload, TRx, TTx>
// where
//     TPayload: Pinned,
//     TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
//     TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
// {
// }

impl<TPayload, TRx, TTx> TxChannelFactory<TPayload> for TxChannelPrototype<TPayload, TRx, TTx>
where
    TPayload: Pinned,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
{
    fn create(
        self,
        ops: &IORuntimeOps,
    ) -> TxChannel<
        TPayload,
        impl FileDescriptor + Readable + Closable + Copy + Send + Unpin,
        impl FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
    > {
        TxChannel {
            ops: ops.duplicate(),
            cnt: self.cnt,
            total: self.cnt,
            rx: self.rx,
            tx: self.tx,
            _p: PhantomData,
        }
    }
}

pub struct RxChannel<TPayload, TRx, TTx>
where
    TPayload: Pinned,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
{
    ops: IORuntimeOps,
    rx: TRx,
    tx: TTx,
    _p: PhantomData<TPayload>,
}

pub struct RxReceipt<TTx>
where
    TTx: FileDescriptor + Writtable + Copy,
{
    ops: IORuntimeOps,
    tx: TTx,
}

impl<TTx> RxReceipt<TTx>
where
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
{
    pub fn complete<'a>(&'a mut self, ops: &'a IORuntimeOps) -> impl Future<Output = Result<(), Option<i32>>> + 'a {
        async fn execute<TTx>(this: &mut RxReceipt<TTx>, ops: &IORuntimeOps) -> Result<(), Option<i32>>
        where
            TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
        {
            let buffer: [u8; 1] = [1; 1];

            trace0(b"completing channel message\n");
            match ops.write(this.tx, &buffer).await {
                Ok(cnt) if cnt == 1 => (),
                Ok(_) => return Err(None),
                Err(errno) => return Err(errno),
            }

            trace0(b"completing channel message; completed\n");
            Ok(())
        }

        execute(self, ops)
    }
}

impl<TPayload, TRx, TTx> RxChannel<TPayload, TRx, TTx>
where
    TPayload: Pinned,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
{
    pub fn read<'a>(
        &'a mut self,
    ) -> impl Future<Output = Option<Result<(RxReceipt<TTx>, TPayload), Option<i32>>>> + 'a {
        async fn execute<TPayload, TRx, TTx>(
            this: &mut RxChannel<TPayload, TRx, TTx>,
        ) -> Option<Result<(RxReceipt<TTx>, TPayload), Option<i32>>>
        where
            TPayload: Pinned,
            TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
            TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
        {
            let buffer: [usize; 2] = [0; 2];

            match this.ops.read(this.rx, &buffer).await {
                Ok(cnt) if cnt == 16 => (),
                Ok(_) => return Some(Err(None)),
                Err(errno) => return Some(Err(errno)),
            }

            let (ptr, len) = (buffer[0], buffer[1]);
            let data = TPayload::from(HeapRef::new(ptr, len));

            Some(Ok((
                RxReceipt {
                    ops: this.ops.duplicate(),
                    tx: this.tx,
                },
                data,
            )))
        }

        execute(self)
    }
}

pub struct TxChannel<TPayload, TRx, TTx>
where
    TPayload: Pinned,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
{
    ops: IORuntimeOps,
    cnt: usize,
    total: usize,
    rx: TRx,
    tx: TTx,
    _p: PhantomData<TPayload>,
}

impl<TPayload, TRx, TTx> TxChannel<TPayload, TRx, TTx>
where
    TPayload: Pinned,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
{
    pub fn write<'a>(&'a mut self, data: TPayload) -> impl Future<Output = Result<(), Option<i32>>> + 'a {
        async fn execute<TPayload, TRx, TTx>(
            this: &mut TxChannel<TPayload, TRx, TTx>,
            data: TPayload,
        ) -> Result<(), Option<i32>>
        where
            TPayload: Pinned,
            TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
            TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
        {
            while this.cnt == 0 {
                let buffer: [u8; 1] = [0; 1];

                match this.ops.read(this.rx, &buffer).await {
                    Ok(cnt) if cnt == 1 => (),
                    Ok(_) => return Err(None),
                    Err(errno) => return Err(errno),
                }

                this.cnt += buffer[0] as usize;
                trace1(b"increasing channel slots; cnt=%d\n", this.cnt);
            }

            let heap: HeapRef = data.into();
            let buffer: [usize; 2] = [heap.ptr(), heap.len()];

            trace1(b"sending channel message; cnt=%d\n", this.cnt);
            match this.ops.write(this.tx, &buffer).await {
                Ok(cnt) if cnt == 16 => (),
                Ok(_) => return Err(None),
                Err(errno) => return Err(errno),
            }

            this.cnt -= 1;
            trace1(b"sending channel message; cnt=%d, completed\n", this.cnt);

            Ok(())
        }

        execute(self, data)
    }

    pub fn flush<'a>(&'a mut self) -> impl Future<Output = Result<(), Option<i32>>> + 'a {
        async fn execute<TPayload, TRx, TTx>(this: &mut TxChannel<TPayload, TRx, TTx>) -> Result<(), Option<i32>>
        where
            TPayload: Pinned,
            TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
            TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
        {
            while this.cnt < this.total {
                let buffer: [u8; 1] = [0; 1];

                trace1(b"awaiting channel message; cnt=%d\n", this.cnt);
                match this.ops.read(this.rx, &buffer).await {
                    Ok(cnt) if cnt == 1 => (),
                    Ok(_) => return Err(None),
                    Err(errno) => return Err(errno),
                }

                this.cnt += buffer[0] as usize;
                trace1(b"awaiting channel message; cnt=%d, completed\n", this.cnt);
            }

            Ok(())
        }

        execute(self)
    }

    pub fn close<'a>(&'a mut self) -> impl Future<Output = Result<(), Option<i32>>> + 'a {
        async fn execute<TPayload, TRx, TTx>(this: &mut TxChannel<TPayload, TRx, TTx>) -> Result<(), Option<i32>>
        where
            TPayload: Pinned,
            TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
            TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
        {
            trace2(b"closing tx channels; rx=%d, tx=%d\n", this.rx.as_fd(), this.tx.as_fd());
            if let Err(errno) = this.ops.close(this.rx).await {
                return Err(errno);
            }

            if let Err(errno) = this.ops.close(this.tx).await {
                return Err(errno);
            }

            trace2(b"closing tx channels; rx=%d, tx=%d, completed\n", this.rx.as_fd(), this.tx.as_fd());
            Ok(())
        }

        execute(self)
    }
}
