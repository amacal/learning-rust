use super::errno::*;
use adma_io::runtime::*;

pub struct SpawnCommand {
    pub times: u32,
    pub delay: u32,
}

impl SpawnCommand {
    pub async fn execute(self, ops: IORuntimeOps) -> Option<&'static [u8]> {
        let stdout = ops.stdout();

        for i in 0..self.times {
            match ops.timeout(self.delay, 0).await {
                Ok(()) => (),
                Err(Some(_)) => return Some(APP_DELAY_FAILED),
                Err(None) => return Some(APP_INTERNALLY_FAILED),
            };

            let spawned = ops.spawn(move |ops| async move {
                for _ in 0..i + 1 {
                    let msg: Option<&'static [u8]> = match ops.timeout(5, 0).await {
                        Ok(()) => continue,
                        Err(Some(_)) => Some(APP_DELAY_FAILED),
                        Err(None) => Some(APP_INTERNALLY_FAILED),
                    };

                    if let Some(msg) = msg {
                        return Some(msg);
                    }
                }

                None
            });

            match spawned.await {
                Ok(()) => (),
                Err(_) => return Some(APP_INTERNALLY_FAILED),
            }
        }

        match ops.write(stdout, b"Spawning completed.\n").await {
            Ok(_) => (),
            Err(Some(_)) => return Some(APP_STDOUT_FAILED),
            Err(None) => return Some(APP_INTERNALLY_FAILED),
        }

        None
    }
}
