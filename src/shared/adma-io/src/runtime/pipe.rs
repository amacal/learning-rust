use super::file::*;
use super::ops::*;
use crate::kernel::*;
use crate::syscall::*;

pub enum CreatePipe {
    Succeeded((ReadPipeDescriptor, WritePipeDescriptor)),
    Failed(isize),
}

impl IORuntimeOps {
    pub fn create_pipe(&self) -> CreatePipe {
        let mut pipefd = [0; 2];
        let ptr = pipefd.as_mut_ptr();

        let flags = O_DIRECT;
        let result = sys_pipe2(ptr, flags);

        if result == 0 {
            CreatePipe::Succeeded((ReadPipeDescriptor { value: pipefd[0] }, WritePipeDescriptor { value: pipefd[1] }))
        } else {
            CreatePipe::Failed(result)
        }
    }
}

pub struct ReadPipeDescriptor {
    value: u32,
}

impl AsFileDescriptor for ReadPipeDescriptor {
    fn as_fd(self) -> u32 {
        self.value
    }
}

impl AsFileDescriptor for &ReadPipeDescriptor {
    fn as_fd(self) -> u32 {
        self.value
    }
}

impl AsClosableFileDescriptor for ReadPipeDescriptor {}
impl AsReadableFileDescriptor for &ReadPipeDescriptor {}

pub struct WritePipeDescriptor {
    value: u32,
}

impl WritePipeDescriptor {
    pub fn at(fd: u32) -> Self {
        Self { value: fd }
    }
}

impl AsFileDescriptor for WritePipeDescriptor {
    fn as_fd(self) -> u32 {
        self.value
    }
}

impl AsFileDescriptor for &WritePipeDescriptor {
    fn as_fd(self) -> u32 {
        self.value
    }
}

impl AsClosableFileDescriptor for WritePipeDescriptor {}
impl AsWrittableFileDescriptor for &WritePipeDescriptor {}
