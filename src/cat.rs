use core::ffi::CStr;

use crate::linux::*;
use crate::uring::*;

pub struct CatCommand<'a> {
    pub src: &'a CStr,
}

impl IORingSubmitBuffer for &MemoryAddress {
    fn extract(self) -> (*const u8, usize) {
        (self.ptr, self.len)
    }
}

impl IORingSubmitBuffer for &MemorySlice {
    fn extract(self) -> (*const u8, usize) {
        (self.ptr, self.len)
    }
}

impl IORingSubmitBuffer for &CStr {
    fn extract(self) -> (*const u8, usize) {
        (self.as_ptr() as *const u8, 0)
    }
}

enum TokenState {
    Idle(),
    Opening(),
    Reading(u32, usize),
    Read(u32, usize, bool),
    Writing(u32, usize, usize),
    Closing(),
}

fn submit<T>(ring: &mut IORing, op: IORingSubmitEntry<T>) -> Result<(), &'static [u8]>
where
    T: IORingSubmitBuffer,
{
    match ring.submit([op]) {
        IORingSubmit::SubmissionFailed(_) => Err(CatCommand::IORING_SUBMISSION_FAILED),
        IORingSubmit::SubmissionMismatched(_) => Err(CatCommand::IORING_SUBMISSION_MISMATCHED),
        IORingSubmit::Succeeded(_) => Ok(()),
    }
}

fn copy(ring: &mut IORing, buf: &MemoryAddress, src: &CStr, dst: u32) -> Result<(), &'static [u8]> {
    let mut tokens = [TokenState::Idle(), TokenState::Idle()];
    submit(ring, IORingSubmitEntry::open_at(src, 0))?;
    tokens[0] = TokenState::Opening();

    loop {
        let entry = loop {
            match ring.complete() {
                IORingComplete::UnexpectedEmpty(_) => continue,
                IORingComplete::Succeeded(entry) => break entry,
                IORingComplete::CompletionFailed(_) => return Err(CatCommand::IORING_COMPLETION_FAILED),
            }
        };

        let token = match tokens.get(entry.user_data as usize) {
            None => return Err(CatCommand::IORING_UNKNOWN_USER_DATA),
            Some(value) => value,
        };

        match token {
            TokenState::Opening() => {
                let fd: u32 = match entry.res {
                    value if value < 0 => return Err(CatCommand::FILE_OPENING_FAILED),
                    value => match value.try_into() {
                        Err(_) => return Err(CatCommand::IORING_INVALID_DESCRIPTOR),
                        Ok(value) => value,
                    },
                };

                submit(ring, IORingSubmitEntry::read(fd, buf, 0, 0))?;
                tokens[0] = TokenState::Reading(fd, 0);
            }
            TokenState::Reading(fd, len) => {
                let read = match entry.res {
                    value if value < 0 => return Err(CatCommand::FILE_READING_FAILED),
                    value => value as usize,
                };

                let buf = match buf.between(0, read) {
                    MemorySlicing::Succeeded(data) => data,
                    _ => return Err(CatCommand::MEMORY_SLICING_FAILED),
                };

                submit(ring, IORingSubmitEntry::write(dst, &buf, 0, 1))?;
                tokens[0] = TokenState::Read(*fd, *len + read, read == 0);
                tokens[1] = TokenState::Writing(dst, read, 0);
            }
            TokenState::Writing(dst_fd, len, off) => {
                let written = match entry.res {
                    value if value < 0 => return Err(CatCommand::FILE_WRITING_FAILED),
                    value => value as usize,
                };

                if *len == *off + written {
                    match tokens[0] {
                        TokenState::Read(src_fd, read, completed) if !completed => {
                            submit(ring, IORingSubmitEntry::read(src_fd, buf, read as u64, 0))?;
                            tokens[1] = TokenState::Idle();
                            tokens[0] = TokenState::Reading(src_fd, read);
                        }
                        TokenState::Read(src_fd, _, _) => {
                            submit(ring, IORingSubmitEntry::close(src_fd, 0))?;
                            tokens[1] = TokenState::Idle();
                            tokens[0] = TokenState::Closing();
                        }
                        _ => return Err(CatCommand::APP_INVALID_TOKEN_STATE),
                    }
                } else {
                    let buf = match buf.between(*off + written, *len) {
                        MemorySlicing::Succeeded(data) => data,
                        _ => return Err(CatCommand::MEMORY_SLICING_FAILED),
                    };

                    submit(ring, IORingSubmitEntry::write(*dst_fd, &buf, 0, 1))?;
                    tokens[1] = TokenState::Writing(*dst_fd, *len, *off + written);
                }
            }
            TokenState::Closing() => {
                return match entry.res {
                    value if value < 0 => Err(CatCommand::FILE_CLOSING_FAILED),
                    _ => Ok(()),
                }
            }
            _ => return Err(CatCommand::APP_INVALID_TOKEN_STATE),
        }
    }
}

pub enum CatCommandExecute {
    Succeeded(),
    Failed(&'static [u8]),
}

fn fail(msg: &'static [u8]) -> CatCommandExecute {
    CatCommandExecute::Failed(msg)
}

impl CatCommand<'_> {
    const IORING_INVALID_DESCRIPTOR: &'static [u8] = b"I/O Ring Init failed: Invalid Descriptor.\n";
    const IORING_SETUP_FAILED: &'static [u8] = b"I/O Ring init failed: Setup Failed.\n";
    const IORING_MAPPING_FAILED: &'static [u8] = b"I/O Ring init failed: Mapping Failed.\n";
    const IORING_SUBMISSION_FAILED: &'static [u8] = b"I/O Ring entry submission failed.\n";
    const IORING_SUBMISSION_MISMATCHED: &'static [u8] = b"I/O Ring entry submission mismatch.\n";
    const IORING_COMPLETION_FAILED: &'static [u8] = b"I/O Ring entry completion failed.\n";
    const IORING_UNKNOWN_USER_DATA: &'static [u8] = b"I/O Ring returned unknown user data.\n";
    const IORING_SHUTDOWN_FAILED: &'static [u8] = b"I/O Ring shutdown failed.\n";

    const MEMORY_ALLOCATION_FAILED: &'static [u8] = b"Cannot allocate memory.\n";
    const MEMORY_DEALLOCATION_FAILED: &'static [u8] = b"Cannot release memory.\n";
    const MEMORY_SLICING_FAILED: &'static [u8] = b"Cannot slice memory.\n";

    const FILE_OPENING_FAILED: &'static [u8] = b"Cannot open source file.\n";
    const FILE_READING_FAILED: &'static [u8] = b"Cannot read source file.\n";
    const FILE_WRITING_FAILED: &'static [u8] = b"Cannot read target file.\n";
    const FILE_CLOSING_FAILED: &'static [u8] = b"Cannot close source file.\n";

    const APP_INVALID_TOKEN_STATE: &'static [u8] = b"Invalid token state.\n";
}

impl CatCommand<'_> {
    pub fn execute(&self) -> CatCommandExecute {
        let mut ring = match IORing::init(32) {
            IORingInit::Succeeded(value) => value,
            IORingInit::InvalidDescriptor(_) => return fail(CatCommand::IORING_INVALID_DESCRIPTOR),
            IORingInit::SetupFailed(_) => return fail(CatCommand::IORING_SETUP_FAILED),
            IORingInit::MappingFailed(_, _) => return fail(CatCommand::IORING_MAPPING_FAILED),
        };

        let mut buffer = match mem_alloc(32 * 4096) {
            MemoryAllocation::Failed(_) => return fail(CatCommand::MEMORY_ALLOCATION_FAILED),
            MemoryAllocation::Succeeded(value) => value,
        };

        if let Err(msg) = copy(&mut ring, &mut buffer, self.src, 1) {
            return fail(msg);
        }

        if let IORingShutdown::Failed() = ring.shutdown() {
            return fail(CatCommand::IORING_SHUTDOWN_FAILED);
        }

        if let MemoryDeallocation::Failed(_) = mem_free(buffer) {
            return fail(CatCommand::MEMORY_DEALLOCATION_FAILED);
        }

        CatCommandExecute::Succeeded()
    }
}
