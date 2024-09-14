mod ack;
mod close;
mod drain;
mod drop;
mod read;
mod wait;
mod write;
mod create;

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
