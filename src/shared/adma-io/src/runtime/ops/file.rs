pub trait AsFileDescriptor {
    fn as_fd(self) -> u32;
}

pub trait AsClosableFileDescriptor {}

pub trait AsReadableFileDescriptor {}
pub trait AsReadableAtOffsetFileDescriptor {}

pub trait AsWrittableFileDescriptor {}
