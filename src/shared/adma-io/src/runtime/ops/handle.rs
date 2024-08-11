use super::*;

impl IORuntimeHandle {
    pub fn submit(&mut self, op: IORingSubmitEntry) -> Option<IORingTaskToken> {
        trace1(b"appending completer to registry; tid=%d\n", self.task.tid());
        let completer = match self.ctx.registry.append_completer(&self.task) {
            Ok(completer) => completer,
            Err(_) => return None,
        };

        let (user_data, entries) = (completer.encode(), [op]);
        trace2(b"submitting op with uring; cidx=%d, cid=%d\n", completer.cidx(), completer.cid());

        match self.ctx.ring.tx.submit(user_data, entries) {
            IORingSubmit::Succeeded(_) => {
                trace1(b"submitting op with uring; cidx=%d, succeeded\n", completer.cidx());
                return Some(IORingTaskToken::from_op(completer));
            }
            IORingSubmit::SubmissionFailed(err) => {
                trace2(b"submitting op with uring; cidx=%d, err=%d\n", completer.cidx(), err);
                None
            }
            IORingSubmit::SubmissionMismatched(_) => {
                trace1(b"submitting op with uring; cidx=%d, failed\n", completer.cidx());
                None
            }
        }
    }

    pub fn schedule(
        &mut self,
        callable: &CallableTarget,
    ) -> Result<(IORingTaskToken, IORingTaskToken), Option<i32>> {
        let queued = match self.ctx.registry.append_completer(&self.task) {
            Ok(completer) => completer,
            Err(IORegistryError::NotEnoughSlots) => return Err(None),
            Err(_) => return Err(None),
        };

        let executed = match self.ctx.registry.append_completer(&self.task) {
            Ok(completer) => completer,
            Err(IORegistryError::NotEnoughSlots) => return Err(None),
            Err(_) => return Err(None),
        };

        let mut slots: [Option<(u64, IORingSubmitEntry)>; 4] = [const { None }; 4];
        let cnt = match self.ctx.threads.execute(&mut slots, [&queued, &executed], callable) {
            Ok(Some(cnt)) => cnt,
            Ok(None) => 0,
            Err(()) => return Err(None),
        };

        // potentially received submits has to be processed
        for index in 0..cnt {
            let (user_data, entry) = unsafe {
                match slots.get_unchecked_mut(index).take() {
                    None => continue,
                    Some((user_data, entry)) => (user_data, entry),
                }
            };

            self.ctx.ring.tx.submit(user_data, [entry]);
        }

        if cnt == 1 {
            Ok((IORingTaskToken::from_queue(queued), IORingTaskToken::from_execute(executed)))
        } else {
            Ok((IORingTaskToken::from_op(queued), IORingTaskToken::from_execute(executed)))
        }
    }

}
