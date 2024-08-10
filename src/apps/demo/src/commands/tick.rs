use super::errno::*;
use adma_io::runtime::*;

pub struct TickCommand {
    pub ticks: u32,
    pub delay: u32,
}

impl TickCommand {
    pub async fn execute(self, mut ops: IORuntimeOps) -> Option<&'static [u8]> {
        let stdout = ops.stdout();

        for _ in 0..self.ticks {
            match ops.timeout(self.delay, 0).await {
                TimeoutResult::Succeeded() => (),
                TimeoutResult::OperationFailed(_) => return Some(APP_DELAY_FAILED),
                TimeoutResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
            };

            match ops.write(&stdout, b".").await {
                Ok(_) => (),
                Err(Some(_)) => return Some(APP_STDOUT_FAILED),
                Err(None) => return Some(APP_INTERNALLY_FAILED),
            }
        }

        match ops.write(&stdout, b"\n").await {
            Ok(_) => (),
            Err(Some(_)) => return Some(APP_STDOUT_FAILED),
            Err(None) => return Some(APP_INTERNALLY_FAILED),
        }

        None
    }
}
