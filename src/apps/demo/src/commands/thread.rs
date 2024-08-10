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
            let task = ops.spawn(move |mut ops| async move {
                for i in 0..self.cpus {
                    match ops.execute(move || -> Result<u32, ()> { Ok(i + j) }).await {
                        Err(_) | Ok(Err(_)) => return Some(APP_CPU_SPAWNING_FAILED),
                        Ok(Ok(value)) => trace3(b"completed %d %d %d\n", i, j, value),
                    }
                }

                None
            });

            match task.await {
                Ok(()) => (),
                Err(_) => return Some(APP_INTERNALLY_FAILED),
            }
        }

        None
    }
}
