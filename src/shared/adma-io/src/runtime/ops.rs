use ::core::future::*;
use ::core::marker::*;
use ::core::mem;
use ::core::ops::*;

use super::callable::*;
use super::pollable::*;
use super::spawn::*;
use crate::heap::*;
use crate::trace::*;
use crate::uring::*;

unsafe impl Send for IORuntimeContext {}

pub struct IORuntimeContext {
    pub task_id: Option<usize>,
    pub heap_pool: Droplet<HeapPool<16>>,
    pub ring: Droplet<IORing>,
}

impl IORuntimeContext {
    fn initialize(mut ctx: Smart<Self>, mut ring: Droplet<IORing>) -> Smart<Self> {
        trace1(b"initializing runtime context; uring=%d\n", ring.fd());
        let pool = HeapPool::new();
        let mut pool = pool.droplet();

        mem::swap(&mut pool, &mut ctx.heap_pool);
        mem::forget(pool);

        mem::swap(&mut ring, &mut ctx.ring);
        mem::forget(ring);

        ctx.task_id = None;
        ctx
    }
}

pub struct IORuntimeOps {
    pub ctx: Smart<IORuntimeContext>,
}

impl IORuntimeOps {
    pub fn allocate(ring: Droplet<IORing>) -> Option<Self> {
        let ctx: Smart<IORuntimeContext> = match Smart::allocate() {
            Some(ctx) => ctx,
            None => return None,
        };

        Some(Self {
            ctx: IORuntimeContext::initialize(ctx, ring),
        })
    }

    pub fn duplicate(&self) -> IORuntimeOps {
        Self {
            ctx: self.ctx.duplicate(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_ops() {
        let ring = match IORing::init(8) {
            Ok(ring) => ring.droplet(),
            _ => return assert!(false),
        };

        let ops = match IORuntimeOps::allocate(ring) {
            None => return assert!(false),
            Some(ops) => ops,
        };

        drop(ops);
    }

    #[test]
    fn duplicates_ops() {
        let ring = match IORing::init(8) {
            Ok(ring) => ring.droplet(),
            _ => return assert!(false),
        };

        let first = match IORuntimeOps::allocate(ring) {
            None => return assert!(false),
            Some(ops) => ops,
        };

        let second = first.duplicate();
        assert!(first.ctx == second.ctx);
    }
}

impl IORuntimeOps {
    pub fn spawn_io<'a, C, F>(&mut self, target: C) -> Spawn
    where
        F: Future<Output = Option<&'static [u8]>> + Send + 'a,
        C: FnOnce(IORuntimeOps) -> F + Unpin + Send + 'a,
    {
        let ops = self.duplicate();
        let target = target.call_once((ops,));

        Spawn {
            task: match PollableTarget::allocate(&mut self.ctx.heap_pool, target) {
                Some(task) => Some(task),
                None => None,
            },
        }
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
