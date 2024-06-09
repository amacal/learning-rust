use super::errno::*;
use crate::core::*;
use crate::heap::*;
use crate::proc::*;
use crate::runtime::*;
use crate::sha1::*;
use crate::trace::*;

pub struct Sha1Command {
    pub args: &'static ProcessArguments,
}

impl Sha1Command {
    pub async fn execute(self) -> Option<&'static [u8]> {
        for arg in 2..self.args.len() {
            // a task will be spawned for each argument
            let task = spawn(async move {
                // an auto dropped memory for a buffer
                let buffer: Droplet<Heap> = match mem_alloc(32 * 4096) {
                    MemoryAllocation::Succeeded(value) => value.droplet(),
                    MemoryAllocation::Failed(_) => return Some(APP_MEMORY_ALLOC_FAILED),
                };

                // a path of the file to hash
                let path: ProcessArgument = match self.args.get(arg) {
                    None => return Some(APP_ARGS_FAILED),
                    Some(value) => value,
                };

                // a file descriptor for a file we opened
                let file: FileDescriptor = match open_file(&path).await {
                    FileOpenResult::Succeeded(value) => value,
                    _ => return Some(APP_FILE_OPENING_FAILED),
                };

                let mut file_offset = 0;
                let mut buffer_offset = 0;
                let mut sha1 = Sha1::new();

                loop {
                    while buffer_offset < buffer.len {
                        // slice a buffer to try it fill till the end
                        let buffer: HeapSlice = match buffer.between(buffer_offset, buffer.len) {
                            HeapSlicing::Succeeded(value) => value,
                            _ => return Some(APP_MEMORY_SLICE_FAILED),
                        };

                        // and read bytes into sliced memory from a given file offset
                        let read = match read_file(&file, buffer, file_offset).await {
                            FileReadResult::Succeeded(_, read) => read as usize,
                            _ => return Some(APP_FILE_READING_FAILED),
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
                        HeapSlicing::Succeeded(val) => val,
                        _ => return Some(APP_MEMORY_SLICE_FAILED),
                    };

                    // to process it outside event loop
                    let task = spawn_cpu(move || -> Result<Sha1, ()> {
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
                    if buffer_offset < buffer.len {
                        break;
                    }

                    // otherwise start filling buffer from the beginning
                    buffer_offset = 0;
                }

                // the buffer may have remainder between 0 and 63 bytes
                let slice: HeapSlice = match buffer.between(buffer_offset / 64 * 64, buffer_offset) {
                    HeapSlicing::Succeeded(slice) => slice,
                    _ => return Some(APP_MEMORY_SLICE_FAILED),
                };

                // which needs to be finalized
                let task = move || -> Result<[u32; 5], ()> {
                    // returning final hash as [u32; 5]
                    Ok(sha1.finalize(slice.ptr() as *mut u8, slice.len(), file_offset))
                };

                // a cpu task has to be awaited
                let hash: [u32; 5] = match spawn_cpu(task) {
                    None => return Some(APP_CPU_SPAWNING_FAILED),
                    Some(task) => match task.await {
                        SpawnCPUResult::Succeeded(Some(Ok(hash))) => hash,
                        _ => return Some(APP_CPU_SPAWNING_FAILED),
                    },
                };

                // a message like sha1sum output is constructed
                let mut msg = [0; 160];
                let len = format6(
                    &mut msg,
                    b"%x%x%x%x%x  %s\n",
                    hash[0],
                    hash[1],
                    hash[2],
                    hash[3],
                    hash[4],
                    path.as_ptr(),
                );

                // to be printed asynchronously in the stdout
                let stdout = open_stdout();
                match write_stdout(&stdout, (msg, len)).await {
                    StdOutWriteResult::Succeeded(_, _) => (),
                    _ => return Some(APP_STDOUT_FAILED),
                }

                // and finally we close a file
                match close_file(file).await {
                    FileCloseResult::Succeeded() => (),
                    _ => return Some(APP_FILE_CLOSING_FAILED),
                }

                None
            });

            // and task has to be awaited to be executed
            match task.await {
                SpawnResult::Succeeded() => (),
                _ => return Some(APP_IO_SPAWNING_FAILED),
            }
        }

        None
    }
}
