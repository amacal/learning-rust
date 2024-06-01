use super::errno::*;
use crate::heap::*;
use crate::proc::*;
use crate::runtime::*;
use crate::trace::*;

struct Sha1 {
    h0: u32,
    h1: u32,
    h2: u32,
    h3: u32,
    h4: u32,
}

impl Sha1 {
    fn new() -> Sha1 {
        Sha1 {
            h0: 0x67452301,
            h1: 0xefcdab89,
            h2: 0x98badcfe,
            h3: 0x10325476,
            h4: 0xc3d2e1f0,
        }
    }

    fn finalize(mut self, ptr: *mut u8, mut len: usize, total: u64) -> [u32; 5] {
        unsafe {
            if len < 55 {
                let total = (total * 8).to_be_bytes();

                *ptr.add(len) = 0x80;
                len += 1;

                while len < 56 {
                    *ptr.add(len) = 0x00;
                    len += 1;
                }

                for i in 0..total.len() {
                    *ptr.add(len) = total[i];
                    len += 1;
                }

                self.update(ptr, len);
            } else {
                *ptr.add(len) = 0x80;
                len += 1;

                while len < 64 {
                    *ptr.add(len) = 0x00;
                    len += 1;
                }

                self.update(ptr, len);
                len = 0;

                while len < 56 {
                    *ptr.add(len) = 0x00;
                    len += 1;
                }

                let total = (total * 8).to_be_bytes();

                for i in 0..total.len() {
                    *ptr.add(len) = total[i];
                    len += 1;
                }

                self.update(ptr, len);
            }
        }

        [self.h0, self.h1, self.h2, self.h3, self.h4]
    }

    fn update(&mut self, ptr: *const u8, len: usize) {
        fn rotate<const T: u32>(value: u32) -> u32 {
            (value << T) ^ (value >> (32 - T))
        }

        for i in 0..(len / 64) {
            let (mut a, mut b, mut c, mut d, mut e) = (self.h0, self.h1, self.h2, self.h3, self.h4);
            let mut w: [u32; 80] = [0; 80];

            unsafe {
                for j in 0..16 {
                    let b0 = *ptr.add(i * 64 + j * 4 + 0) as u32;
                    let b1 = *ptr.add(i * 64 + j * 4 + 1) as u32;
                    let b2 = *ptr.add(i * 64 + j * 4 + 2) as u32;
                    let b3 = *ptr.add(i * 64 + j * 4 + 3) as u32;

                    w[j] = (b0 << 24) + (b1 << 16) + (b2 << 8) + b3;
                }

                for j in 16..80 {
                    w[j] = rotate::<1>(w[j - 3] ^ w[j - 8] ^ w[j - 14] ^ w[j - 16])
                }

                for j in 0..80 {
                    let (f, k) = match j {
                        0..20 => ((b & c) | (!b & d), 0x5a827999),
                        20..40 => (b ^ c ^ d, 0x6ed9eba1),
                        40..60 => (((b & c) | (b & d) | (c & d)), 0x8f1bbcdc),
                        _ => (b ^ c ^ d, 0xca62c1d6),
                    };

                    let t = rotate::<5>(a) + f + e + k + w[j];
                    e = d;
                    d = c;
                    c = rotate::<30>(b);
                    b = a;
                    a = t;
                }

                self.h0 = self.h0.wrapping_add(a);
                self.h1 = self.h1.wrapping_add(b);
                self.h2 = self.h2.wrapping_add(c);
                self.h3 = self.h3.wrapping_add(d);
                self.h4 = self.h4.wrapping_add(e);
            }
        }
    }
}

pub struct Sha1Command {
    pub args: &'static ProcessArguments,
}

impl Sha1Command {
    pub async fn execute(self) -> Option<&'static [u8]> {
        for arg in 2..self.args.len() {
            let task = spawn(async move {
                let buffer = match mem_alloc(32 * 4096) {
                    MemoryAllocation::Failed(_) => return Some(APP_MEMORY_ALLOC_FAILED),
                    MemoryAllocation::Succeeded(value) => value.droplet(),
                };

                let stdout = open_stdout();
                let path = match self.args.get(arg) {
                    None => return Some(APP_ARGS_FAILED),
                    Some(value) => value,
                };

                let file = match open_file(path).await {
                    FileOpenResult::Succeeded(value) => value,
                    FileOpenResult::OperationFailed(_) => return Some(APP_FILE_OPENING_FAILED),
                    FileOpenResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
                };

                let mut file_offset = 0;
                let mut buffer_offset = 0;
                let mut sha1 = Sha1::new();

                loop {
                    while buffer_offset < buffer.len {
                        let slice = match buffer.between(buffer_offset, buffer.len) {
                            HeapSlicing::Succeeded(val) => val,
                            _ => return Some(APP_MEMORY_SLICE_FAILED),
                        };

                        let read = match read_file(&file, slice, file_offset).await {
                            FileReadResult::Succeeded(_, read) => read as usize,
                            FileReadResult::OperationFailed(_, _) => return Some(APP_FILE_READING_FAILED),
                            FileReadResult::InternallyFailed() => return Some(APP_INTERNALLY_FAILED),
                        };

                        buffer_offset += read;
                        file_offset += read as u64;

                        if read == 0 {
                            break;
                        }
                    }

                    let slice = match buffer.between(0, buffer_offset / 64 * 64) {
                        HeapSlicing::Succeeded(val) => val,
                        _ => return Some(APP_MEMORY_SLICE_FAILED),
                    };

                    let task = spawn_cpu(move || {
                        sha1.update(slice.ptr as *const u8, slice.len);
                        Ok(sha1)
                    });

                    sha1 = match task.await {
                        SpawnCPUResult::Succeeded(Some(sha1)) => sha1,
                        SpawnCPUResult::Succeeded(None) => todo!(),
                        SpawnCPUResult::OperationFailed() => todo!(),
                        SpawnCPUResult::InternallyFailed() => todo!(),
                    };

                    if buffer_offset < buffer.len {
                        break;
                    }

                    buffer_offset = 0;
                }

                let slice = match buffer.between(buffer_offset / 64 * 64, buffer_offset) {
                    HeapSlicing::Succeeded(slice) => slice,
                    _ => return Some(APP_MEMORY_SLICE_FAILED),
                };

                let task = spawn_cpu(move || Ok(sha1.finalize(slice.ptr as *mut u8, slice.len, file_offset)));

                let hash = match task.await {
                    SpawnCPUResult::Succeeded(Some(hash)) => hash,
                    SpawnCPUResult::Succeeded(None) => todo!(),
                    SpawnCPUResult::OperationFailed() => todo!(),
                    SpawnCPUResult::InternallyFailed() => todo!(),
                };

                drop(buffer);
                trace3(b"hash: %d %d %x\n", buffer_offset, file_offset, hash[0]);

                match close_file(file).await {
                    FileCloseResult::Succeeded() => None,
                    FileCloseResult::OperationFailed(_) => Some(APP_FILE_CLOSING_FAILED),
                    FileCloseResult::InternallyFailed() => Some(APP_INTERNALLY_FAILED),
                }
            });

            match task.await {
                SpawnResult::Succeeded() => None,
                SpawnResult::OperationFailed() => Some(APP_INTERNALLY_FAILED),
                SpawnResult::InternallyFailed() => Some(APP_INTERNALLY_FAILED),
            };
        }

        None
    }
}
