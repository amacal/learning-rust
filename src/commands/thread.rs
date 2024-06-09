use super::errno::*;
use crate::runtime::*;
use crate::trace::*;

pub struct ThreadCommand {
    pub ios: u32,
    pub cpus: u32,
}

impl ThreadCommand {
    pub async fn execute(self) -> Option<&'static [u8]> {
        for j in 0..self.ios {
            let task = spawn(async move {
                for i in 0..self.cpus {
                    let value = match spawn_cpu(move || -> Result<u32, ()> { Ok(i + j) }) {
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

            match task.await {
                SpawnResult::Succeeded() => (),
                SpawnResult::OperationFailed() => return Some(APP_INTERNALLY_FAILED),
                SpawnResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
            }
        }

        None
    }
}
