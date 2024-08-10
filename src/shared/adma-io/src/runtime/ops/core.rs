use ::core::future::*;
use ::core::marker::*;
use ::core::ops::*;

use super::*;
use crate::runtime::callable::*;
use crate::runtime::pollable::*;
use crate::runtime::spawn::*;

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
            callable: match PollableTarget::allocate(&mut self.ctx.heap, target) {
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
        let task = match CallableTarget::allocate(&mut self.ctx.heap, target) {
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
