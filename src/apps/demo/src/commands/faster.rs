use super::errno::*;
use adma_io::heap::*;
use adma_io::proc::*;
use adma_io::runtime::*;

pub struct FasterCommand {
    pub args: &'static ProcessArguments,
    pub delay: u32,
}

impl FasterCommand {
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
        let mut timeout = timeout(self.delay);

        loop {
            let read = read_file(&file, buffer, offset);
            let (result, returned) = match select(timeout, read).await {
                SelectResult::Failed() => return Some(APP_SELECT_FAILED),
                SelectResult::Result2(result, timeout) => (result, timeout),
                SelectResult::Result1(result, _) => match result {
                    TimeoutResult::Succeeded() => break,
                    TimeoutResult::OperationFailed(_) => return Some(APP_DELAY_FAILED),
                    TimeoutResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
                },
            };

            let (buf, read) = match result {
                FileReadResult::Succeeded(buffer, read) => (buffer, read),
                FileReadResult::OperationFailed(_, _) => return Some(APP_FILE_READING_FAILED),
                FileReadResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
            };

            timeout = returned;
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
                    StdOutWriteResult::OperationFailed(_, _) => return Some(APP_STDOUT_FAILED),
                    StdOutWriteResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
                };

                if written <= 0 {
                    return Some(APP_FILE_WRITING_FAILED);
                }

                remaining -= written;
            }
        }

        None
    }
}
