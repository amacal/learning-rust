use ::core::marker::*;
use ::core::ops::*;

use super::*;
use crate::runtime::callable::*;
use crate::runtime::spawn::*;

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
