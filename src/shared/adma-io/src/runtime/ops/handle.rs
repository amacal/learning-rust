use super::*;

impl IORuntimeHandle for IORuntimeOps {
    fn tid(&self) -> u32 {
        self.task.tid()
    }

    fn heap(&mut self) -> &mut HeapPool<16> {
        &mut self.ctx.heap
    }

    fn extract(&mut self, completer: &IORingCompleterRef) -> Result<Option<i32>, Option<i32>> {
        self.ctx.extract(completer)
    }

    fn complete_queue(&mut self, completer: &IORingCompleterRef) -> Result<(), Option<i32>> {
        self.ctx.enqueue(completer)
    }

    fn complete_execute(&mut self, completer: &IORingCompleterRef) -> Result<(), Option<i32>> {
        self.ctx.release(completer)
    }

    fn spawn<'a, TFuture, TFnOnce>(
        &mut self,
        callback: TFnOnce,
    ) -> Option<(Option<IORingTaskRef>, Option<&'static [u8]>)>
    where
        TFuture: Future<Output = Option<&'static [u8]>> + Send + 'a,
        TFnOnce: FnOnce(IORuntimeOps) -> TFuture + Unpin + Send + 'a,
    {
        IORuntimeContext::spawn(&mut self.ctx, callback)
    }

    fn submit(&mut self, op: IORingSubmitEntry) -> Option<IORingTaskToken> {
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

    fn schedule(&mut self, callable: &CallableTarget) -> Result<(IORingTaskToken, IORingTaskToken), Option<i32>> {
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
            Ok(cnt) => cnt,
            Err(errno) => return Err(errno),
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
