use core::task::Waker;

use super::core::*;
use super::pin::*;
use super::refs::*;
use crate::uring::*;

pub struct IORingTaskToken {
    completer: IORingCompleterRef,
}

impl IORingTaskToken {
    fn new(completer: IORingCompleterRef) -> Self {
        Self { completer }
    }

    fn context(waker: &Waker) -> &mut IORingRuntimeContext {
        IORingRuntime::from_waker(waker)
    }
}

pub enum IORingTaskTokenExtract {
    Succeeded(i32),
    Failed(IORingTaskToken),
}

impl IORingTaskToken {
    pub fn extract(self, waker: &Waker) -> IORingTaskTokenExtract {
        match Self::context(waker).extract(&self.completer) {
            IORingRuntimeExtract::Succeeded(value) => IORingTaskTokenExtract::Succeeded(value),
            IORingRuntimeExtract::NotCompleted() => IORingTaskTokenExtract::Failed(self),
            IORingRuntimeExtract::NotFound() => IORingTaskTokenExtract::Failed(self),
        }
    }
}

impl IORingTaskToken {
    pub fn submit<T: IORingSubmitBuffer>(waker: &Waker, entry: IORingSubmitEntry<T>) -> Option<IORingTaskToken> {
        match Self::context(waker).submit(entry) {
            IORingRuntimeSubmit::Succeeded(completer) => Some(IORingTaskToken::new(completer)),
            IORingRuntimeSubmit::SubmissionFailed(_) => None,
            IORingRuntimeSubmit::InternallyFailed() => None,
            IORingRuntimeSubmit::NotEnoughSlots() => None,
        }
    }
}

impl IORingTaskToken {
    pub fn spawn(waker: &Waker, pinned: IORingPin) -> bool {
        match Self::context(waker).spawn(pinned) {
            IORingRuntimeSpawn::Pending(_) => true,
            _ => false,
        }
    }
}
