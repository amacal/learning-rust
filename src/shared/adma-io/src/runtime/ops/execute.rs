use super::*;

impl IORuntimeOps {
    pub fn execute<'a, TFnOnce, TResult, TError>(
        &self,
        target: TFnOnce,
    ) -> impl Future<Output = Result<Result<TResult, TError>, Option<i32>>> + 'a
    where
        TFnOnce: FnOnce() -> Result<TResult, TError> + Unpin + Send + 'a,
        TResult: Unpin + Send + 'a,
        TError: Unpin + Send + 'a,
    {
        ExecuteFuture::<'a, _, TFnOnce, TResult, TError> {
            queued: None,
            executed: None,
            phantom: PhantomData,
            handle: self.handle(),
            task: Some(target),
            callable: None,
        }
    }
}

struct ExecuteFuture<'a, THandle, TFnOnce, TResult, TError>
where
    THandle: IORuntimeHandle + Unpin,
    TFnOnce: FnOnce() -> Result<TResult, TError> + Unpin + Send + 'a,
    TResult: Unpin + Send,
    TError: Unpin + Send,
{
    handle: THandle,
    task: Option<TFnOnce>,
    queued: Option<IORingTaskToken>,
    executed: Option<IORingTaskToken>,
    callable: Option<CallableTarget>,
    phantom: PhantomData<&'a (TFnOnce, TResult, TError)>,
}

impl<'a, THandle, TFnOnce, TResult, TError> Future for ExecuteFuture<'a, THandle, TFnOnce, TResult, TError>
where
    THandle: IORuntimeHandle + Unpin,
    TFnOnce: FnOnce() -> Result<TResult, TError> + Unpin + Send + 'a,
    TResult: Unpin + Send,
    TError: Unpin + Send,
{
    type Output = Result<Result<TResult, TError>, Option<i32>>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        trace1(b"# polling spawn-cpu; tid=%d\n", this.handle.tid());

        if let Some(token) = this.queued.take() {
            trace0(b"# polling spawn-cpu; extracting queued\n");
            let result = match token.extract(&mut this.handle) {
                Ok((Some(value), None)) => Some(value),
                Ok((None, Some(token))) => {
                    this.queued = Some(token);
                    None
                }
                Ok(_) | Err(None) => return Poll::Ready(Err(None)),
                Err(err) => return Poll::Ready(Err(err)),
            };

            if let Some(result) = result {
                return if result < 0 {
                    Poll::Ready(Err(Some(result)))
                } else {
                    trace1(b"# polling spawn-cpu; stage=queued, res=%d\n", result);
                    Poll::Pending
                };
            }
        }

        if let Some(token) = this.executed.take() {
            trace0(b"# polling spawn-cpu; extracting executed\n");
            let result = match token.extract(&mut this.handle) {
                Ok((Some(value), None)) => Some(value),
                Ok((None, Some(token))) => {
                    this.executed = Some(token);
                    None
                }
                Ok(_) | Err(None) => return Poll::Ready(Err(None)),
                Err(err) => return Poll::Ready(Err(err)),
            };

            if let Some(result) = result {
                return if result < 0 {
                    Poll::Ready(Err(Some(result)))
                } else {
                    trace1(b"# polling spawn-cpu; stage=executed, res=%d\n", result);
                    match this.callable.take() {
                        None => Poll::Ready(Err(None)),
                        Some(callable) => match callable.result::<16, TFnOnce, TResult, TError>(this.handle.heap()) {
                            Ok(Some(value)) => Poll::Ready(Ok(value)),
                            Ok(None) | Err(_) => Poll::Ready(Err(None)),
                        },
                    }
                };
            }
        }

        if this.queued.is_some() || this.executed.is_some() {
            return Poll::Pending;
        }

        let callable = match this.task.take() {
            Some(target) => match CallableTarget::allocate(this.handle.heap(), target) {
                Ok(callable) => callable,
                Err(_) => return Poll::Ready(Err(None)),
            },
            None => return Poll::Ready(Err(None)),
        };

        let poll = match this.handle.schedule(&callable) {
            Ok((queued, executed)) => {
                trace2(b"callable; scheduled, qid=%d, eid=%d\n", queued.cid(), executed.cid());
                this.queued = Some(queued);
                this.executed = Some(executed);
                Poll::Pending
            }
            Err(_) => {
                trace0(b"callable not scheduled\n");
                Poll::Ready(Err(None))
            }
        };

        this.callable = Some(callable);
        poll
    }
}

impl<'a, THandle, TFnOnce, TResult, TError> Drop for ExecuteFuture<'a, THandle, TFnOnce, TResult, TError>
where
    THandle: IORuntimeHandle + Unpin,
    TFnOnce: FnOnce() -> Result<TResult, TError> + Unpin + Send + 'a,
    TResult: Unpin + Send + 'a,
    TError: Unpin + Send + 'a,
{
    fn drop(&mut self) {
        if let Some(callable) = self.callable.take() {
            let (ptr, len) = callable.as_ref().as_ptr();
            trace2(b"callable; releasing task, heap=%x, size=%d\n", ptr, len);

            if let Err(_) = callable.release(self.handle.heap()) {
                trace2(b"callable; releasing task, heap=%x, size=%d, failed\n", ptr, len);
            }
        }
    }
}
