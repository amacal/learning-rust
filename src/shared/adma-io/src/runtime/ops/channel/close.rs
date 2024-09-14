use super::*;

pub trait ChannelClosable {
    type Source;
    type Target;
    type Result;

    fn source(self) -> Droplet<Self::Source>;
    fn execute(ops: &IORuntimeOps, target: Droplet<Self::Source>) -> impl Future<Output = Self::Result>;
}

impl IORuntimeOps {
    pub fn channel_close<'a, TChannel: ChannelClosable + 'a>(
        &'a self,
        target: TChannel,
    ) -> impl Future<Output = TChannel::Result> + 'a {
        TChannel::execute(self, target.source())
    }
}

pub fn close_descriptor<'a>(
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

impl<TPayload, TRx, TTx> ChannelClosable for Droplet<RxChannel<Drained, TPayload, TRx, TTx>>
where
    TPayload: Pinned + Send + Unpin,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Duplicable + Copy + Send + Unpin,
{
    type Source = RxChannel<Drained, TPayload, TRx, TTx>;
    type Target = RxChannel<Closed, TPayload, TRx, TTx>;
    type Result = Result<Droplet<Self::Target>, Option<i32>>;

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

impl<TPayload, TRx, TTx, TSx> ChannelClosable for Droplet<TxChannel<Open, TPayload, TRx, TTx, TSx>>
where
    TPayload: Pinned + Send + Unpin,
    TRx: FileDescriptor + Readable + Closable + Copy + Send + Unpin,
    TTx: FileDescriptor + Writtable + Closable + Copy + Send + Unpin,
    TSx: FileDescriptor + Closable + Copy + Send + Unpin,
{
    type Source = TxChannel<Open, TPayload, TRx, TTx, TSx>;
    type Target = TxChannel<Closed, TPayload, TRx, TTx, TSx>;
    type Result = Result<Droplet<Self::Target>, Option<i32>>;

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
