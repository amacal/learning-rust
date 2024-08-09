use ::core::future::*;
use ::core::marker::*;
use ::core::mem;
use ::core::ops::*;
use ::core::pin::*;
use ::core::task::*;

use super::callable::*;
use super::pollable::*;
use super::pool::*;
use super::refs::*;
use super::registry::*;
use super::spawn::*;
use super::token::*;
use crate::heap::*;
use crate::trace::*;
use crate::uring::*;

unsafe impl Send for IORuntimeContext {}

pub struct IORuntimeContext {
    pub heap_pool: Droplet<HeapPool<16>>,
    pub threads: Droplet<IORuntimePool<12>>,
    pub registry: Droplet<IORingRegistry<64, 256>>,
    ring: Droplet<IORing>,
}

impl IORuntimeContext {
    pub fn allocate(
        ring: Droplet<IORing>,
        threads: Droplet<IORuntimePool<12>>,
        registry: Droplet<IORingRegistry<64, 256>>,
    ) -> Option<Smart<Self>> {
        let ctx: Smart<IORuntimeContext> = match Smart::allocate() {
            Some(ctx) => ctx,
            None => return None,
        };

        Some(IORuntimeContext::initialize(ctx.duplicate(), ring, threads, registry))
    }

    fn initialize(
        mut ctx: Smart<Self>,
        mut ring: Droplet<IORing>,
        mut threads: Droplet<IORuntimePool<12>>,
        mut registry: Droplet<IORingRegistry<64, 256>>,
    ) -> Smart<Self> {
        trace1(b"initializing runtime context; uring=%d\n", ring.fd());
        let mut pool = HeapPool::new().droplet();

        mem::swap(&mut pool, &mut ctx.heap_pool);
        mem::forget(pool);

        mem::swap(&mut ring, &mut ctx.ring);
        mem::forget(ring);

        mem::swap(&mut threads, &mut ctx.threads);
        mem::forget(threads);

        mem::swap(&mut registry, &mut ctx.registry);
        mem::forget(registry);

        ctx
    }
}

impl IORuntimeContext {
    pub fn submit<const C: usize>(&mut self, user_data: u64, entries: [IORingSubmitEntry; C]) -> IORingSubmit {
        self.ring.tx.submit(user_data, entries)
    }

    pub fn flush(&mut self) -> IORingSubmit {
        self.ring.tx.flush()
    }

    pub fn receive<const T: usize>(&self, entries: &mut [IORingCompleteEntry; T]) -> IORingComplete {
        self.ring.rx.complete(entries)
    }

    pub fn poll(&mut self, task: &IORingTaskRef, cx: &mut Context<'_>) -> Option<(usize, Poll<Option<&'static [u8]>>)> {
        match self.registry.poll(&task, cx) {
            Err(_) => None,
            Ok((completers, status)) => Some((completers, status)),
        }
    }

    pub fn extract(&mut self, completer: &IORingCompleterRef) -> Option<i32> {
        trace2(b"extracting completer; cidx=%d, cid=%d\n", completer.cidx(), completer.cid());

        match self.registry.remove_completer(completer) {
            Err(_) => return None,
            Ok(completer) => completer.result(),
        }
    }


    pub fn ops(ctx: &mut Smart<Self>, task: IORingTaskRef) -> IORuntimeOps {
        IORuntimeOps {
            task: task.clone(),
            ctx: ctx.duplicate(),
        }
    }

    pub fn spawn<'a, F, C>(ctx: &mut Smart<Self>, callback: C) -> Option<IORingTaskRef>
    where
        F: Future<Output = Option<&'static [u8]>> + Send + 'a,
        C: FnOnce(IORuntimeOps) -> F + Unpin + Send + 'a,
    {
        let task = match ctx.registry.prepare_task() {
            Ok(val) => val,
            Err(_) => return None,
        };

        let ops = IORuntimeContext::ops(ctx, task);
        let target = callback.call_once((ops,));

        let target = match PollableTarget::allocate(&mut ctx.heap_pool, target) {
            Some(pinned) => pinned,
            None => return None,
        };

        Some(ctx.registry.append_task(task, target))
    }
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
    use ::core::ptr;

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

impl IORuntimeOps {
    pub fn spawn_io<'a, C, F>(&mut self, callback: C) -> Option<impl Future<Output = Result<(), ()>>>
    where
        F: Future<Output = Option<&'static [u8]>> + Send + 'a,
        C: FnOnce(IORuntimeOps) -> F + Unpin + Send + 'a,
    {
        let task = match self.ctx.registry.prepare_task() {
            Ok(val) => val,
            Err(_) => return None,
        };

        let ops = IORuntimeContext::ops(&mut self.ctx, task);
        let target = callback.call_once((ops,));

        Some(Spawn {
            task: task,
            callable: match PollableTarget::allocate(&mut self.ctx.heap_pool, target) {
                Some(task) => Some(task),
                None => None,
            },
        })
    }
}

impl IORuntimeOps {
    pub fn spawn_cpu<'a, F, R, E>(&mut self, target: F) -> Option<SpawnCPU<'a, F, R, E>>
    where
        F: FnOnce() -> Result<R, E> + Unpin + Send + 'a,
        R: Unpin + Send,
        E: Unpin + Send,
    {
        let task = match CallableTarget::allocate(&mut self.ctx.heap_pool, target) {
            Ok(target) => target,
            Err(_) => return None,
        };

        Some(SpawnCPU {
            ctx: self.ctx.duplicate(),
            queued: None,
            executed: None,
            phantom: PhantomData,
            task: Some(task),
        })
    }
}

struct NoopFuture {
    ops: IORuntimeOps,
    token: Option<IORingTaskToken>,
}

impl IORuntimeOps {
    pub fn noop(&self) -> impl Future<Output = Result<(), ()>> {
        NoopFuture {
            ops: self.duplicate(),
            token: None,
        }
    }
}

impl Future for NoopFuture {
    type Output = Result<(), ()>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace0(b"# polling noop\n");

        let (token, poll) = match this.token.take() {
            None => match this.ops.submit(IORingSubmitEntry::Noop()) {
                None => (None, Poll::Ready(Err(()))),
                Some(token) => (Some(token), Poll::Pending),
            },
            Some(token) => match token.extract_ctx(&mut this.ops.ctx) {
                Err(token) => (Some(token), Poll::Pending),
                Ok(_) => (None, Poll::Ready(Ok(()))),
            },
        };

        this.token = token;
        poll
    }
}
