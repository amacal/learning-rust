use super::errno::*;
use adma_io::runtime::*;

pub struct HelloCommand {
    pub msg: &'static [u8],
}

impl HelloCommand {
    pub async fn execute(self, mut ops: IORuntimeOps) -> Option<&'static [u8]> {
        let stdout = ops.open_stdout();
        let written = match ops.write_stdout(&stdout, self.msg).await {
            StdOutWriteResult::Succeeded(_, written) => written,
            StdOutWriteResult::OperationFailed(_, _) => return Some(APP_STDOUT_FAILED),
            StdOutWriteResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
        };

        if written as usize != self.msg.len() {
            return Some(APP_STDOUT_INCOMPLETE);
        }

        None
    }
}
