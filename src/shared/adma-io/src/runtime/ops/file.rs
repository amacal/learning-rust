pub trait FileDescriptor {
    fn as_fd(self) -> u32;
}

pub trait Closable {}

pub trait Readable {}
pub trait ReadableAtOffset {}

pub trait Writtable {}
