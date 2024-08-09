use super::errno::*;
use adma_io::heap::*;
use adma_io::proc::*;
use adma_io::runtime::*;

pub struct CatCommand {
    pub args: &'static ProcessArguments,
}

impl CatCommand {
    pub async fn execute(self, mut ops: IORuntimeOps) -> Option<&'static [u8]> {
        let buffer = match Heap::allocate(32 * 4096) {
            Err(_) => return Some(APP_MEMORY_ALLOC_FAILED),
            Ok(value) => value.droplet(),
        };

        let stdout = ops.open_stdout();
        let path = match self.args.get(2) {
            None => return Some(APP_ARGS_FAILED),
            Some(value) => value,
        };

        let file = match ops.open_file(&path).await {
            Ok(value) => value,
            Err(Some(_)) => return Some(APP_FILE_OPENING_FAILED),
            Err(None) => return Some(APP_INTERNALLY_FAILED),
        };

        let mut offset = 0;

        loop {
            let read = match ops.read_file(&file, &buffer, offset).await {
                Ok(cnt) => cnt,
                Err(None) => return Some(APP_INTERNALLY_FAILED),
                Err(Some(_)) => return Some(APP_FILE_READING_FAILED),
            };

            offset += read as u64;

            if read == 0 {
                break;
            }

            let mut remaining = read as usize;

            while remaining > 0 {
                let offset = read as usize - remaining;
                let slice = match buffer.between(offset, remaining) {
                    Ok(value) => value,
                    Err(()) => return Some(APP_MEMORY_SLICE_FAILED),
                };

                let written = match ops.write_stdout(&stdout, &slice).await {
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

        match ops.close_file(file).await {
            Ok(()) => None,
            Err(None) => Some(APP_INTERNALLY_FAILED),
            Err(Some(_)) => Some(APP_FILE_CLOSING_FAILED),
        }
    }
}
