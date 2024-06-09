use core::ptr::{null, null_mut, read_volatile, write_volatile};

use crate::kernel::*;
use crate::syscall::*;
use crate::trace::*;

pub struct IORing {
    fd: u32,
    sq_ptr: *mut (),
    sq_ptr_len: usize,
    sq_sqes: *mut io_uring_sqe,
    sq_sqes_len: usize,
    cq_ptr: *mut (),
    cq_ptr_len: usize,
}

pub struct IORingSubmitter {
    fd: u32,
    cnt: usize,
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
    const IORING_OFF_SQ_RING: usize = 0x00000000;
    const IORING_OFF_CQ_RING: usize = 0x08000000;
    const IORING_OFF_SQES: usize = 0x010000000;
    const IORING_ENTER_GETEVENTS: u32 = 0x00000001;

    const IORING_OP_NOP: u8 = 0;
    const IORING_OP_TIMEOUT: u8 = 11;
    const IORING_OP_OPENAT: u8 = 18;
    const IORING_OP_CLOSE: u8 = 19;
    const IORING_OP_READ: u8 = 22;
    const IORING_OP_WRITE: u8 = 23;

    const AT_FDCWD: i32 = -100;

    const PROT_READ: usize = 0x00000001;
    const PROT_WRITE: usize = 0x00000002;

    const MAP_SHARED: usize = 0x00000001;
    const MAP_POPULATE: usize = 0x00008000;
}

#[allow(dead_code)]
pub enum IORingInit {
    Succeeded(IORingSubmitter, IORingCompleter),
    InvalidDescriptor(isize),
    SetupFailed(isize),
    MappingFailed(&'static [u8], isize),
}

impl IORing {
    pub fn init(entries: u32) -> IORingInit {
        let mut params = io_uring_params::default();
        let fd: u32 = match sys_io_uring_setup(entries, &mut params as *mut io_uring_params) {
            value if value < 0 => return IORingInit::SetupFailed(value),
            value => match value.try_into() {
                Err(_) => return IORingInit::InvalidDescriptor(value),
                Ok(value) => value,
            },
        };

        fn map<T>(fd: u32, array: u32, count: u32, offset: usize) -> (isize, usize) {
            let array = array as usize;
            let size = core::mem::size_of::<T>() as usize;

            let addr = null_mut();
            let length = array + size * count as usize;

            let prot = IORing::PROT_READ | IORing::PROT_WRITE;
            let flags = IORing::MAP_SHARED | IORing::MAP_POPULATE;

            (sys_mmap(addr, length, prot, flags, fd as usize, offset), length)
        }

        let sq_array = params.sq_off.array;
        let sq_entries = params.sq_entries;

        let offset = IORing::IORING_OFF_SQ_RING;
        let (sq_ptr, sq_ptr_len) = match map::<u32>(fd, sq_array, sq_entries, offset) {
            (res, _) if res <= 0 => return IORingInit::MappingFailed(b"SQ_RING", res),
            (res, len) => (res as *mut (), len),
        };

        let sq_tail = (sq_ptr as usize + params.sq_off.tail as usize) as *mut u32;
        let sq_array = (sq_ptr as usize + params.sq_off.array as usize) as *mut u32;
        let sq_ring_mask = (sq_ptr as usize + params.sq_off.ring_mask as usize) as *mut u32;
        let sq_ring_entries = (sq_ptr as usize + params.sq_off.ring_entries as usize) as *mut u32;

        trace2(b"ring ready; fd=%d, sq=%d\n", fd, unsafe { *sq_ring_entries });

        let offset = IORing::IORING_OFF_SQES;
        let (sq_sqes, sq_sqes_len) = match map::<io_uring_sqe>(fd, 0, sq_entries, offset) {
            (res, _) if res <= 0 => return IORingInit::MappingFailed(b"SQ_SQES", res),
            (res, len) => (res as *mut io_uring_sqe, len),
        };
        let cq_array = params.cq_off.cqes;
        let cq_entries = params.cq_entries;

        let offset = IORing::IORING_OFF_CQ_RING;
        let (cq_ptr, cq_ptr_len) = match map::<io_uring_cqe>(fd, cq_array, cq_entries, offset) {
            (res, _) if res <= 0 => return IORingInit::MappingFailed(b"CQ_RING", res),
            (res, len) => (res as *mut (), len),
        };

        let cq_head = (cq_ptr as usize + params.cq_off.head as usize) as *mut u32;
        let cq_tail = (cq_ptr as usize + params.cq_off.tail as usize) as *mut u32;
        let cq_cqes = (sq_ptr as usize + params.cq_off.cqes as usize) as *mut io_uring_cqe;
        let cq_ring_mask = (cq_ptr as usize + params.cq_off.ring_mask as usize) as *mut u32;
        let cq_ring_entries = (cq_ptr as usize + params.cq_off.ring_entries as usize) as *mut u32;

        trace2(b"ring ready; fd=%d, cq=%d\n", fd, unsafe { *cq_ring_entries });

        let submitter = IORingSubmitter {
            fd: fd,
            cnt: 0,
            sq_ptr: sq_ptr,
            sq_ptr_len: sq_ptr_len,
            sq_tail: sq_tail,
            sq_ring_mask: sq_ring_mask,
            sq_array: sq_array,
            sq_sqes: sq_sqes,
            sq_sqes_len: sq_sqes_len,
        };

        let completer = IORingCompleter {
            fd: fd,
            cq_ptr: cq_ptr,
            cq_ptr_len: cq_ptr_len,
            cq_head: cq_head,
            cq_tail: cq_tail,
            cq_ring_mask: cq_ring_mask,
            cq_cqes: cq_cqes,
        };

        IORingInit::Succeeded(submitter, completer)
    }
}

#[allow(dead_code)]
pub enum IORingSubmit {
    Succeeded(usize),
    SubmissionFailed(isize),
    SubmissionMismatched(usize),
}

pub trait IORingSubmitBuffer {
    fn extract(&self) -> (*const u8, usize);
}

pub struct IORingSubmitEntryTimeout {
    timespec: *const timespec,
}

pub struct IORingSubmitEntryOpenAt<T: IORingSubmitBuffer> {
    fd: u32,
    buf: T,
}

pub struct IORingSubmitEntryClose {
    fd: u32,
}

pub struct IORingSubmitEntryRead {
    fd: u32,
    buf: *const u8,
    len: usize,
    off: u64,
}

pub struct IORingSubmitEntryWrite {
    fd: u32,
    buf: *const u8,
    len: usize,
    off: u64,
}

pub enum IORingSubmitEntry<T: IORingSubmitBuffer> {
    Noop(),
    Timeout(IORingSubmitEntryTimeout),
    OpenAt(IORingSubmitEntryOpenAt<T>),
    Close(IORingSubmitEntryClose),
    Read(IORingSubmitEntryRead),
    Write(IORingSubmitEntryWrite),
}

impl IORingSubmitBuffer for *const u8 {
    fn extract(&self) -> (*const u8, usize) {
        (*self, 0)
    }
}

impl IORingSubmitEntry<*const u8> {
    pub fn noop() -> Self {
        Self::Noop()
    }

    pub fn read(fd: u32, buf: *const u8, len: usize, off: u64) -> Self {
        Self::Read(IORingSubmitEntryRead {
            fd: fd,
            buf: buf,
            len: len,
            off: off,
        })
    }

    pub fn write(fd: u32, buf: *const u8, len: usize, off: u64) -> Self {
        Self::Write(IORingSubmitEntryWrite {
            fd: fd,
            buf: buf,
            len: len,
            off: off,
        })
    }

    pub fn timeout(timespec: *const timespec) -> Self {
        Self::Timeout(IORingSubmitEntryTimeout { timespec: timespec })
    }

    pub fn close(fd: u32) -> Self {
        Self::Close(IORingSubmitEntryClose { fd: fd })
    }
}

impl<T: IORingSubmitBuffer> IORingSubmitEntry<T> {
    pub fn open_at(buf: T) -> Self {
        Self::OpenAt(IORingSubmitEntryOpenAt {
            fd: IORing::AT_FDCWD as u32,
            buf: buf,
        })
    }
}

impl IORingSubmitter {
    pub fn submit<T, const C: usize>(&mut self, user_data: u64, entries: [IORingSubmitEntry<T>; C]) -> IORingSubmit
    where
        T: IORingSubmitBuffer,
    {
        let min_complete = 0;
        let to_submit = entries.len() as u32;

        for entry in entries.into_iter() {
            let ring_mask = unsafe { read_volatile(self.sq_ring_mask) };
            let sq_tail = unsafe { read_volatile(self.sq_tail) & ring_mask };

            let (opcode, fd, ptr, len, offset) = match entry {
                IORingSubmitEntry::Noop() => {
                    /* fmt */
                    (IORing::IORING_OP_NOP, 0, null() as *const u8, 0, 0)
                }
                IORingSubmitEntry::Timeout(data) => {
                    /* fmt */
                    (IORing::IORING_OP_TIMEOUT, 0, data.timespec as *const u8, 1, 0)
                }
                IORingSubmitEntry::OpenAt(data) => match data.buf.extract() {
                    (ptr, _) => (IORing::IORING_OP_OPENAT, data.fd, ptr, 0, 0),
                },
                IORingSubmitEntry::Close(data) => {
                    /* fmt */
                    (IORing::IORING_OP_CLOSE, data.fd, null(), 0, 0)
                }
                IORingSubmitEntry::Read(data) => {
                    /* fmt */
                    (IORing::IORING_OP_READ, data.fd, data.buf, data.len, data.off)
                },
                IORingSubmitEntry::Write(data) => {
                    /* fmt */
                    (IORing::IORING_OP_WRITE, data.fd, data.buf, data.len, data.off)
                },
            };

            unsafe {
                self.cnt += 1;
                let sqe = self.sq_sqes.offset(sq_tail as isize);

                trace3(
                    b"submitting ring operation; op=%d, user=%d, cnt=%d\n",
                    opcode,
                    user_data,
                    self.cnt,
                );

                (*sqe).opcode = opcode;
                (*sqe).fd = fd as i32;
                (*sqe).addr = ptr as u64;
                (*sqe).len = len as u32;
                (*sqe).off = offset;
                (*sqe).user_data = user_data;

                write_volatile(self.sq_array.add(sq_tail as usize), sq_tail);
                write_volatile(self.sq_tail, (sq_tail + 1) & ring_mask);
            }
        }

        let submitted = match sys_io_uring_enter(self.fd, to_submit, min_complete, 0, null(), 0) {
            value if value < 0 => return IORingSubmit::SubmissionFailed(value),
            value => value as usize,
        };

        if submitted != to_submit as usize {
            IORingSubmit::SubmissionMismatched(submitted)
        } else {
            IORingSubmit::Succeeded(submitted)
        }
    }
}

#[allow(dead_code)]
pub enum IORingComplete {
    Succeeded(IORingCompleteEntry),
    UnexpectedEmpty(usize),
    CompletionFailed(isize),
}

#[allow(dead_code)]
#[derive(Default)]
pub struct IORingCompleteEntry {
    pub res: i32,
    pub flags: u32,
    pub user_data: u64,
}

impl IORingCompleter {
    fn extract(&self) -> Option<IORingCompleteEntry> {
        let ring_mask = unsafe { read_volatile(self.cq_ring_mask) };
        let cq_head = unsafe { read_volatile(self.cq_head) };
        let cq_tail = unsafe { read_volatile(self.cq_tail) };

        if cq_head == cq_tail {
            return None;
        }

        let index = cq_head & ring_mask;
        let entry = unsafe { self.cq_cqes.offset(index as isize) };
        let entry = IORingCompleteEntry {
            res: unsafe { (*entry).res },
            flags: unsafe { (*entry).flags },
            user_data: unsafe { (*entry).user_data },
        };

        unsafe { write_volatile(self.cq_head, cq_head + 1) };
        Some(entry)
    }

    pub fn complete(&self) -> IORingComplete {
        if let Some(entry) = self.extract() {
            return IORingComplete::Succeeded(entry);
        }

        let to_submit = 0;
        let min_complete = 1;
        let flags = IORing::IORING_ENTER_GETEVENTS;

        let count = match sys_io_uring_enter(self.fd, to_submit, min_complete, flags, null(), 0) {
            value if value < 0 => return IORingComplete::CompletionFailed(value),
            value => value as usize,
        };

        if let Some(entry) = self.extract() {
            IORingComplete::Succeeded(entry)
        } else {
            IORingComplete::UnexpectedEmpty(count)
        }
    }
}

#[allow(dead_code)]
pub enum IORingJoin {
    Succeeded(IORing),
    MismatchedDescriptor(IORingSubmitter, IORingCompleter),
}

impl IORing {
    pub fn join(submitter: IORingSubmitter, completer: IORingCompleter) -> IORingJoin {
        if submitter.fd != completer.fd {
            return IORingJoin::MismatchedDescriptor(submitter, completer);
        }

        let ring = IORing {
            fd: submitter.fd,
            sq_ptr: submitter.sq_ptr,
            sq_ptr_len: submitter.sq_ptr_len,
            sq_sqes: submitter.sq_sqes,
            sq_sqes_len: submitter.sq_sqes_len,
            cq_ptr: completer.cq_ptr,
            cq_ptr_len: completer.cq_ptr_len,
        };

        IORingJoin::Succeeded(ring)
    }
}

pub enum IORingShutdown {
    Succeeded(),
    Failed(),
}

impl IORing {
    pub fn shutdown(self) -> IORingShutdown {
        let mut failed = false;

        failed = failed || 0 != sys_munmap(self.sq_ptr, self.sq_ptr_len);
        failed = failed || 0 != sys_munmap(self.sq_sqes as *mut (), self.sq_sqes_len);
        failed = failed || 0 != sys_munmap(self.cq_ptr, self.cq_ptr_len);
        failed = failed || 0 > sys_close(self.fd);

        if failed {
            IORingShutdown::Failed()
        } else {
            IORingShutdown::Succeeded()
        }
    }
}
