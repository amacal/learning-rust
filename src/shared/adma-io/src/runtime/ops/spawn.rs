use super::*;

impl IORuntimeOps {
    pub fn spawn<'a, TFnOnce, TFuture>(
        &self,
        call: TFnOnce,
    ) -> Result<(), Option<i32>>
    where
        TFuture: Future<Output = Option<&'static [u8]>> + Send + 'a,
        TFnOnce: FnOnce(IORuntimeOps) -> TFuture + Unpin + Send + 'a,
    {
        match self.handle().spawn(call) {
            Some((Some(_), _)) => Ok(()),
            _ => Err(None),
        }
    }
}
