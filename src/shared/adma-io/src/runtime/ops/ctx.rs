use ::core::future::*;
use ::core::mem;
use ::core::task::*;

use super::*;
use crate::runtime::callable::*;
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

        mem::swap(&mut pool, &mut ctx.heap);
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

    pub fn extract(&mut self, completer: &IORingCompleterRef) -> Result<Option<i32>, Option<i32>> {
        trace2(b"extracting completer; cidx=%d, cid=%d\n", completer.cidx(), completer.cid());

        match self.registry.remove_completer(completer) {
            Ok(completer) => Ok(completer.result()),
            Err(IORegistryError::CompleterNotReady) => return Ok(None),
            Err(_) => return Err(None),
        }
    }

    pub fn ops(ctx: &mut Smart<Self>, task: IORingTaskRef) -> IORuntimeOps {
        IORuntimeOps {
            task: task.clone(),
            ctx: ctx.duplicate(),
        }
    }

    pub fn spawn<'a, TFuture, TFnOnce>(
        ctx: &mut Smart<Self>,
        callback: TFnOnce,
        cx: &mut Context<'_>,
    ) -> Option<(Option<IORingTaskRef>, Option<&'static [u8]>)>
    where
        TFuture: Future<Output = Option<&'static [u8]>> + Send + 'a,
        TFnOnce: FnOnce(IORuntimeOps) -> TFuture + Unpin + Send + 'a,
    {
        let task = match ctx.registry.prepare_task() {
            Ok(val) => val,
            Err(_) => return None,
        };

        let ops = IORuntimeContext::ops(ctx, task);
        let target = callback.call_once((ops,));

        let target = match PollableTarget::allocate(&mut ctx.heap, target) {
            Some(pinned) => pinned,
            None => return None,
        };

        Some(ctx.registry.append_task(task, target));

        // to be initially polled
        let (result, completions) = match ctx.poll(&task, cx) {
            Some((cnt, Poll::Ready(val))) => (val, cnt),
            Some((_, Poll::Pending)) => return Some((Some(task), None)),
            None => return None,
        };

        if completions == 0 {
            // to be immediately removed if ready without hanging completers
            let result = match ctx.registry.remove_task(&task) {
                Ok(task) => task.release(),
                Err(_) => return None,
            };

            match result {
                None => trace1(b"task completed; tid=%d\n", task.tid()),
                Some(res) => trace2(b"task completed; tid=%d, res='%s'\n", task.tid(), res),
            }

            return Some((None, result));
        }

        match result {
            None => trace1(b"task draining; tid=%d\n", task.tid()),
            Some(res) => trace2(b"task draining; tid=%d, res='%s'\n", task.tid(), res),
        }

        // otherwise we left it in a draining mode
        Some((Some(task), None))
    }

    pub fn enqueue(&mut self, completer: &IORingCompleterRef) -> Result<(), Option<i32>> {
        self.threads.enqueue(completer);
        self.trigger()
    }

    pub fn release(&mut self, completer: &IORingCompleterRef) -> Result<(), Option<i32>> {
        self.threads.release(completer);
        self.trigger()
    }

    fn trigger(&mut self) -> Result<(), Option<i32>> {
        let mut slots: [Option<(u64, IORingSubmitEntry)>; 1] = [const { None }; 1];

        // possibly it will be triggered now
        let cnt = match self.threads.trigger(&mut slots) {
            Ok(None) | Ok(Some(0)) => 0,
            Ok(Some(cnt)) => cnt,
            Err(err) => return Err(err),
        };

        // potentially received submits has to be processed
        for index in 0..cnt {
            let (user_data, entry) = unsafe {
                match slots.get_unchecked_mut(index).take() {
                    None => continue,
                    Some((user_data, entry)) => (user_data, entry),
                }
            };

            self.submit(user_data, [entry]);
        }

        Ok(())
    }

    pub fn schedule(
        &mut self,
        task: &IORingTaskRef,
        callable: &CallableTarget,
    ) -> Result<(IORingTaskToken, IORingTaskToken), Option<i32>> {
        let queued = match self.registry.append_completer(task) {
            Ok(completer) => completer,
            Err(IORegistryError::NotEnoughSlots) => return Err(None),
            Err(_) => return Err(None),
        };

        let executed = match self.registry.append_completer(task) {
            Ok(completer) => completer,
            Err(IORegistryError::NotEnoughSlots) => return Err(None),
            Err(_) => return Err(None),
        };

        let mut slots: [Option<(u64, IORingSubmitEntry)>; 4] = [const { None }; 4];
        let cnt = match self.threads.execute(&mut slots, [&queued, &executed], callable) {
            Ok(Some(cnt)) => cnt,
            Ok(None) => 0,
            Err(()) => return Err(None),
        };

        // potentially received submits has to be processed
        for index in 0..cnt {
            let (user_data, entry) = unsafe {
                match slots.get_unchecked_mut(index).take() {
                    None => continue,
                    Some((user_data, entry)) => (user_data, entry),
                }
            };

            self.submit(user_data, [entry]);
        }

        if cnt == 1 {
            Ok((IORingTaskToken::from_queue(queued), IORingTaskToken::from_execute(executed)))
        } else {
            Ok((IORingTaskToken::from_op(queued), IORingTaskToken::from_execute(executed)))
        }
    }
}
