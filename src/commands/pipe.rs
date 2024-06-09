use super::errno::*;
use crate::heap::*;
use crate::runtime::*;

pub struct PipeCommand {
    pub msg: &'static [u8],
}

impl PipeCommand {
    pub async fn execute(self) -> Option<&'static [u8]> {
        let stdout = open_stdout();
        let (reader, writer) = match create_pipe() {
            CreatePipe::Succeeded((reader, writer)) => (reader, writer),
            _ => return Some(APP_PIPE_CREATING_FAILED),
        };

        let reader = spawn(async move {
            let buffer = match mem_alloc(1 * 4096) {
                MemoryAllocation::Failed(_) => return Some(APP_MEMORY_ALLOC_FAILED),
                MemoryAllocation::Succeeded(value) => value.droplet(),
            };

            let (buffer, cnt) = match read_pipe(&reader, buffer).await {
                PipeReadResult::Succeeded(buffer, cnt) => (buffer, cnt),
                _ => return Some(APP_PIPE_READING_FAILED),
            };

            let slice = match buffer.between(0, cnt as usize) {
                HeapSlicing::Succeeded(value) => value,
                _ => return Some(APP_MEMORY_SLICE_FAILED),
            };

            let written = match write_stdout(&stdout, &slice).await {
                StdOutWriteResult::Succeeded(_, written) => written,
                _ => return Some(APP_STDOUT_FAILED),
            };

            if written as usize != self.msg.len() {
                return Some(APP_STDOUT_INCOMPLETE);
            }

            match close_pipe(reader).await {
                PipeCloseResult::Succeeded() => None,
                _ => Some(APP_PIPE_CLOSING_FAILED),
            }
        });

        let writer = spawn(async move {
            match write_pipe(&writer, self.msg).await {
                PipeWriteResult::Succeeded(_, _) => (),
                _ => return Some(APP_PIPE_WRITING_FAILED),
            }

            match close_pipe(writer).await {
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
