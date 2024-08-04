#![cfg_attr(not(feature = "std"), no_std)]

mod complete;
mod entry;
mod init;
mod kernel;
mod shutdown;
mod submit;
mod syscall;
mod trace;

use crate::kernel::*;
use crate::trace::*;

pub use crate::complete::IORingComplete;
pub use crate::complete::IORingCompleteEntry;
pub use crate::entry::IORingSubmitEntry;
pub use crate::kernel::timespec;
pub use crate::submit::IORingSubmit;

pub enum IORingError {
    InvalidDescriptor,
    SetupFailed,
    MappingFailed,
    ReleaseFailed,
}

pub struct IORing {
    fd: u32,
    pub rx: IORingCompleter,
    pub tx: IORingSubmitter,
}

pub struct IORingSubmitter {
    fd: u32,
    cnt_total: usize,
    cnt_queued: usize,
    sq_ptr: *mut (),
    sq_ptr_len: usize,
    sq_tail: *mut u32,
    sq_ring_mask: *mut u32,
    sq_array: *mut u32,
    sq_sqes: *mut io_uring_sqe,
    sq_sqes_len: usize,
}

pub struct IORingCompleter {
    fd: u32,
    cq_ptr: *mut (),
    cq_ptr_len: usize,
    cq_head: *mut u32,
    cq_tail: *mut u32,
    cq_ring_mask: *mut u32,
    cq_cqes: *mut io_uring_cqe,
}

impl IORing {
    pub fn fd(&self) -> u32 {
        self.fd
    }
}
