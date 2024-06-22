use super::errno::*;
use adma_io::heap::*;
use adma_io::proc::*;
use adma_io::runtime::*;

pub struct CatCommand {
    pub args: &'static ProcessArguments,
}

impl CatCommand {
    pub async fn execute(self) -> Option<&'static [u8]> {
        let mut buffer = match mem_alloc(32 * 4096) {
            MemoryAllocation::Failed(_) => return Some(APP_MEMORY_ALLOC_FAILED),
            MemoryAllocation::Succeeded(value) => value.droplet(),
        };

        let stdout = open_stdout();
        let path = match self.args.get(2) {
            None => return Some(APP_ARGS_FAILED),
            Some(value) => value,
        };

        let file = match open_file(&path).await {
            FileOpenResult::Succeeded(value) => value,
            FileOpenResult::OperationFailed(_) => return Some(APP_FILE_OPENING_FAILED),
            FileOpenResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
        };

        let mut offset = 0;

        loop {
            let (buf, read) = match read_file(&file, buffer, offset).await {
                FileReadResult::Succeeded(buffer, read) => (buffer, read),
                FileReadResult::OperationFailed(_, _) => return Some(APP_FILE_READING_FAILED),
                FileReadResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
            };

            offset += read as u64;
            buffer = buf;

            if read == 0 {
                break;
            }

            let mut remaining = read as usize;

            while remaining > 0 {
                let offset = read as usize - remaining;
                let slice = match buffer.between(offset, remaining) {
                    HeapSlicing::Succeeded(value) => value,
                    _ => return Some(APP_MEMORY_SLICE_FAILED),
                };

                let written = match write_stdout(&stdout, &slice).await {
                    StdOutWriteResult::Succeeded(_, written) => written as usize,
                    StdOutWriteResult::OperationFailed(_, _) => return Some(APP_FILE_WRITING_FAILED),
                    StdOutWriteResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
                };

                if written <= 0 {
                    return Some(APP_FILE_WRITING_FAILED);
                }

                remaining -= written;
            }
        }

        match close_file(file).await {
            FileCloseResult::Succeeded() => None,
            FileCloseResult::OperationFailed(_) => Some(APP_FILE_CLOSING_FAILED),
            FileCloseResult::InternallyFailed() => Some(APP_INTERNALLY_FAILED),
        }
    }
}
