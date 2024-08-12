use super::*;

impl IORuntimeOps {
    pub fn stdout(&self) -> impl FileDescriptor + Writtable + Copy {
        StdOutDescriptor {}
    }
}

#[derive(Clone, Copy)]
struct StdOutDescriptor {}

impl FileDescriptor for StdOutDescriptor {
    fn as_fd(self) -> u32 {
        1
    }
}

impl Writtable for StdOutDescriptor {}
