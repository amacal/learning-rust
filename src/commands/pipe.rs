use crate::heap::*;
use crate::runtime::*;
use super::errno::*;

pub struct PipeCommand {
    pub msg: &'static [u8],
}

impl PipeCommand {
    pub async fn execute(self) -> Option<&'static [u8]> {
        let stdout = open_stdout();
        let (reader, writer) = match create_pipe() {
            CreatePipe::Succeeded((reader, writer)) => (reader, writer),
            CreatePipe::Failed(_) => return Some(APP_PIPE_CREATING_FAILED),
        };

        match timeout(3).await {
            TimeoutResult::Succeeded() => (),
            TimeoutResult::OperationFailed(_) => return Some(APP_DELAY_FAILED),
            TimeoutResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
        };

        let spawned = spawn(async move {
            let buffer = match mem_alloc(1 * 4096) {
                MemoryAllocation::Failed(_) => return Some(APP_MEMORY_ALLOC_FAILED),
                MemoryAllocation::Succeeded(value) => value.droplet(),
            };

            let (buffer, cnt) = match read_pipe(&reader, buffer).await {
                PipeReadResult::Succeeded(buffer, cnt) => (buffer, cnt),
                PipeReadResult::OperationFailed(_, _) => return Some(APP_PIPE_READING_FAILED),
                PipeReadResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
            };

            let slice = match buffer.between(0, cnt as usize) {
                HeapSlicing::Succeeded(value) => value,
                _ => return Some(APP_MEMORY_SLICE_FAILED),
            };

            match write_stdout(&stdout, &slice).await {
                StdOutWriteResult::Succeeded(_, _) => (),
                StdOutWriteResult::OperationFailed(_, _) => return Some(APP_STDOUT_FAILED),
                StdOutWriteResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
            }

            match close_pipe(reader).await {
                PipeCloseResult::Succeeded() => None,
                PipeCloseResult::OperationFailed(_) => Some(APP_PIPE_WRITING_FAILED),
                PipeCloseResult::InternallyFailed() => Some(APP_INTERNALLY_FAILED),
            }
        });

        match spawned.await {
            SpawnResult::Succeeded() => (),
            SpawnResult::OperationFailed() => return Some(APP_INTERNALLY_FAILED),
            SpawnResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
        }

        let spawned = spawn(async move {
            match timeout(3).await {
                TimeoutResult::Succeeded() => (),
                TimeoutResult::OperationFailed(_) => return Some(APP_DELAY_FAILED),
                TimeoutResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
            };

            match write_pipe(&writer, self.msg).await {
                PipeWriteResult::Succeeded(_, _) => (),
                PipeWriteResult::OperationFailed(_, _) => return Some(APP_PIPE_WRITING_FAILED),
                PipeWriteResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
            }

            match close_pipe(writer).await {
                PipeCloseResult::Succeeded() => None,
                PipeCloseResult::OperationFailed(_) => Some(APP_PIPE_WRITING_FAILED),
                PipeCloseResult::InternallyFailed() => Some(APP_INTERNALLY_FAILED),
            }
        });

        match spawned.await {
            SpawnResult::Succeeded() => (),
            SpawnResult::OperationFailed() => return Some(APP_INTERNALLY_FAILED),
            SpawnResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
        }

        None
    }
}
