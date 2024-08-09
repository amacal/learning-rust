use super::errno::*;
use adma_io::runtime::*;
use adma_io::trace::*;

pub struct ThreadCommand {
    pub ios: u32,
    pub cpus: u32,
}

impl ThreadCommand {
    pub async fn execute(self, mut ops: IORuntimeOps) -> Option<&'static [u8]> {
        for j in 0..self.ios {
            let task = ops.spawn_io(move |mut ops| async move {
                for i in 0..self.cpus {
                    let value = match ops.spawn_cpu(move || -> Result<u32, ()> { Ok(i + j) }) {
                        None => return Some(APP_CPU_SPAWNING_FAILED),
                        Some(task) => match task.await {
                            SpawnCPUResult::Succeeded(value) => value,
                            SpawnCPUResult::OperationFailed() => return Some(APP_INTERNALLY_FAILED),
                            SpawnCPUResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
                        },
                    };

                    if let Some(Ok(val)) = value {
                        trace3(b"completed %d %d %d\n", i, j, val);
                    }
                }

                None
            });

            match task {
                None => return Some(APP_INTERNALLY_FAILED),
                Some(task) => match task.await {
                    Err(()) => return Some(APP_INTERNALLY_FAILED),
                    Ok(()) => (),
                },
            }
        }

        None
    }
}
