use ::core::future::*;
use ::core::marker::*;

use super::callable::*;
use super::pin::*;
use super::spawn::*;
use crate::heap::*;

pub struct IORuntimeContext {
    pub task_id: Option<usize>,
    pub heap_pool: HeapPool<16>,
}

impl IORuntimeContext {
    pub fn new() -> Self {
        Self {
            task_id: None,
            heap_pool: HeapPool::new(),
        }
    }
}

pub struct IORuntimeOps {
    pub ctx: Smart<IORuntimeContext>,
}

impl IORuntimeOps {
    pub fn allocate() -> Option<Self> {
        Some(Self {
            ctx: match Smart::allocate() {
                None => return None,
                Some(ptr) => ptr,
            },
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
        assert!(IORuntimeOps::allocate().is_some());
    }

    #[test]
    fn duplicates_ops() {
        let first = match IORuntimeOps::allocate() {
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
            task: match IORingPin::allocate(&mut self.ctx.heap_pool, target) {
                IORingPinAllocate::Succeeded(task) => Some(task),
                IORingPinAllocate::AllocationFailed(_) => None,
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
            CallableTargetAllocate::Succeeded(target) => target,
            CallableTargetAllocate::AllocationFailed(_) => return None,
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
