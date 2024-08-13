use super::errno::*;
use adma_io::heap::*;
use adma_io::proc::*;
use adma_io::runtime::*;

pub struct FasterCommand {
    pub args: &'static ProcessArguments,
    pub delay: u32,
}

impl FasterCommand {
    pub async fn execute(self, ops: IORuntimeOps) -> Option<&'static [u8]> {
        let buffer = match Heap::allocate(32 * 4096) {
            Err(_) => return Some(APP_MEMORY_ALLOC_FAILED),
            Ok(heap) => heap.droplet(),
        };

        let stdout = ops.stdout();
        let path = match self.args.get(2) {
            None => return Some(APP_ARGS_FAILED),
            Some(value) => value,
        };

        let file = match ops.open_at(&path).await {
            Ok(value) => value,
            Err(Some(_)) => return Some(APP_FILE_OPENING_FAILED),
            Err(None) => return Some(APP_INTERNALLY_FAILED),
        };

        let mut offset = 0;
        let mut timeout = ops.timeout(self.delay, 0);

        loop {
            let read = ops.read_at_offset(file, &buffer, offset);
            let (result, returned) = match select(timeout, read).await {
                SelectResult::Failed() => return Some(APP_SELECT_FAILED),
                SelectResult::Result2(result, timeout) => (result, timeout),
                SelectResult::Result1(result, _) => match result {
                    Ok(()) => break,
                    Err(Some(_)) => return Some(APP_DELAY_FAILED),
                    Err(None) => return Some(APP_INTERNALLY_FAILED),
                },
            };

            let read = match result {
                Ok(cnt) => cnt,
                Err(None) => return Some(APP_INTERNALLY_FAILED),
                Err(Some(_)) => return Some(APP_FILE_READING_FAILED),
            };

            timeout = returned;
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

                let written = match ops.write(stdout, &slice).await {
                    Ok(cnt) => cnt as usize,
                    Err(Some(_)) => return Some(APP_STDOUT_FAILED),
                    Err(None) => return Some(APP_INTERNALLY_FAILED),
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
