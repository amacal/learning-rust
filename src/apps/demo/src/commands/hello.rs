use super::errno::*;
use adma_io::runtime::*;

pub struct HelloCommand {
    pub msg: &'static [u8],
}

impl HelloCommand {
    pub async fn execute(self, mut ops: IORuntimeOps) -> Option<&'static [u8]> {
        let stdout = ops.stdout();
        let written = match ops.write(&stdout, &self.msg).await {
            Ok(cnt) => cnt,
            Err(Some(_)) => return Some(APP_STDOUT_FAILED),
            Err(None) => return Some(APP_INTERNALLY_FAILED),
        };

        if written as usize != self.msg.len() {
            return Some(APP_STDOUT_INCOMPLETE);
        }

        None
    }
}
