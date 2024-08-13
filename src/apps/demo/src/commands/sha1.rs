use super::errno::*;
use crate::trace::*;

use adma_io::core::*;
use adma_io::heap::*;
use adma_io::proc::*;
use adma_io::runtime::*;
use adma_io::sha1::*;

pub struct Sha1Command {
    pub args: &'static ProcessArguments,
}

impl Sha1Command {
    async fn sha1sum(ops: &IORuntimeOps, path: ProcessArgument) -> Option<&'static [u8]> {
        // an auto dropped memory for a buffer
        let buffer: Droplet<Heap> = match Heap::allocate(32 * 4096) {
            Ok(value) => value.droplet(),
            Err(_) => return Some(APP_MEMORY_ALLOC_FAILED),
        };

        // a file descriptor for a file we opened
        let file = match ops.open_at(&path).await {
            Ok(value) => value,
            _ => return Some(APP_FILE_OPENING_FAILED),
        };

        let mut file_offset = 0;
        let mut buffer_offset = 0;
        let mut sha1 = Sha1::new();

        loop {
            while buffer_offset < buffer.as_ref().len() {
                // slice a buffer to try it fill till the end
                let buffer: HeapSlice = match buffer.between(buffer_offset, buffer.as_ref().len()) {
                    Err(()) => return Some(APP_MEMORY_SLICE_FAILED),
                    Ok(value) => value,
                };

                // and read bytes into sliced memory from a given file offset
                let read = match ops.read_at_offset(file, &buffer, file_offset).await {
                    Err(_) => return Some(APP_FILE_READING_FAILED),
                    Ok(cnt) => cnt as usize,
                };

                // both counters have to be incremented
                buffer_offset += read;
                file_offset += read as u64;

                // and in case of end of file we return what we managed to read
                if read == 0 {
                    break;
                }
            }

            // let's slice till 512-bits boundary, as sha1 requires
            let slice = match buffer.between(0, buffer_offset / 64 * 64) {
                Err(()) => return Some(APP_MEMORY_SLICE_FAILED),
                Ok(val) => val,
            };

            // to process it outside event loop
            let task = ops.execute(move || -> Result<Sha1, ()> {
                // just processing a slice and returning new self
                Ok(sha1.update(slice.ptr() as *const u8, slice.len()))
            });

            // the cpu task has to be awaited
            sha1 = match task.await {
                Ok(Ok(sha1)) => sha1,
                Ok(_) | Err(_) => return Some(APP_CPU_SPAWNING_FAILED),
            };

            // and in case we didn't full entire buffer
            // we may assume the file is completed
            if buffer_offset < buffer.as_ref().len() {
                break;
            }

            // otherwise start filling buffer from the beginning
            buffer_offset = 0;
        }

        // the buffer may have remainder between 0 and 63 bytes
        let slice: HeapSlice = match buffer.between(buffer_offset / 64 * 64, buffer_offset) {
            Err(()) => return Some(APP_MEMORY_SLICE_FAILED),
            Ok(slice) => slice,
        };

        // which needs to be finalized
        let task = move || -> Result<[u32; 5], ()> {
            // returning final hash as [u32; 5]
            Ok(sha1.finalize(slice.ptr() as *mut u8, slice.len(), file_offset))
        };

        // a cpu task has to be awaited
        let hash: [u32; 5] = match ops.execute(task).await {
            Ok(Ok(hash)) => hash,
            Ok(_) | Err(_) => return Some(APP_CPU_SPAWNING_FAILED),
        };

        // and finally we close a file
        match ops.close(file).await {
            Err(_) => return Some(APP_FILE_CLOSING_FAILED),
            Ok(()) => (),
        }

        // a message like sha1sum output is constructed
        let mut msg = [0; 160];
        let len = format6(&mut msg, b"%x%x%x%x%x  %s\n", hash[0], hash[1], hash[2], hash[3], hash[4], path.as_ptr());

        // to be printed asynchronously in the stdout
        let stdout = ops.stdout();
        match ops.write(stdout, &(msg, len)).await {
            Ok(_) => (),
            Err(_) => return Some(APP_STDOUT_FAILED),
        }

        None
    }

    pub async fn execute(self, ops: IORuntimeOps) -> Option<&'static [u8]> {
        let (rx, tx) = match ops.channel::<ProcessArgument>(4) {
            Ok((rx, tx)) => (rx, tx),
            Err(_) => return Some(APP_CHANNEL_CREATING_FAILED),
        };

        // a task will be spawned to queue all files
        let write = |ops| async move {
            let mut tx = tx.create(&ops);

            for arg in 2..self.args.len() {
                // a path of the file to hash
                let path: ProcessArgument = match self.args.get(arg) {
                    None => return Some(APP_ARGS_FAILED),
                    Some(value) => value,
                };

                if let Err(_) = tx.write(path).await {
                    return Some(APP_CHANNEL_WRITING_FAILED);
                }
            }

            if let Err(_) = tx.flush().await {
                return Some(APP_CHANNEL_FLUSHING_FAILED);
            }

            if let Err(_) = tx.close().await {
                return Some(APP_CHANNEL_CLOSING_FAILED);
            }

            None
        };

        if let Err(_) = ops.spawn(write).await {
            return Some(APP_IO_SPAWNING_FAILED);
        }

        // a task will be spawned to process n files concurrently
        let read = |ops| async move {
            let mut rx = rx.create(&ops);

            while let Some(item) = rx.read().await {
                let (mut receipt, data) = match item {
                    Ok((receipt, data)) => (receipt, data),
                    Err(_) => return Some(APP_CHANNEL_READING_FAILED),
                };

                // a task will be spawned to process each file separately
                let process = |ops| async move {
                    if let Some(msg) = Self::sha1sum(&ops, data).await {
                        return Some(msg);
                    }

                    if let Err(_) = receipt.complete(&ops).await {
                        return Some(APP_CHANNEL_COMPLETING_FAILED);
                    }

                    None
                };

                // and task has to be awaited to be executed
                if let Err(_) = ops.spawn(process).await {
                    break;
                }
            };

            None
        };

        if let Err(_) = ops.spawn(read).await {
            return Some(APP_IO_SPAWNING_FAILED);
        }

        None
    }
}
