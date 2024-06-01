use super::errno::*;
use crate::runtime::*;

pub struct SpawnCommand {
    pub times: u32,
    pub delay: u32,
}

impl SpawnCommand {
    pub async fn execute(self) -> Option<&'static [u8]> {
        let stdout = open_stdout();

        for i in 0..self.times {
            match timeout(self.delay).await {
                TimeoutResult::Succeeded() => (),
                TimeoutResult::OperationFailed(_) => return Some(APP_DELAY_FAILED),
                TimeoutResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
            };

                let spawned = spawn(async move {
                for _ in 0..i + 1 {
                    let msg: Option<&'static [u8]> = match timeout(5).await {
                        TimeoutResult::Succeeded() => continue,
                        TimeoutResult::OperationFailed(_) => Some(APP_DELAY_FAILED),
                        TimeoutResult::InternallyFailed() => Some(APP_INTERNALLY_FAILED),
                    };

                    if let Some(msg) = msg {
                        return Some(msg);
                    }
                }

                None
            });

            match spawned.await {
                SpawnResult::Succeeded() => (),
                SpawnResult::OperationFailed() => return Some(APP_INTERNALLY_FAILED),
                SpawnResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
            }
        }

        match write_stdout(&stdout, b"Spawning completed.\n").await {
            StdOutWriteResult::Succeeded(_, _) => (),
            StdOutWriteResult::OperationFailed(_, _) => return Some(APP_STDOUT_FAILED),
            StdOutWriteResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
        }

        None
    }
}
