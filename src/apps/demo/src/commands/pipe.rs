use super::errno::*;
use adma_io::heap::*;
use adma_io::runtime::*;

pub struct PipeCommand {
    pub msg: &'static [u8],
}

impl PipeCommand {
    pub async fn execute(self, mut ops: IORuntimeOps) -> Option<&'static [u8]> {
        let stdout = ops.open_stdout();
        let (reader, writer) = match ops.create_pipe() {
            CreatePipe::Succeeded((reader, writer)) => (reader, writer),
            _ => return Some(APP_PIPE_CREATING_FAILED),
        };

        let reader = ops.spawn_io(move |mut ops| async move {
            let buffer = match Heap::allocate(1 * 4096) {
                Err(_) => return Some(APP_MEMORY_ALLOC_FAILED),
                Ok(value) => value.droplet(),
            };

            let (buffer, cnt) = match ops.read_pipe(&reader, buffer).await {
                PipeReadResult::Succeeded(buffer, cnt) => (buffer, cnt),
                _ => return Some(APP_PIPE_READING_FAILED),
            };

            let slice = match buffer.between(0, cnt as usize) {
                Ok(value) => value,
                Err(()) => return Some(APP_MEMORY_SLICE_FAILED),
            };

            let written = match ops.write_stdout(&stdout, &slice).await {
                StdOutWriteResult::Succeeded(_, written) => written,
                _ => return Some(APP_STDOUT_FAILED),
            };

            if written as usize != self.msg.len() {
                return Some(APP_STDOUT_INCOMPLETE);
            }

            match ops.close_pipe(reader).await {
                PipeCloseResult::Succeeded() => None,
                _ => Some(APP_PIPE_CLOSING_FAILED),
            }
        });

        let writer = ops.spawn_io(move |mut ops| async move {
            match ops.write_pipe(&writer, self.msg).await {
                PipeWriteResult::Succeeded(_, _) => (),
                _ => return Some(APP_PIPE_WRITING_FAILED),
            }

            match ops.close_pipe(writer).await {
                PipeCloseResult::Succeeded() => None,
                _ => Some(APP_PIPE_CLOSING_FAILED),
            }
        });

        match reader.await {
            SpawnResult::Succeeded() => (),
            _ => return Some(APP_IO_SPAWNING_FAILED),
        }

        match writer.await {
            SpawnResult::Succeeded() => (),
            _ => return Some(APP_IO_SPAWNING_FAILED),
        }

        None
    }
}
