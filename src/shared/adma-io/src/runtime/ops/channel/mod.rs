mod close;
mod drain;
mod drop;
mod read;

use super::*;
use crate::kernel::*;
use crate::syscall::*;

pub struct Open {}
pub struct Drained {}
pub struct Closed {}

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

impl<TState, TPayload, TRx, TTx> RxChannel<TState, TPayload, TRx, TTx>
where
    TPayload: Pinned + Send + Unpin,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Duplicable + Copy + Send + Unpin,
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
