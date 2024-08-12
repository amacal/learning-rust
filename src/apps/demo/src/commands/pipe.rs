use super::errno::*;
use adma_io::heap::*;
use adma_io::runtime::*;

pub struct PipeCommand {
    pub msg: &'static [u8],
}

impl PipeCommand {
    pub async fn execute(self, mut ops: IORuntimeOps) -> Option<&'static [u8]> {
        let stdout = ops.stdout();
        let (reader, writer) = match ops.pipe() {
            Ok((reader, writer)) => (reader, writer),
            Err(_) => return Some(APP_PIPE_CREATING_FAILED),
        };

        let reader = ops.spawn(move |mut ops| async move {
            let buffer = match Heap::allocate(1 * 4096) {
                Err(_) => return Some(APP_MEMORY_ALLOC_FAILED),
                Ok(value) => value.droplet(),
            };

            let cnt = match ops.read(reader, &buffer).await {
                Ok(cnt) => cnt,
                Err(_) => return Some(APP_PIPE_READING_FAILED),
            };

            let slice = match buffer.between(0, cnt as usize) {
                Ok(value) => value,
                Err(()) => return Some(APP_MEMORY_SLICE_FAILED),
            };

            let written = match ops.write(stdout, &slice).await {
                Ok(cnt) => cnt,
                Err(_) => return Some(APP_STDOUT_FAILED),
            };

            if written as usize != self.msg.len() {
                return Some(APP_STDOUT_INCOMPLETE);
            }

            match ops.close(reader).await {
                Ok(()) => None,
                Err(_) => Some(APP_PIPE_CLOSING_FAILED),
            }
        });

        let writer = ops.spawn(move |mut ops| async move {
            match ops.write(writer, &self.msg).await {
                Ok(_) => (),
                Err(_) => return Some(APP_PIPE_WRITING_FAILED),
            }

            match ops.close(writer).await {
                Ok(()) => None,
                Err(_) => Some(APP_PIPE_CLOSING_FAILED),
            }
        });

        match reader.await {
            Ok(()) => (),
            Err(_) => return Some(APP_IO_SPAWNING_FAILED),
        }

        match writer.await {
            Ok(()) => (),
            Err(_) => return Some(APP_IO_SPAWNING_FAILED),
        }

        None
    }
}
