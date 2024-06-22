pub trait AsNullTerminatedRef {
    fn as_ptr(&self) -> *const u8;
}
