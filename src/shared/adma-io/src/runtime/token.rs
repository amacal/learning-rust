use ::core::task::Waker;

use super::callable::*;
use super::core::*;
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
    fn from_op(completer: IORingCompleterRef) -> Self {
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
        IORingRuntime::from_waker(waker)
    }

    pub fn cid(&self) -> u32 {
        self.completer.cid()
    }
}

pub enum IORingTaskTokenExtract {
    Succeeded(i32),
    Failed(IORingTaskToken),
}

impl IORingTaskToken {
    pub fn extract(self, waker: &Waker) -> IORingTaskTokenExtract {
        let context = Self::context(waker);
        let value = match context.extract(&self.completer) {
            IORingRuntimeExtract::Succeeded(value) => value,
            _ => return IORingTaskTokenExtract::Failed(self),
        };

        if let IORingTaskTokenKind::Queue = self.kind {
            // enqueue sent callable
            context.enqueue(&self.completer);
        }

        if let IORingTaskTokenKind::Execute = self.kind {
            // trigger awaiting callable
            context.trigger(&self.completer);
        }

        IORingTaskTokenExtract::Succeeded(value)
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
    pub fn spawn(waker: &Waker, task: PollableTarget) -> bool {
        match Self::context(waker).spawn(task) {
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
