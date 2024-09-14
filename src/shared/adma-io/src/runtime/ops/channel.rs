use super::*;
use crate::kernel::*;
use crate::syscall::*;

pub struct Open {}
pub struct Drained {}
pub struct Closed {}

pub trait ChannelClosable {
    type Source;
    type Target;

    fn source(self) -> Droplet<Self::Source>;

    fn execute(
        ops: &IORuntimeOps,
        target: Droplet<Self::Source>,
    ) -> impl Future<Output = Result<Droplet<Self::Target>, Option<i32>>>;
}

pub trait ChanelDrainable {
    type Source;
    type Target;

    fn source(self) -> Droplet<Self::Source>;

    fn execute(
        ops: &IORuntimeOps,
        target: Droplet<Self::Source>,
    ) -> impl Future<Output = Result<Droplet<Self::Target>, Option<i32>>>;
}

fn close_descriptor<'a>(
    ops: &'a IORuntimeOps,
    descriptor: impl FileDescriptor + Closable + Copy + 'a,
) -> impl Future<Output = Result<(), Option<i32>>> + 'a {
    async move {
        trace1(b"closing channel; fd=%d\n", descriptor.as_fd());
        let result = ops.close(descriptor).await;

        if let Err(None) = result {
            trace1(b"closing channel; fd=%d, failed\n", descriptor.as_fd());
        }

        if let Err(Some(errno)) = result {
            trace2(b"closing channel; fd=%d, err=%d\n", descriptor.as_fd(), errno);
        }

        trace1(b"closing channel; fd=%d, completed\n", descriptor.as_fd());
        result
    }
}

fn drain_descriptor<TPayload: Pinned>(rx: impl FileDescriptor + Readable) -> Result<(), Option<i32>> {
    let fd = rx.as_fd();
    let buffer: [usize; 2] = [0; 2];

    let result = loop {
        // we expect that the read operation won't block due to O_NONBLOCK mode
        trace1(b"draining channel; fd=%d\n", fd);
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
            trace1(b"draining channel; fd=%d, breaking\n", fd);
            break Err(None);
        }

        let (ptr, len) = (buffer[0], buffer[1]);
        drop(TPayload::from(HeapRef::new(ptr, len)));
        trace2(b"draining channel; addr=%x, len=%d, dropped\n", ptr, len);
    };

    if let Err(None) = result {
        trace1(b"draining channel; fd=%d, failed\n", fd);
    }

    if let Err(Some(errno)) = result {
        trace2(b"draining channel; fd=%d, failed, res=%d\n", fd, errno);
    }

    trace1(b"draining channel; fd=%d, completed\n", fd);
    result
}

pub struct RxChannel<TState, TPayload, TRx, TTx>
where
    TPayload: Pinned,
    TRx: FileDescriptor,
    TTx: FileDescriptor,
{
    ops: IORuntimeOps,

    rx: TRx,
    rx_closed: bool,
    rx_drained: bool,
    tx: TTx,
    tx_closed: bool,

    _state: PhantomData<TState>,
    _payload: PhantomData<TPayload>,
}

impl<TPayload, TRx, TTx> ChannelClosable for Droplet<RxChannel<Drained, TPayload, TRx, TTx>>
where
    TPayload: Pinned + Send + Unpin,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
{
    type Source = RxChannel<Drained, TPayload, TRx, TTx>;
    type Target = RxChannel<Closed, TPayload, TRx, TTx>;

    fn source(self) -> Droplet<Self::Source> {
        self
    }

    fn execute(
        ops: &IORuntimeOps,
        mut target: Droplet<Self::Source>,
    ) -> impl Future<Output = Result<Droplet<Self::Target>, Option<i32>>> {
        async move {
            if target.rx_closed == false {
                match close_descriptor(&ops, target.rx).await {
                    Err(errno) => return Err(errno),
                    Ok(()) => target.rx_closed = true,
                }
            }

            if target.tx_closed == false {
                match close_descriptor(&ops, target.tx).await {
                    Err(errno) => return Err(errno),
                    Ok(()) => target.tx_closed = true,
                }
            }

            Ok(RxChannel::transform::<Closed>(target))
        }
    }
}

impl<TState, TPayload, TRx, TTx> RxChannel<TState, TPayload, TRx, TTx>
where
    TPayload: Pinned + Send + Unpin,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
{
    fn transform<TOther>(target: Droplet<Self>) -> Droplet<RxChannel<TOther, TPayload, TRx, TTx>> {
        let target = Droplet::extract(target);
        let target = RxChannel {
            ops: target.ops,
            rx: target.rx,
            rx_closed: target.rx_closed,
            rx_drained: target.rx_drained,
            tx: target.tx,
            tx_closed: target.tx_closed,
            _state: PhantomData,
            _payload: PhantomData,
        };

        target.droplet()
    }
}

impl<TState, TPayload, TRx, TTx> RxChannel<TState, TPayload, TRx, TTx>
where
    TPayload: Pinned + Send + Unpin,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Closable + Copy + Send + Unpin,
{
    pub fn droplet(self) -> Droplet<Self> {
        fn drop_by_reference<TState, TPayload, TRx, TTx>(target: &mut RxChannel<TState, TPayload, TRx, TTx>)
        where
            TPayload: Pinned + Send + Unpin,
            TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
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
                    let _ = drain_descriptor::<TPayload>(rx);
                }

                if rx_closed == false {
                    let _ = close_descriptor(&ops, rx).await;
                }

                if tx_closed == false {
                    let _ = close_descriptor(&ops, tx).await;
                }

                trace2(b"releasing channel-rx droplet; rx=%d, tx=%d, completed\n", rx.as_fd(), tx.as_fd());
                None::<&'static [u8]>
            };

            trace2(b"triggering channel-rx droplet; rx=%d, tx=%d\n", rx.as_fd(), tx.as_fd());
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

pub struct TxChannel<TState, TPayload, TRx, TTx, TSx>
where
    TPayload: Pinned,
    TRx: FileDescriptor,
    TTx: FileDescriptor,
    TSx: FileDescriptor,
{
    total: usize,
    cnt: usize,
    ops: IORuntimeOps,

    rx: TRx,
    rx_closed: bool,
    tx: TTx,
    tx_closed: bool,
    sx: TSx,
    sx_closed: bool,

    _state: PhantomData<TState>,
    _payload: PhantomData<TPayload>,
}

impl<TPayload, TRx, TTx, TSx> ChannelClosable for Droplet<TxChannel<Open, TPayload, TRx, TTx, TSx>>
where
    TPayload: Pinned + Send + Unpin,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
    TSx: FileDescriptor + Closable + Copy + Send + Unpin,
{
    type Source = TxChannel<Open, TPayload, TRx, TTx, TSx>;
    type Target = TxChannel<Closed, TPayload, TRx, TTx, TSx>;

    fn source(self) -> Droplet<Self::Source> {
        self
    }

    fn execute(
        ops: &IORuntimeOps,
        mut target: Droplet<Self::Source>,
    ) -> impl Future<Output = Result<Droplet<Self::Target>, Option<i32>>> {
        async move {
            if target.rx_closed == false {
                match close_descriptor(&ops, target.rx).await {
                    Err(errno) => return Err(errno),
                    Ok(()) => target.rx_closed = true,
                }
            }

            if target.tx_closed == false {
                match close_descriptor(&ops, target.tx).await {
                    Err(errno) => return Err(errno),
                    Ok(()) => target.tx_closed = true,
                }
            }

            Ok(TxChannel::transform::<Closed>(target))
        }
    }
}

impl<TState, TPayload, TRx, TTx, TSx> TxChannel<TState, TPayload, TRx, TTx, TSx>
where
    TPayload: Pinned + Send + Unpin,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
    TSx: FileDescriptor + Closable + Copy + Send + Unpin,
{
    fn transform<TOther>(target: Droplet<Self>) -> Droplet<TxChannel<TOther, TPayload, TRx, TTx, TSx>> {
        let target = Droplet::extract(target);
        let target = TxChannel {
            total: target.total,
            cnt: target.cnt,
            ops: target.ops,
            rx: target.rx,
            rx_closed: target.rx_closed,
            tx: target.tx,
            tx_closed: target.tx_closed,
            sx: target.sx,
            sx_closed: target.sx_closed,
            _state: PhantomData,
            _payload: PhantomData,
        };

        target.droplet()
    }
}

impl<TState, TPayload, TRx, TTx, TSx> TxChannel<TState, TPayload, TRx, TTx, TSx>
where
    TPayload: Pinned + Send + Unpin,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Closable + Copy + Send + Unpin,
    TSx: FileDescriptor + Closable + Copy + Send + Unpin,
{
    pub fn droplet(self) -> Droplet<Self> {
        fn drop_by_reference<TState, TPayload, TRx, TTx, TSx>(target: &mut TxChannel<TState, TPayload, TRx, TTx, TSx>)
        where
            TPayload: Pinned + Send + Unpin,
            TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
            TTx: FileDescriptor + Closable + Copy + Send + Unpin,
            TSx: FileDescriptor + Closable + Copy + Send + Unpin,
        {
            let rx = target.rx;
            let tx = target.tx;
            let sx = target.sx;

            let rx_closed = target.rx_closed;
            let tx_closed = target.tx_closed;
            let sx_closed = target.tx_closed;

            // drops asynchronously all involved components
            let drop = move |ops: IORuntimeOps| async move {
                if rx_closed == false {
                    let _ = close_descriptor(&ops, rx).await;
                }

                if tx_closed == false {
                    let _ = close_descriptor(&ops, tx).await;
                }

                if sx_closed == false {
                    let _ = close_descriptor(&ops, sx).await;
                }

                trace3(
                    b"releasing channel-tx droplet; rx=%d, tx=%d, sx=%d, completed\n",
                    rx.as_fd(),
                    tx.as_fd(),
                    sx.as_fd(),
                );

                None::<&'static [u8]>
            };

            trace3(b"triggering channel-tx droplet; rx=%d, tx=%d, sx=%d\n", rx.as_fd(), tx.as_fd(), sx.as_fd());
            if rx_closed == false || tx_closed == false || sx_closed == false {
                if let Err(_) = target.ops.spawn(drop) {
                    trace3(
                        b"releasing channel-tx droplet; rx=%d, tx=%d, sx=%d, failed\n",
                        rx.as_fd(),
                        tx.as_fd(),
                        sx.as_fd(),
                    );
                }
            }
        }

        trace1(b"creating channel-rx droplet; fd=%d\n", self.tx.as_fd());
        Droplet::from(self, drop_by_reference)
    }
}

pub struct RxReceipt<TState, TTx>
where
    TTx: FileDescriptor + Copy,
{
    tx: TTx,
    ops: IORuntimeOps,
    ack: bool,
    closed: bool,

    _state: PhantomData<TState>,
}

impl<TTx> RxReceipt<Open, TTx>
where
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
{
    pub fn droplet(self) -> Droplet<Self> {
        fn drop_by_reference<TTx>(target: &mut RxReceipt<Open, TTx>)
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

    pub fn channel_read<'a, TPayload, TRx, TTx>(
        &'a self,
        channel: &'a mut RxChannel<Open, TPayload, TRx, TTx>,
    ) -> impl Future<Output = Option<Result<(TPayload, RxReceipt<Open, TTx>), Option<i32>>>> + 'a
    where
        TPayload: Pinned,
        TRx: FileDescriptor + Readable + Copy,
        TTx: FileDescriptor + Duplicable + Copy,
    {
        async move {
            let buffer: [usize; 2] = [0; 2];

            trace1(b"reading channel message; fd=%d\n", channel.rx.as_fd());
            match self.read(channel.rx, &buffer).await {
                Ok(cnt) if cnt == 16 => (),
                Ok(_) => return Some(Err(None)),
                Err(errno) => return Some(Err(errno)),
            }

            if buffer[0] == 0 {
                trace1(b"reading channel message; fd=%d, breaking\n", channel.rx.as_fd());
                return None;
            }

            let (ptr, len) = (buffer[0], buffer[1]);
            let data = TPayload::from(HeapRef::new(ptr, len));

            let tx = match self.clone(channel.tx) {
                Ok(tx) => tx,
                Err(errno) => return Some(Err(errno)),
            };

            trace2(b"reading channel message; fd=%d, tx=%d, completed\n", channel.rx.as_fd(), tx.as_fd());

            Some(Ok((
                data,
                RxReceipt {
                    ops: self.duplicate(),
                    tx: tx,
                    ack: false,
                    closed: false,
                    _state: PhantomData,
                },
            )))
        }
    }

    pub fn channel_drain<'a, TPayload, TRx, TTx>(
        &'a self,
        mut target: Droplet<RxChannel<Open, TPayload, TRx, TTx>>,
    ) -> impl Future<Output = Result<Droplet<RxChannel<Drained, TPayload, TRx, TTx>>, Option<i32>>> + 'a
    where
        TPayload: Pinned + Send + Unpin,
        TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
        TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
        Droplet<RxChannel<Open, TPayload, TRx, TTx>>: 'a,
    {
        async move {
            if target.rx_drained == false {
                if let Err(errno) = drain_descriptor::<TPayload>(target.rx) {
                    return Err(errno);
                } else {
                    target.rx_drained = true;
                }
            }

            Ok(RxChannel::transform(target))
        }
    }

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

            let heap: HeapRef = data.into();
            let buffer: [usize; 2] = [heap.ptr(), heap.len()];

            trace1(b"writing channel message; cnt=%d\n", channel.cnt);
            let result = match self.write(channel.tx, &buffer).await {
                Ok(cnt) if cnt == 16 => Ok(()),
                Ok(_) => Err(None),
                Err(errno) => Err(errno),
            };

            if let Err(errno) = result {
                trace2(b"draining channel; addr=%x, len=%d, dropping\n", heap.ptr(), heap.len());
                drop(TPayload::from(heap));

                trace1(b"writing channel message; cnt=%d, failed\n", channel.cnt);
                return Err(errno);
            }

            channel.cnt -= 1;
            trace1(b"writing channel message; cnt=%d, completed\n", channel.cnt);

            Ok(())
        }
    }

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

    pub fn channel_close<'a, TChannel: ChannelClosable + 'a>(
        &'a self,
        target: TChannel,
    ) -> impl Future<Output = Result<Droplet<TChannel::Target>, Option<i32>>> + 'a {
        TChannel::execute(self, target.source())
    }

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
