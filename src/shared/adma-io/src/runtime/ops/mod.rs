mod channel;
mod close;
mod context;
mod execute;
mod file;
mod handle;
mod mem;
mod noop;
mod open;
mod pipe;
mod read;
mod spawn;
mod std;
mod time;
mod token;
mod write;

use ::core::future::*;
use ::core::marker::*;
use ::core::pin::*;
use ::core::task::*;

use super::callable::*;
use super::pollable::*;
use super::pool::*;
use super::refs::*;
use super::registry::*;
use crate::core::*;
use crate::heap::*;
use crate::trace::*;
use crate::uring::*;

pub use file::*;
pub use mem::*;
pub use channel::*;

pub struct IORuntimeContext {
    heap: Droplet<HeapPool<16>>,
    threads: Droplet<IORuntimePool<1>>,
    pub registry: Droplet<IORingRegistry<64, 256>>,
    ring: Droplet<IORing>,
}

pub struct IORuntimeOps {
    task: IORingTaskRef,
    ctx: Smart<IORuntimeContext>,
    none: PhantomData<()>,
}

struct IORingTaskToken {
    kind: IORingTaskTokenKind,
    completer: IORingCompleterRef,
}

enum IORingTaskTokenKind {
    Op,
    Queue,
    Execute,
}

trait IORuntimeHandle {
    fn tid(&self) -> u32;
    fn heap(&mut self) -> &mut HeapPool<16>;

    fn submit(&mut self, op: IORingSubmitEntry) -> Option<IORingTaskToken>;
    fn extract(&mut self, completer: &IORingCompleterRef) -> Result<Option<i32>, Option<i32>>;

    fn schedule(&mut self, callable: &CallableTarget) -> Result<(IORingTaskToken, IORingTaskToken), Option<i32>>;
    fn complete_queue(&mut self, completer: &IORingCompleterRef) -> Result<(), Option<i32>>;
    fn complete_execute(&mut self, completer: &IORingCompleterRef) -> Result<(), Option<i32>>;

    fn spawn<'a, TFuture, TFnOnce>(
        &mut self,
        callback: TFnOnce,
    ) -> Option<(Option<IORingTaskRef>, Option<&'static [u8]>)>
    where
        TFuture: Future<Output = Option<&'static [u8]>> + Send + 'a,
        TFnOnce: FnOnce(IORuntimeOps) -> TFuture + Unpin + Send + 'a;
}

impl IORuntimeOps {
    fn duplicate(&self) -> Self {
        Self {
            task: self.task,
            ctx: self.ctx.duplicate(),
            none: PhantomData,
        }
    }

    fn handle(&self) -> impl IORuntimeHandle {
        Self {
            task: self.task,
            ctx: self.ctx.duplicate(),
            none: PhantomData,
        }
    }
}

unsafe impl Sync for IORuntimeOps {}

#[cfg(test)]
mod tests {
    use super::*;

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

        let task = ctx.registry.append_task(task, target);
        let (completers, poll) = match ctx.poll(&task) {
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

        let (completers, poll) = match ctx.poll(&task) {
            Some((completers, poll)) => (completers, poll),
            None => return assert!(false),
        };

        assert_eq!(completers, 0);
        assert_eq!(poll, Poll::Ready(None));
    }
}
