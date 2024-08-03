use ::core::future::*;
use ::core::marker::*;

use super::callable::*;
use super::pollable::*;
use super::spawn::*;
use crate::heap::*;
use crate::uring::*;

pub struct IORuntimeSubmitterInContext(pub IORingSubmitter);
unsafe impl Send for IORuntimeContext {}

pub struct IORuntimeContext {
    pub task_id: Option<usize>,
    pub heap_pool: HeapPool<16>,
    pub submitter: IORuntimeSubmitterInContext,
}

impl IORuntimeContext {
    fn initialize(mut ctx: Smart<Self>, submitter: IORingSubmitter) -> Smart<Self> {
        ctx.task_id = None;
        ctx.heap_pool = HeapPool::new();
        ctx.submitter = IORuntimeSubmitterInContext(submitter);

        ctx
    }
}

pub struct IORuntimeOps {
    pub ctx: Smart<IORuntimeContext>,
}

impl IORuntimeOps {
    pub fn allocate(submitter: IORingSubmitter) -> Option<Self> {
        let ctx: Smart<IORuntimeContext> = match Smart::allocate() {
            None => return None,
            Some(ctx) => ctx,
        };

        Some(Self {
            ctx: IORuntimeContext::initialize(ctx, submitter),
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
        let (_, tx) = match IORing::init(8) {
            Ok((tx, rx)) => (rx, tx),
            _ => return assert!(false),
        };

        assert!(IORuntimeOps::allocate(tx).is_some());
    }

    #[test]
    fn duplicates_ops() {
        let (_, tx) = match IORing::init(8) {
            Ok((tx, rx)) => (rx, tx),
            _ => return assert!(false),
        };

        let first = match IORuntimeOps::allocate(tx) {
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
