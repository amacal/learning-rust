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
    pub fn extract_ctx(self, ctx: &mut IORuntimeContext) -> Result<(Option<i32>, Option<IORingTaskToken>), Option<i32>> {
        let value = match ctx.extract(&self.completer) {
            Ok(Some(value)) => value,
            Ok(None) => return Ok((None, Some(self))),
            Err(err) => return Err(err),
        };

        if let IORingTaskTokenKind::Queue = self.kind {
            // enqueue sent callable
            if let Err(err) = ctx.enqueue(&self.completer) {
                return Err(err);
            }
        }

        if let IORingTaskTokenKind::Execute = self.kind {
            // trigger awaiting callable
            if let Err(err) = ctx.release(&self.completer) {
                return Err(err);
            }
        }

        Ok((Some(value), None))
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
