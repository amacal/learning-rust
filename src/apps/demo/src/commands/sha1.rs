use super::errno::*;
use adma_io::core::*;
use adma_io::heap::*;
use adma_io::proc::*;
use adma_io::runtime::*;
use adma_io::sha1::*;
use adma_io::trace::*;

pub struct Sha1Command {
    pub args: &'static ProcessArguments,
}

impl Sha1Command {
    pub async fn execute(self, mut ops: IORuntimeOps) -> Option<&'static [u8]> {
        for arg in 2..self.args.len() {
            // a task will be spawned for each argument
            let task = ops.spawn_io(move |mut ops| async move {
                // an auto dropped memory for a buffer
                let buffer: Droplet<Heap> = match Heap::allocate(32 * 4096) {
                    Ok(value) => value.droplet(),
                    Err(_) => return Some(APP_MEMORY_ALLOC_FAILED),
                };

                // a path of the file to hash
                let path: ProcessArgument = match self.args.get(arg) {
                    None => return Some(APP_ARGS_FAILED),
                    Some(value) => value,
                };

                // a file descriptor for a file we opened
                let file: FileDescriptor = match ops.open_at(&path).await {
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
                        let read = match ops.read_file(&file, &buffer, file_offset).await {
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
                    let task = ops.spawn_cpu(move || -> Result<Sha1, ()> {
                        // just processing a slice and returning new self
                        Ok(sha1.update(slice.ptr() as *const u8, slice.len()))
                    });

                    // the cpu task has to be awaited
                    sha1 = match task {
                        None => return Some(APP_CPU_SPAWNING_FAILED),
                        Some(task) => match task.await {
                            SpawnCPUResult::Succeeded(Some(Ok(sha1))) => sha1,
                            _ => return Some(APP_CPU_SPAWNING_FAILED),
                        },
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
                let hash: [u32; 5] = match ops.spawn_cpu(task) {
                    None => return Some(APP_CPU_SPAWNING_FAILED),
                    Some(task) => match task.await {
                        SpawnCPUResult::Succeeded(Some(Ok(hash))) => hash,
                        _ => return Some(APP_CPU_SPAWNING_FAILED),
                    },
                };

                // a message like sha1sum output is constructed
                let mut msg = [0; 160];
                let len =
                    format6(&mut msg, b"%x%x%x%x%x  %s\n", hash[0], hash[1], hash[2], hash[3], hash[4], path.as_ptr());

                // to be printed asynchronously in the stdout
                let stdout = ops.open_stdout();
                match ops.write_stdout(&stdout, (msg, len)).await {
                    StdOutWriteResult::Succeeded(_, _) => (),
                    _ => return Some(APP_STDOUT_FAILED),
                }

                // and finally we close a file
                match ops.close(file).await {
                    Err(_) => return Some(APP_FILE_CLOSING_FAILED),
                    Ok(()) => (),
                }

                None
            });

            // and task has to be awaited to be executed
            match task {
                None => return Some(APP_IO_SPAWNING_FAILED),
                Some(task) => match task.await {
                    Err(()) => return Some(APP_IO_SPAWNING_FAILED),
                    Ok(()) => (),
                },
            }
        }

        None
    }
}
