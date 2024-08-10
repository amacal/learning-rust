pub trait AsFileDescriptor {
    fn as_fd(self) -> u32;
}

pub trait AsClosableFileDescriptor {}

pub trait AsReadableFileDescriptor {}
pub trait AsReadableAtOffsetFileDescriptor {}

pub trait AsWrittableFileDescriptor {}

pub struct StdOutDescriptor {}

impl StdOutDescriptor {
    pub fn new() -> Self {
        Self {}
    }
}

impl AsFileDescriptor for &StdOutDescriptor {
    fn as_fd(self) -> u32 {
        1
    }
}

impl AsWrittableFileDescriptor for &StdOutDescriptor {}

pub struct FileDescriptor {
    value: u32,
}

impl FileDescriptor {
    pub fn new(value: u32) -> Self {
        Self { value: value }
    }
}

impl AsFileDescriptor for FileDescriptor {
    fn as_fd(self) -> u32 {
        self.value
    }
}

impl AsFileDescriptor for &FileDescriptor {
    fn as_fd(self) -> u32 {
        self.value
    }
}

impl AsClosableFileDescriptor for FileDescriptor {}

impl AsReadableAtOffsetFileDescriptor for &FileDescriptor {}
