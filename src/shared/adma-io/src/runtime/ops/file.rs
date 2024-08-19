pub trait FileDescriptor {
    fn as_fd(self) -> u32;
}

pub trait Closable {}

pub trait Duplicable {
    fn from(fd: u32) -> Self;
}

pub trait Readable {}
pub trait ReadableAtOffset {}

pub trait Writtable {}
pub trait WrittableAtOffset {}
