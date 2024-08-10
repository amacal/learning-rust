use ::core::future::*;
use ::core::mem;
use ::core::task::*;

use super::*;
use crate::runtime::pollable::*;
use crate::trace::*;

unsafe impl Send for IORuntimeContext {}

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
