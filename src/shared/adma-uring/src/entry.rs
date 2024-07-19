use super::kernel::*;

pub struct IORingSubmitEntryTimeout {
    pub timespec: *const timespec,
}

pub struct IORingSubmitEntryOpenAt {
    pub fd: u32,
    pub buf: *const u8,
}

pub struct IORingSubmitEntryClose {
    pub fd: u32,
}

pub struct IORingSubmitEntryRead {
    pub fd: u32,
    pub buf: *const u8,
    pub len: usize,
    pub off: u64,
}

pub struct IORingSubmitEntryWrite {
    pub fd: u32,
    pub buf: *const u8,
    pub len: usize,
    pub off: u64,
}

pub enum IORingSubmitEntry {
    Noop(),
    Timeout(IORingSubmitEntryTimeout),
    OpenAt(IORingSubmitEntryOpenAt),
    Close(IORingSubmitEntryClose),
    Read(IORingSubmitEntryRead),
    Write(IORingSubmitEntryWrite),
}

impl IORingSubmitEntry {
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

    pub fn open_at(path: *const u8) -> Self {
        Self::OpenAt(IORingSubmitEntryOpenAt {
            fd: AT_FDCWD as u32,
            buf: path,
        })
    }
}
