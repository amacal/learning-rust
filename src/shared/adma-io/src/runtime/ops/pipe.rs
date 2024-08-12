use super::*;
use crate::kernel::*;
use crate::syscall::*;

impl IORuntimeOps {
    pub fn pipe(&self) -> Result<(impl FileDescriptor + Readable + Closable + Copy, impl FileDescriptor + Writtable + Closable + Copy), Option<i32>> {
        let mut pipefd = [0; 2];
        let ptr = pipefd.as_mut_ptr();

        let flags = O_DIRECT;
        let result = sys_pipe2(ptr, flags);

        match i32::try_from(result) {
            Ok(val) if val < 0 => return Err(Some(val)),
            Ok(val) if val > 0 => return Err(None),
            Err(_) => return Err(None),
            Ok(_) => (),
        }

        Ok((ReadPipeDescriptor { fd: pipefd[0] }, WritePipeDescriptor { fd: pipefd[1] }))
    }
}

#[derive(Clone, Copy)]
struct ReadPipeDescriptor {
    fd: u32,
}

impl FileDescriptor for ReadPipeDescriptor {
    fn as_fd(&self) -> u32 {
        self.fd
    }
}

impl Closable for ReadPipeDescriptor {}
impl Readable for ReadPipeDescriptor {}

#[derive(Clone, Copy)]
struct WritePipeDescriptor {
    fd: u32,
}

impl FileDescriptor for WritePipeDescriptor {
    fn as_fd(&self) -> u32 {
        self.fd
    }
}

impl Closable for WritePipeDescriptor {}
impl Writtable for WritePipeDescriptor {}
