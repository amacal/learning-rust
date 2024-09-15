use adma_heap::*;

use crate::runtime::*;

struct BoxedNumber {
    x: usize,
    y: usize,
}

impl Pinned for BoxedNumber {
    fn into(self: Self) -> HeapRef {
        HeapRef::new(self.x, self.y)
    }

    fn from(heap: HeapRef) -> Self {
        BoxedNumber {
            x: heap.ptr(),
            y: heap.len(),
        }
    }
}

#[test]
fn create_channel_and_autodrop_it() {
    let mut runtime = match IORingRuntime::allocate() {
        IORingRuntimeAllocate::Succeeded(runtime) => runtime,
        IORingRuntimeAllocate::RingAllocationFailed() => return assert!(false),
        IORingRuntimeAllocate::PoolAllocationFailed() => return assert!(false),
        IORingRuntimeAllocate::RegistryAllocationFailed() => return assert!(false),
    };

    async fn callback(ops: IORuntimeOps) -> Option<&'static [u8]> {
        let (rx, tx) = ops.channel_create::<BoxedNumber>(1).unwrap();
        let (_, _) = (rx.droplet(), tx.droplet());

        None
    }

    match runtime.run(callback) {
        IORingRuntimeRun::Completed(res) => assert!(res.is_none()),
        IORingRuntimeRun::CompletionFailed(_) => assert!(false),
        IORingRuntimeRun::AllocationFailed(_) => assert!(false),
        IORingRuntimeRun::InternallyFailed() => assert!(false),
    }
}

#[test]
fn synchronous_flow_with_one_item() {
    let mut runtime = match IORingRuntime::allocate() {
        IORingRuntimeAllocate::Succeeded(runtime) => runtime,
        IORingRuntimeAllocate::RingAllocationFailed() => return assert!(false),
        IORingRuntimeAllocate::PoolAllocationFailed() => return assert!(false),
        IORingRuntimeAllocate::RegistryAllocationFailed() => return assert!(false),
    };

    async fn callback(ops: IORuntimeOps) -> Option<&'static [u8]> {
        let (rx, tx) = ops.channel_create::<BoxedNumber>(1).unwrap();
        let (mut rx, mut tx) = (rx.droplet(), tx.droplet());

        ops.channel_write(&mut tx, BoxedNumber { x: 1, y: 2 }).await.unwrap();
        let (number, receipt) = ops.channel_read(&mut rx).await.unwrap().unwrap();
        let receipt = receipt.droplet();

        assert_eq!(number.x, 1);
        assert_eq!(number.y, 2);

        ops.channel_ack(receipt).await.unwrap();
        ops.channel_wait(&mut tx).await.unwrap();

        let rx = ops.channel_drain(rx).await.unwrap();

        ops.channel_close(rx).await.unwrap();
        ops.channel_close(tx).await.unwrap();

        None
    }

    match runtime.run(callback) {
        IORingRuntimeRun::Completed(res) => assert!(res.is_none()),
        IORingRuntimeRun::CompletionFailed(_) => assert!(false),
        IORingRuntimeRun::AllocationFailed(_) => assert!(false),
        IORingRuntimeRun::InternallyFailed() => assert!(false),
    }
}

#[test]
fn asynchronous_flow_with_ten_items() {
    let mut runtime = match IORingRuntime::allocate() {
        IORingRuntimeAllocate::Succeeded(runtime) => runtime,
        IORingRuntimeAllocate::RingAllocationFailed() => return assert!(false),
        IORingRuntimeAllocate::PoolAllocationFailed() => return assert!(false),
        IORingRuntimeAllocate::RegistryAllocationFailed() => return assert!(false),
    };

    async fn callback(ops: IORuntimeOps) -> Option<&'static [u8]> {
        let (rx, tx) = ops.channel_create::<BoxedNumber>(1).unwrap();
        let (mut rx, mut tx) = (rx.droplet(), tx.droplet());

        ops.spawn(|ops: IORuntimeOps| async move {
            for i in 1..11 {
                ops.channel_write(&mut tx, BoxedNumber { x: i, y: 2 * i })
                    .await
                    .unwrap();
            }

            ops.channel_close(tx).await.unwrap();
            None
        })
        .unwrap();

        ops.spawn(|ops: IORuntimeOps| async move {
            let (mut x, mut y) = (0, 0);

            while let Some(item) = ops.channel_read(&mut rx).await {
                let (number, receipt) = item.unwrap();
                let receipt = receipt.droplet();

                x += number.x;
                y += number.y;

                ops.channel_ack(receipt).await.unwrap();
            }

            ops.channel_close(ops.channel_drain(rx).await.unwrap()).await.unwrap();

            assert_eq!(x, 55);
            assert_eq!(y, 110);

            None
        })
        .unwrap();

        None
    }

    match runtime.run(callback) {
        IORingRuntimeRun::Completed(res) => assert!(res.is_none()),
        IORingRuntimeRun::CompletionFailed(_) => assert!(false),
        IORingRuntimeRun::AllocationFailed(_) => assert!(false),
        IORingRuntimeRun::InternallyFailed() => assert!(false),
    }
}

#[test]
fn asynchronous_flow_interrupted() {
    let mut runtime = match IORingRuntime::allocate() {
        IORingRuntimeAllocate::Succeeded(runtime) => runtime,
        IORingRuntimeAllocate::RingAllocationFailed() => return assert!(false),
        IORingRuntimeAllocate::PoolAllocationFailed() => return assert!(false),
        IORingRuntimeAllocate::RegistryAllocationFailed() => return assert!(false),
    };

    async fn callback(ops: IORuntimeOps) -> Option<&'static [u8]> {
        let (rx, tx) = ops.channel_create::<BoxedNumber>(1).unwrap();
        let (mut rx, mut tx) = (rx.droplet(), tx.droplet());

        ops.spawn(|ops: IORuntimeOps| async move {
            for i in 1..11 {
                let res = ops
                    .channel_write(&mut tx, BoxedNumber { x: i, y: 2 * i })
                    .await
                    .unwrap();

                if i <= 5 {
                    assert!(res.is_none());
                }

                if i >= 7 {
                    assert!(res.is_some());
                }
            }

            ops.channel_close(tx).await.unwrap();
            None
        })
        .unwrap();

        ops.spawn(|ops: IORuntimeOps| async move {
            let (mut x, mut y) = (0, 0);

            while let Some(item) = ops.channel_read(&mut rx).await {
                let (number, receipt) = item.unwrap();
                let receipt = receipt.droplet();

                x += number.x;
                y += number.y;

                ops.channel_ack(receipt).await.unwrap();

                if number.x == 5 {
                    break;
                }
            }

            ops.channel_close(ops.channel_drain(rx).await.unwrap()).await.unwrap();

            assert_eq!(x, 15);
            assert_eq!(y, 30);

            None
        })
        .unwrap();

        None
    }

    match runtime.run(callback) {
        IORingRuntimeRun::Completed(res) => assert!(res.is_none()),
        IORingRuntimeRun::CompletionFailed(_) => assert!(false),
        IORingRuntimeRun::AllocationFailed(_) => assert!(false),
        IORingRuntimeRun::InternallyFailed() => assert!(false),
    }
}
