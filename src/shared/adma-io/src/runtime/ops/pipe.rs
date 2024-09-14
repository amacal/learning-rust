use super::*;
use crate::kernel::*;
use crate::syscall::*;

impl IORuntimeOps {
    pub fn pipe(
        &self,
    ) -> Result<
        (
            impl FileDescriptor + Readable + Duplicable + Closable + Copy,
            impl FileDescriptor + Writtable + Duplicable + Closable + Copy,
        ),
        Option<i32>,
    > {
        let mut pipefd = [0; 2];
        let ptr = pipefd.as_mut_ptr();

        let flags = O_DIRECT | O_NONBLOCK;
        let result = sys_pipe2(ptr, flags);

        match i32::try_from(result) {
            Ok(val) if val < 0 => return Err(Some(val)),
            Ok(val) if val > 0 => return Err(None),
            Err(_) => return Err(None),
            Ok(_) => (),
        }

        Ok((ReadPipeDescriptor { fd: pipefd[0] }, WritePipeDescriptor { fd: pipefd[1] }))
    }

    pub fn clone<TDescriptor>(&self, descriptor: TDescriptor) -> Result<TDescriptor, Option<i32>>
    where
        TDescriptor: FileDescriptor + Duplicable,
    {
        match sys_dup(descriptor.as_fd()) {
            value if value < 0 => match i32::try_from(value) {
                Ok(value) => Err(Some(value)),
                Err(_) => Err(None),
            },
            value => match u32::try_from(value) {
                Ok(value) => Ok(TDescriptor::from(value)),
                Err(_) => Err(None),
            },
        }
    }
}

#[derive(Clone, Copy)]
struct ReadPipeDescriptor {
    fd: u32,
}

impl FileDescriptor for ReadPipeDescriptor {
    fn as_fd(self) -> u32 {
        self.fd
    }
}

impl Closable for ReadPipeDescriptor {}
impl Readable for ReadPipeDescriptor {}

impl Duplicable for ReadPipeDescriptor {
    fn from(fd: u32) -> Self {
        ReadPipeDescriptor { fd: fd }
    }
}

#[derive(Clone, Copy)]
struct WritePipeDescriptor {
    fd: u32,
}

impl FileDescriptor for WritePipeDescriptor {
    fn as_fd(self) -> u32 {
        self.fd
    }
}

impl Closable for WritePipeDescriptor {}
impl Writtable for WritePipeDescriptor {}

impl Duplicable for WritePipeDescriptor {
    fn from(fd: u32) -> Self {
        WritePipeDescriptor { fd: fd }
    }
}
