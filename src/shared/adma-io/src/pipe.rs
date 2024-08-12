use ::core::mem;

use crate::syscall::*;
use crate::kernel::*;

pub struct PipeChannel {
    incoming: u32,
    outgoing: u32,
}

impl PipeChannel {
    fn new(incoming: u32, outgoing: u32) -> Self {
        Self { incoming, outgoing }
    }

    pub fn create() -> Result<Self, Option<i32>> {
        let mut pipefd = [0; 2];
        let ptr = pipefd.as_mut_ptr();

        match sys_pipe2(ptr, O_DIRECT) {
            result if result < 0 => match i32::try_from(result) {
                Ok(value) => Err(Some(value)),
                Err(_) => Err(None),
            },
            _ => Ok(PipeChannel::new(pipefd[0], pipefd[1])),
        }
    }

    pub fn extract(self) -> (u32, u32) {
        let incoming = self.incoming;
        let outgoing = self.outgoing;

        mem::forget(self);
        (incoming, outgoing)
    }
}

impl Drop for PipeChannel {
    fn drop(&mut self) {
        if self.incoming > 0 {
            sys_close(self.incoming);
            self.incoming = 0;
        }

        if self.outgoing > 0 {
            sys_close(self.outgoing);
            self.outgoing = 0;
        }
    }
}
