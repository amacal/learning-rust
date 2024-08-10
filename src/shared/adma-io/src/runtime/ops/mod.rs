mod close;
mod core;
mod ctx;
mod noop;
mod open;
mod read;
mod std;
mod write;
mod time;

use super::pool::*;
use super::refs::*;
use super::registry::*;
use super::token::*;
use crate::heap::*;
use crate::trace::*;
use crate::uring::*;

pub struct IORuntimeContext {
    pub heap: Droplet<HeapPool<16>>,
    pub threads: Droplet<IORuntimePool<12>>,
    pub registry: Droplet<IORingRegistry<64, 256>>,
    ring: Droplet<IORing>,
}

pub struct IORuntimeOps {
    task: IORingTaskRef,
    pub ctx: Smart<IORuntimeContext>,
}

impl IORuntimeOps {
    pub fn tid(&self) -> u32 {
        self.task.tid()
    }

    pub fn duplicate(&self) -> IORuntimeOps {
        Self {
            task: self.task,
            ctx: self.ctx.duplicate(),
        }
    }

    pub fn submit(&mut self, op: IORingSubmitEntry) -> Option<IORingTaskToken> {
        trace1(b"appending completer to registry; tid=%d\n", self.task.tid());
        let completer = match self.ctx.registry.append_completer(&self.task) {
            Ok(completer) => completer,
            Err(_) => return None,
        };

        let (user_data, entries) = (completer.encode(), [op]);
        trace2(b"submitting op with uring; cidx=%d, cid=%d\n", completer.cidx(), completer.cid());

        match self.ctx.submit(user_data, entries) {
            IORingSubmit::Succeeded(_) => {
                trace1(b"submitting op with uring; cidx=%d, succeeded\n", completer.cidx());
                return Some(IORingTaskToken::from_op(completer));
            }
            IORingSubmit::SubmissionFailed(err) => {
                trace2(b"submitting op with uring; cidx=%d, err=%d\n", completer.cidx(), err);
                None
            }
            IORingSubmit::SubmissionMismatched(_) => {
                trace1(b"submitting op with uring; cidx=%d, failed\n", completer.cidx());
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::pollable::*;

    use ::core::ptr;
    use ::core::task::*;

    const VTABLE: RawWakerVTable = RawWakerVTable::new(|_| NOOP, |_| {}, |_| {}, |_| {});
    const NOOP: RawWaker = RawWaker::new(ptr::null(), &VTABLE);

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

        let mut ops = match IORuntimeContext::allocate(ring, threads, registry) {
            None => return assert!(false),
            Some(mut ctx) => IORuntimeContext::ops(&mut ctx, task),
        };

        let token = match ops.submit(op) {
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
                Err(()) => assert!(false),
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
