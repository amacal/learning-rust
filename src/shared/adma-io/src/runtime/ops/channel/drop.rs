use super::close::close_descriptor;
use super::drain::drain_descriptor;
use super::*;

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
