mod close;
mod ctx;
mod execute;
mod noop;
mod open;
mod read;
mod spawn;
mod std;
mod time;
mod write;
mod handle;

use super::callable::*;
use super::pool::*;
use super::refs::*;
use super::registry::*;
use super::token::*;
use crate::heap::*;
use crate::trace::*;
use crate::uring::*;

pub struct IORuntimeContext {
    heap: Droplet<HeapPool<16>>,
    threads: Droplet<IORuntimePool<12>>,
    pub registry: Droplet<IORingRegistry<64, 256>>,
    ring: Droplet<IORing>,
}

pub struct IORuntimeOps {
    task: IORingTaskRef,
    ctx: Smart<IORuntimeContext>,
}

struct IORuntimeHandle {
    task: IORingTaskRef,
    ctx: Smart<IORuntimeContext>,
}

impl IORuntimeOps {
    fn handle(&self) -> IORuntimeHandle {
        IORuntimeHandle {
            task: self.task,
            ctx: self.ctx.duplicate(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::pollable::*;

    use ::core::ptr;
    use ::core::task::*;

    const NOOP: RawWaker = RawWaker::new(ptr::null(), &VTABLE);
    const VTABLE: RawWakerVTable = RawWakerVTable::new(|_| NOOP, |_| {}, |_| {}, |_| {});

    #[test]
    fn allocates_context() {
        let ring = match IORing::init(8) {
            Ok(ring) => ring.droplet(),
            Err(_) => return assert!(false),
        };

        let threads = match IORuntimePool::allocate() {
            Ok(threads) => threads.droplet(),
            Err(_) => return assert!(false),
        };

        let registry = match IORingRegistry::allocate() {
            Ok(threads) => threads.droplet(),
            Err(_) => return assert!(false),
        };

        let ctx = match IORuntimeContext::allocate(ring, threads, registry) {
            None => return assert!(false),
            Some(ctx) => ctx,
        };

        drop(ctx);
    }

    #[test]
    fn duplicates_context() {
        let ring = match IORing::init(8) {
            Ok(ring) => ring.droplet(),
            Err(_) => return assert!(false),
        };

        let threads = match IORuntimePool::allocate() {
            Ok(threads) => threads.droplet(),
            Err(_) => return assert!(false),
        };

        let registry = match IORingRegistry::allocate() {
            Ok(threads) => threads.droplet(),
            Err(_) => return assert!(false),
        };

        let first = match IORuntimeContext::allocate(ring, threads, registry) {
            None => return assert!(false),
            Some(ctx) => ctx,
        };

        let second = first.duplicate();
        assert!(first == second);
    }

    #[test]
    fn submits_op_via_ops() {
        let ring = match IORing::init(8) {
            Ok(ring) => ring.droplet(),
            Err(_) => return assert!(false),
        };

        let threads = match IORuntimePool::allocate() {
            Ok(threads) => threads.droplet(),
            Err(_) => return assert!(false),
        };

        let mut registry = match IORingRegistry::allocate() {
            Ok(threads) => threads.droplet(),
            Err(_) => return assert!(false),
        };

        let mut pool = HeapPool::<1>::new();
        let target = async { None::<&'static [u8]> };

        let target = match PollableTarget::allocate(&mut pool, target) {
            None => return assert!(false),
            Some(target) => target,
        };

        let op = IORingSubmitEntry::noop();
        let task = match registry.prepare_task() {
            Err(_) => return assert!(false),
            Ok(task) => registry.append_task(task, target),
        };

        let ops = match IORuntimeContext::allocate(ring, threads, registry) {
            None => return assert!(false),
            Some(mut ctx) => IORuntimeContext::ops(&mut ctx, task),
        };

        let token = match ops.handle().submit(op) {
            None => return assert!(false),
            Some(token) => token,
        };

        assert_eq!(token.cid(), 1);
    }

    #[test]
    fn polls_op_via_pollable() {
        let ring = match IORing::init(8) {
            Ok(ring) => ring.droplet(),
            Err(_) => return assert!(false),
        };

        let threads = match IORuntimePool::allocate() {
            Ok(threads) => threads.droplet(),
            Err(_) => return assert!(false),
        };

        let mut registry = match IORingRegistry::allocate() {
            Ok(threads) => threads.droplet(),
            Err(_) => return assert!(false),
        };

        let task = match registry.prepare_task() {
            Err(_) => return assert!(false),
            Ok(task) => task,
        };

        let (ops, mut ctx) = match IORuntimeContext::allocate(ring, threads, registry) {
            None => return assert!(false),
            Some(mut ctx) => (IORuntimeContext::ops(&mut ctx, task), ctx),
        };

        let mut pool = HeapPool::<1>::new();
        let target = async move {
            match ops.noop().await {
                Ok(()) => assert!(true),
                Err(_) => assert!(false),
            }

            None::<&'static [u8]>
        };

        let target = match PollableTarget::allocate(&mut pool, target) {
            None => return assert!(false),
            Some(target) => target,
        };

        let waker = unsafe { Waker::from_raw(NOOP) };
        let mut cx = Context::from_waker(&waker);

        let task = ctx.registry.append_task(task, target);
        let (completers, poll) = match ctx.poll(&task, &mut cx) {
            Some((completers, poll)) => (completers, poll),
            None => return assert!(false),
        };

        assert_eq!(completers, 1);
        assert_eq!(poll, Poll::Pending);

        match ctx.flush() {
            IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
            _ => assert!(false),
        }

        let mut entries = [IORingCompleteEntry::default(); 1];
        match ctx.receive(&mut entries) {
            IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
            _ => assert!(false),
        }

        let completer = IORingCompleterRef::decode(entries[0].user_data);
        match ctx.registry.complete(&completer, 17) {
            Ok((_, ready, _)) => assert_eq!(ready, false),
            Err(_) => assert!(false),
        }

        let (completers, poll) = match ctx.poll(&task, &mut cx) {
            Some((completers, poll)) => (completers, poll),
            None => return assert!(false),
        };

        assert_eq!(completers, 0);
        assert_eq!(poll, Poll::Ready(None));
    }
}
