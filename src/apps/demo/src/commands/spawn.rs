use super::errno::*;
use adma_io::runtime::*;

pub struct SpawnCommand {
    pub times: u32,
    pub delay: u32,
}

impl SpawnCommand {
    pub async fn execute(self, mut ops: IORuntimeOps) -> Option<&'static [u8]> {
        let stdout = ops.open_stdout();

        for i in 0..self.times {
            match ops.timeout(self.delay, 0).await {
                TimeoutResult::Succeeded() => (),
                TimeoutResult::OperationFailed(_) => return Some(APP_DELAY_FAILED),
                TimeoutResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
            };

            let spawned = ops.spawn_io(move |mut ops| async move {
                for _ in 0..i + 1 {
                    let msg: Option<&'static [u8]> = match ops.timeout(5, 0).await {
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

            match spawned {
                None => return Some(APP_INTERNALLY_FAILED),
                Some(spawned) => match spawned.await {
                    Err(()) => return Some(APP_INTERNALLY_FAILED),
                    Ok(()) => (),
                },
            }
        }

        match ops.write_stdout(&stdout, b"Spawning completed.\n").await {
            StdOutWriteResult::Succeeded(_, _) => (),
            StdOutWriteResult::OperationFailed(_, _) => return Some(APP_STDOUT_FAILED),
            StdOutWriteResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
        }

        None
    }
}
