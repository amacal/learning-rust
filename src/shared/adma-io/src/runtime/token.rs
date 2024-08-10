use ::core::task::Waker;

use super::callable::*;
use super::core::*;
use super::ops::*;
use super::pollable::*;
use super::refs::*;
use crate::uring::*;

pub struct IORingTaskToken {
    kind: IORingTaskTokenKind,
    completer: IORingCompleterRef,
}

enum IORingTaskTokenKind {
    Op,
    Queue,
    Execute,
}

impl IORingTaskToken {
    pub fn from_op(completer: IORingCompleterRef) -> Self {
        Self {
            completer,
            kind: IORingTaskTokenKind::Op,
        }
    }

    fn from_queue(completer: IORingCompleterRef) -> Self {
        Self {
            completer,
            kind: IORingTaskTokenKind::Queue,
        }
    }

    fn from_execute(completer: IORingCompleterRef) -> Self {
        Self {
            completer,
            kind: IORingTaskTokenKind::Execute,
        }
    }

    fn context(waker: &Waker) -> &mut IORingRuntimeContext {
        IORingRuntimeContext::from_waker(waker)
    }

    pub fn cid(&self) -> u32 {
        self.completer.cid()
    }
}

impl IORingTaskToken {
    pub fn extract(self, waker: &Waker) -> Result<i32, IORingTaskToken> {
        let context = Self::context(waker);
        let value = match context.extract(&self.completer) {
            IORingRuntimeExtract::Succeeded(value) => value,
            _ => return Err(self),
        };

        if let IORingTaskTokenKind::Queue = self.kind {
            // enqueue sent callable
            context.enqueue(&self.completer);
        }

        if let IORingTaskTokenKind::Execute = self.kind {
            // trigger awaiting callable
            context.trigger(&self.completer);
        }

        Ok(value)
    }

    pub fn extract_ctx(self, ctx: &mut IORuntimeContext) -> Result<i32, IORingTaskToken> {
        let value = match ctx.extract(&self.completer) {
            Some(value) => value,
            None => return Err(self),
        };

        if let IORingTaskTokenKind::Queue = self.kind {
            // enqueue sent callable
            //ops.enqueue(&self.completer);
        }

        if let IORingTaskTokenKind::Execute = self.kind {
            // trigger awaiting callable
            //ops.trigger(&self.completer);
        }

        Ok(value)
    }
}

impl IORingTaskToken {
    pub fn submit(waker: &Waker, entry: IORingSubmitEntry) -> Option<IORingTaskToken> {
        match Self::context(waker).submit(entry) {
            IORingRuntimeSubmit::Succeeded(completer) => Some(IORingTaskToken::from_op(completer)),
            IORingRuntimeSubmit::SubmissionFailed(_) => None,
            IORingRuntimeSubmit::InternallyFailed() => None,
            IORingRuntimeSubmit::NotEnoughSlots() => None,
        }
    }
}

impl IORingTaskToken {
    pub fn spawn(waker: &Waker, task: IORingTaskRef, callable: PollableTarget) -> bool {
        match Self::context(waker).spawn(task, callable) {
            IORingRuntimeSpawn::Pending(_) => true,
            _ => false,
        }
    }
}

impl IORingTaskToken {
    pub fn execute(waker: &Waker, task: &CallableTarget) -> Option<(IORingTaskToken, IORingTaskToken)> {
        match Self::context(waker).execute(task) {
            IORingRuntimeExecute::Queued(queued, executed) => {
                Some((IORingTaskToken::from_queue(queued), IORingTaskToken::from_execute(executed)))
            }
            IORingRuntimeExecute::Executed(queued, executed) => {
                Some((IORingTaskToken::from_op(queued), IORingTaskToken::from_execute(executed)))
            }
            IORingRuntimeExecute::NotEnoughSlots() => None,
            IORingRuntimeExecute::InternallyFailed() => None,
        }
    }
}
