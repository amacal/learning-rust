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
        let _ = ops.channel_create::<BoxedNumber>(1).unwrap();
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
        let (mut rx, mut tx) = ops.channel_create::<BoxedNumber>(1).unwrap();

        ops.channel_write(&mut tx, BoxedNumber { x: 1, y: 2 }).await.unwrap();
        let (number, receipt) = ops.channel_read(&mut rx).await.unwrap().unwrap();

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
        let (mut rx, mut tx) = ops.channel_create::<BoxedNumber>(1).unwrap();

        ops.spawn_unwrap(|ops: IORuntimeOps| async move {
            for i in 1..11 {
                let data = BoxedNumber { x: i, y: 2 * i };
                ops.channel_write(&mut tx, data).await.unwrap();
            }

            ops.channel_close(tx).await.unwrap();
            None
        });

        ops.spawn_unwrap(|ops: IORuntimeOps| async move {
            let (mut x, mut y) = (0, 0);

            while let Some(item) = ops.channel_read(&mut rx).await {
                let (number, receipt) = item.unwrap();

                x += number.x;
                y += number.y;

                ops.channel_ack(receipt).await.unwrap();
            }

            ops.channel_close(ops.channel_drain(rx).await.unwrap()).await.unwrap();

            assert_eq!(x, 55);
            assert_eq!(y, 110);

            None
        });

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
fn asynchronous_flow_with_ten_items_where_receipt_isnt_acknowledged_explicitely() {
    let mut runtime = match IORingRuntime::allocate() {
        IORingRuntimeAllocate::Succeeded(runtime) => runtime,
        IORingRuntimeAllocate::RingAllocationFailed() => return assert!(false),
        IORingRuntimeAllocate::PoolAllocationFailed() => return assert!(false),
        IORingRuntimeAllocate::RegistryAllocationFailed() => return assert!(false),
    };

    async fn callback(ops: IORuntimeOps) -> Option<&'static [u8]> {
        let (mut rx, mut tx) = ops.channel_create::<BoxedNumber>(1).unwrap();

        ops.spawn_unwrap(|ops: IORuntimeOps| async move {
            for i in 1..11 {
                let data = BoxedNumber { x: i, y: 2 * i };
                ops.channel_write(&mut tx, data).await.unwrap();
            }

            ops.channel_close(tx).await.unwrap();
            None
        });

        ops.spawn_unwrap(|ops: IORuntimeOps| async move {
            let (mut x, mut y) = (0, 0);

            while let Some(item) = ops.channel_read(&mut rx).await {
                let (number, _) = item.unwrap();

                x += number.x;
                y += number.y;
            }

            ops.channel_close(ops.channel_drain(rx).await.unwrap()).await.unwrap();

            assert_eq!(x, 55);
            assert_eq!(y, 110);

            None
        });

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
fn asynchronous_flow_with_ten_items_interrupted() {
    let mut runtime = match IORingRuntime::allocate() {
        IORingRuntimeAllocate::Succeeded(runtime) => runtime,
        IORingRuntimeAllocate::RingAllocationFailed() => return assert!(false),
        IORingRuntimeAllocate::PoolAllocationFailed() => return assert!(false),
        IORingRuntimeAllocate::RegistryAllocationFailed() => return assert!(false),
    };

    async fn callback(ops: IORuntimeOps) -> Option<&'static [u8]> {
        let (mut rx, mut tx) = ops.channel_create::<BoxedNumber>(1).unwrap();

        ops.spawn_unwrap(|ops: IORuntimeOps| async move {
            for i in 1..11 {
                let data = BoxedNumber { x: i, y: 2 * i };
                let res = ops.channel_write(&mut tx, data).await.unwrap();

                if i <= 5 {
                    assert!(res.is_none());
                }

                if i >= 7 {
                    assert!(res.is_some());
                }
            }

            ops.channel_close(tx).await.unwrap();
            None
        });

        ops.spawn_unwrap(|ops: IORuntimeOps| async move {
            let (mut x, mut y) = (0, 0);

            while let Some(item) = ops.channel_read(&mut rx).await {
                let (number, receipt) = item.unwrap();

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
        });

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
fn never_produced_any_item() {
    let mut runtime = match IORingRuntime::allocate() {
        IORingRuntimeAllocate::Succeeded(runtime) => runtime,
        IORingRuntimeAllocate::RingAllocationFailed() => return assert!(false),
        IORingRuntimeAllocate::PoolAllocationFailed() => return assert!(false),
        IORingRuntimeAllocate::RegistryAllocationFailed() => return assert!(false),
    };

    async fn callback(ops: IORuntimeOps) -> Option<&'static [u8]> {
        let (mut rx, tx) = ops.channel_create::<BoxedNumber>(1).unwrap();

        ops.channel_close(tx).await.unwrap();
        ops.spawn_unwrap(|ops: IORuntimeOps| async move {
            while let Some(_) = ops.channel_read(&mut rx).await {
                assert!(false);
            }

            ops.channel_close(ops.channel_drain(rx).await.unwrap()).await.unwrap();
            None
        });

        None
    }

    match runtime.run(callback) {
        IORingRuntimeRun::Completed(res) => assert!(res.is_none()),
        IORingRuntimeRun::CompletionFailed(_) => assert!(false),
        IORingRuntimeRun::AllocationFailed(_) => assert!(false),
        IORingRuntimeRun::InternallyFailed() => assert!(false),
    }
}
