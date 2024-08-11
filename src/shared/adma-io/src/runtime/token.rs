use super::ops::*;
use super::refs::*;

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

    pub fn from_queue(completer: IORingCompleterRef) -> Self {
        Self {
            completer,
            kind: IORingTaskTokenKind::Queue,
        }
    }

    pub fn from_execute(completer: IORingCompleterRef) -> Self {
        Self {
            completer,
            kind: IORingTaskTokenKind::Execute,
        }
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
